//! AI categorisation (PRD 5.8): one provider interface for OpenAI, Anthropic,
//! Google Gemini and OpenAI-compatible endpoints.
//!
//! Rules enforced here:
//! - Only a bookmark's URL, domain, title and description are sent, plus the
//!   names/descriptions of categories the user hasn't excluded.
//! - Responses must be structured JSON; anything invalid is rejected.
//! - Suggested categories must exist (or be a new name when the user allows
//!   new categories). Excluded categories are never suggested.
//! - API keys are passed in per call and never stored, logged or echoed.

use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::db::now;
use crate::error::{Error, Result};
use crate::models::{Bookmark, Category, NewCategory};
use crate::{bookmarks, categories};

const TIMEOUT: Duration = Duration::from_secs(45);
const ANTHROPIC_VERSION: &str = "2026-10-21";
pub const DEFAULT_ANTHROPIC_MODEL: &str = "claude-opus-5-5";

// ------------------------------------------------------------------ config

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Openai,
    #[default]
    Anthropic,
    Gemini,
    Custom,
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Provider::Openai => "openai",
            Provider::Anthropic => "anthropic",
            Provider::Gemini => "gemini",
            Provider::Custom => "custom",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Provider::Openai => "OpenAI",
            Provider::Anthropic => "Anthropic",
            Provider::Gemini => "Google Gemini",
            Provider::Custom => "Custom endpoint",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Manual,
    #[default]
    Suggestion,
    Automatic,
}

/// Non-secret AI settings, stored in `application_settings` (never the key).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AiConfig {
    pub enabled: bool,
    pub provider: Provider,
    pub model: String,
    /// Base URL for the custom (OpenAI-compatible) provider, e.g. `http://localhost:11434/v1`.
    pub endpoint: String,
    pub mode: Mode,
    pub allow_new_categories: bool,
    pub never_overwrite_manual: bool,
    pub auto_categorise_new: bool,
    pub excluded_category_ids: Vec<String>,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: Provider::Anthropic,
            model: DEFAULT_ANTHROPIC_MODEL.into(),
            endpoint: String::new(),
            mode: Mode::Suggestion,
            allow_new_categories: false,
            never_overwrite_manual: true,
            auto_categorise_new: true,
            excluded_category_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AiUsage {
    pub requests: u64,
    pub suggestions: u64,
    pub accepted: u64,
    pub errors: u64,
    pub last_error: Option<String>,
    pub last_used_at: Option<String>,
}

fn get_setting<T: for<'de> Deserialize<'de> + Default>(conn: &Connection, key: &str) -> Result<T> {
    let raw: Option<String> =
        conn.query_row("SELECT value FROM application_settings WHERE key = ?1", [key], |r| r.get(0)).optional()?;
    Ok(raw.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default())
}

fn put_setting<T: Serialize>(conn: &Connection, key: &str, value: &T) -> Result<()> {
    let json = serde_json::to_string(value).map_err(|_| Error::Validation("Couldn't save settings.".into()))?;
    conn.execute(
        "INSERT INTO application_settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, json],
    )?;
    Ok(())
}

pub fn get_config(conn: &Connection) -> Result<AiConfig> {
    get_setting(conn, "ai_config")
}

pub fn save_config(conn: &Connection, cfg: &AiConfig) -> Result<AiConfig> {
    let mut cfg = cfg.clone();
    cfg.model = cfg.model.trim().to_string();
    cfg.endpoint = cfg.endpoint.trim().trim_end_matches('/').to_string();
    if cfg.provider == Provider::Custom && !cfg.endpoint.is_empty() {
        let u = url::Url::parse(&cfg.endpoint).map_err(|_| Error::Validation("Enter the endpoint as a full URL, like https://host/v1.".into()))?;
        if !matches!(u.scheme(), "http" | "https") {
            return Err(Error::Validation("The endpoint must start with http:// or https://.".into()));
        }
    }
    put_setting(conn, "ai_config", &cfg)?;
    Ok(cfg)
}

pub fn get_usage(conn: &Connection) -> Result<AiUsage> {
    get_setting(conn, "ai_usage")
}

fn bump_usage(conn: &Connection, f: impl FnOnce(&mut AiUsage)) -> Result<()> {
    let mut u = get_usage(conn)?;
    f(&mut u);
    u.last_used_at = Some(now());
    put_setting(conn, "ai_usage", &u)
}

pub fn record_error(conn: &Connection, message: &str) -> Result<()> {
    bump_usage(conn, |u| {
        u.errors += 1;
        u.last_error = Some(message.to_string());
    })
}

// ------------------------------------------------------------------ prompt

/// What the model must return (JSON Schema, `additionalProperties: false`).
pub fn response_schema(category_ids: &[String], allow_new: bool) -> Value {
    let mut ids: Vec<Value> = category_ids.iter().map(|i| Value::String(i.clone())).collect();
    ids.push(Value::String("none".into()));
    let new_name = if allow_new {
        json!({ "anyOf": [ { "type": "string" }, { "type": "null" } ] })
    } else {
        json!({ "type": "null" })
    };
    json!({
        "type": "object",
        "properties": {
            "category_id": { "type": "string", "enum": ids },
            "new_category": new_name,
            "tags": { "type": "array", "items": { "type": "string" } },
            "confidence": { "type": "number" },
            "reason": { "type": "string" }
        },
        "required": ["category_id", "new_category", "tags", "confidence", "reason"],
        "additionalProperties": false
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Draft {
    pub url: String,
    pub domain: String,
    pub title: String,
    pub description: Option<String>,
}

impl From<&Bookmark> for Draft {
    fn from(b: &Bookmark) -> Self {
        Draft { url: b.url.clone(), domain: b.domain.clone(), title: b.title.clone(), description: b.description.clone() }
    }
}

fn category_path(cats: &[Category], c: &Category) -> String {
    let mut parts = vec![c.name.clone()];
    let mut parent = c.parent_id.clone();
    while let Some(p) = parent.and_then(|id| cats.iter().find(|x| x.id == id)) {
        parts.insert(0, p.name.clone());
        parent = p.parent_id.clone();
    }
    parts.join(" › ")
}

/// Build (system, user) prompts. Strip query strings and fragments from the
/// URL so tracking tokens or private parameters are never sent.
pub fn build_prompt(draft: &Draft, cats: &[Category], cfg: &AiConfig) -> (String, String, Vec<String>) {
    let allowed: Vec<&Category> = cats.iter().filter(|c| !cfg.excluded_category_ids.contains(&c.id)).collect();
    let ids: Vec<String> = allowed.iter().map(|c| c.id.clone()).collect();
    let listing = allowed
        .iter()
        .map(|c| match &c.description {
            Some(d) => format!("- id: {} | {} — {}", c.id, category_path(cats, c), d),
            None => format!("- id: {} | {}", c.id, category_path(cats, c)),
        })
        .collect::<Vec<_>>()
        .join("\n");
    let system = format!(
        "You sort bookmarks into the user's existing categories.\n\
         Prefer the most specific matching category (a subcategory over its parent). \
         Use category_id \"none\" when nothing fits well.{}\n\
         Return 1-4 short lowercase tags without '#'. confidence is 0 to 1. \
         reason is one short sentence a user would understand.",
        if cfg.allow_new_categories {
            " If nothing fits and a new category would clearly help, set category_id to \"none\" and put a short name (max 40 characters) in new_category; otherwise new_category is null."
        } else {
            " new_category must be null."
        }
    );
    let url = url::Url::parse(&draft.url)
        .map(|mut u| {
            u.set_query(None);
            u.set_fragment(None);
            u.to_string()
        })
        .unwrap_or_else(|_| draft.domain.clone());
    let user = format!(
        "Categories:\n{}\n\nBookmark:\nURL: {}\nDomain: {}\nTitle: {}\nDescription: {}",
        if listing.is_empty() { "(none yet)".into() } else { listing },
        url,
        draft.domain,
        draft.title,
        draft.description.as_deref().unwrap_or("(none)")
    );
    (system, user, ids)
}

// ------------------------------------------------------------------ result

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub category_id: Option<String>,
    pub new_category: Option<String>,
    pub tags: Vec<String>,
    pub confidence: f64,
    pub reason: String,
    pub provider: String,
    pub model: String,
}

#[derive(Deserialize)]
struct RawResult {
    category_id: String,
    #[serde(default)]
    new_category: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    confidence: f64,
    #[serde(default)]
    reason: String,
}

/// Validate the model's JSON before anything touches the database.
pub fn parse_result(text: &str, allowed_ids: &[String], allow_new: bool, provider: Provider, model: &str) -> Result<Suggestion> {
    let invalid = || Error::Validation("The AI returned an answer Magpie couldn't use. Try again.".into());
    // Some providers wrap JSON in a code fence even when asked not to.
    let trimmed = text.trim().trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
    let raw: RawResult = serde_json::from_str(trimmed).map_err(|_| invalid())?;
    let category_id = match raw.category_id.as_str() {
        "none" | "" => None,
        id if allowed_ids.iter().any(|a| a == id) => Some(id.to_string()),
        _ => return Err(invalid()),
    };
    let new_category = if allow_new && category_id.is_none() {
        raw.new_category.map(|n| n.trim().chars().take(40).collect::<String>()).filter(|n| !n.is_empty())
    } else {
        None
    };
    let tags = raw
        .tags
        .iter()
        .map(|t| t.trim().trim_start_matches('#').to_lowercase())
        .filter(|t| !t.is_empty() && t.len() <= 40)
        .take(4)
        .collect();
    Ok(Suggestion {
        category_id,
        new_category,
        tags,
        confidence: if raw.confidence.is_finite() { raw.confidence.clamp(0.0, 1.0) } else { 0.0 },
        reason: raw.reason.chars().take(300).collect(),
        provider: provider.as_str().into(),
        model: model.into(),
    })
}

// ------------------------------------------------------------------ network

fn client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(TIMEOUT)
        .user_agent("Magpie/0.1")
        .build()
        .map_err(|_| Error::Network("Couldn't start a network request.".into()))
}

/// Turn an HTTP failure into a friendly message that never contains the key.
fn http_error(status: u16, body: &str, key: &str) -> Error {
    let msg = match status {
        401 => "The API key was rejected. Check it in Settings › AI.".to_string(),
        403 => "This API key isn't allowed to use that model.".to_string(),
        404 => "The model or endpoint wasn't found. Pick another model.".to_string(),
        429 => "The provider is rate-limiting requests or the quota is used up. Try again later.".to_string(),
        500..=599 => "The AI provider is having problems right now. Try again later.".to_string(),
        _ => {
            let detail = serde_json::from_str::<Value>(body)
                .ok()
                .and_then(|v| {
                    v.pointer("/error/message").and_then(Value::as_str).map(str::to_string)
                })
                .unwrap_or_default();
            format!("The AI provider returned error {status}. {detail}")
        }
    };
    let scrubbed = if key.len() >= 6 { msg.replace(key, "••••") } else { msg };
    Error::Network(scrubbed.trim().to_string())
}

fn send(req: reqwest::blocking::RequestBuilder, key: &str) -> Result<Value> {
    let resp = req.send().map_err(|e| {
        Error::Network(if e.is_timeout() { "The AI provider took too long to answer.".into() } else { "Couldn't reach the AI provider.".into() })
    })?;
    let status = resp.status().as_u16();
    let body = resp.text().unwrap_or_default();
    if !(200..300).contains(&status) {
        return Err(http_error(status, &body, key));
    }
    serde_json::from_str(&body).map_err(|_| Error::Network("The AI provider sent an unreadable response.".into()))
}

fn openai_base(cfg: &AiConfig) -> String {
    match cfg.provider {
        Provider::Custom => cfg.endpoint.clone(),
        _ => "https://api.openai.com/v1".into(),
    }
}

/// Run one categorisation request and return the raw JSON text.
pub fn complete(cfg: &AiConfig, key: &str, system: &str, user: &str, schema: &Value) -> Result<String> {
    if cfg.model.is_empty() {
        return Err(Error::Validation("Choose a model in Settings › AI.".into()));
    }
    let c = client()?;
    match cfg.provider {
        Provider::Anthropic => {
            let body = json!({
                "model": cfg.model,
                "max_tokens": 1024,
                "system": system,
                "messages": [{ "role": "user", "content": user }],
                "output_config": { "effort": "low", "format": { "type": "json_schema", "schema": schema } },
                // Server-side refusal fallback (Anthropic's recommended default for current models).
                "fallbacks": "default"
            });
            let v = send(
                c.post("https://api.anthropic.com/v1/messages")
                    .header("x-api-key", key)
                    .header("anthropic-version", ANTHROPIC_VERSION)
                    .header("anthropic-beta", "server-side-fallback-2026-07-01")
                    .json(&body),
                key,
            )?;
            if v.get("stop_reason").and_then(Value::as_str) == Some("refusal") {
                return Err(Error::Validation("The AI declined to categorise this bookmark.".into()));
            }
            v.get("content")
                .and_then(Value::as_array)
                .and_then(|blocks| blocks.iter().find(|b| b.get("type").and_then(Value::as_str) == Some("text")))
                .and_then(|b| b.get("text").and_then(Value::as_str))
                .map(str::to_string)
                .ok_or_else(|| Error::Network("The AI provider sent an empty answer.".into()))
        }
        Provider::Openai | Provider::Custom => {
            let response_format = if cfg.provider == Provider::Openai {
                json!({ "type": "json_schema", "json_schema": { "name": "categorisation", "strict": true, "schema": schema } })
            } else {
                // Many OpenAI-compatible servers only support plain JSON mode.
                json!({ "type": "json_object" })
            };
            let system = if cfg.provider == Provider::Custom {
                format!("{system}\nReply with JSON only, matching this schema: {schema}")
            } else {
                system.to_string()
            };
            let body = json!({
                "model": cfg.model,
                "messages": [ { "role": "system", "content": system }, { "role": "user", "content": user } ],
                "response_format": response_format
            });
            let mut req = c.post(format!("{}/chat/completions", openai_base(cfg))).json(&body);
            if !key.is_empty() {
                req = req.bearer_auth(key);
            }
            let v = send(req, key)?;
            v.pointer("/choices/0/message/content")
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| Error::Network("The AI provider sent an empty answer.".into()))
        }
        Provider::Gemini => {
            let body = json!({
                "systemInstruction": { "parts": [{ "text": system }] },
                "contents": [{ "role": "user", "parts": [{ "text": user }] }],
                "generationConfig": { "responseMimeType": "application/json", "temperature": 0 }
            });
            let model = cfg.model.trim_start_matches("models/");
            let v = send(
                c.post(format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"))
                    .header("x-goog-api-key", key)
                    .json(&body),
                key,
            )?;
            v.pointer("/candidates/0/content/parts/0/text")
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| Error::Network("The AI provider sent an empty answer.".into()))
        }
    }
}

/// Models the key can use (also serves as "Test connection").
pub fn list_models(cfg: &AiConfig, key: &str) -> Result<Vec<String>> {
    let c = client()?;
    let mut models: Vec<String> = match cfg.provider {
        Provider::Anthropic => {
            let v = send(
                c.get("https://api.anthropic.com/v1/models?limit=100")
                    .header("x-api-key", key)
                    .header("anthropic-version", ANTHROPIC_VERSION),
                key,
            )?;
            v.get("data").and_then(Value::as_array).into_iter().flatten().filter_map(|m| m.get("id")?.as_str().map(str::to_string)).collect()
        }
        Provider::Openai | Provider::Custom => {
            let mut req = c.get(format!("{}/models", openai_base(cfg)));
            if !key.is_empty() {
                req = req.bearer_auth(key);
            }
            let v = send(req, key)?;
            let all = v.get("data").and_then(Value::as_array).into_iter().flatten().filter_map(|m| m.get("id")?.as_str().map(str::to_string));
            if cfg.provider == Provider::Openai {
                all.filter(|id| (id.starts_with("gpt-") || id.starts_with('o')) && !id.contains("audio") && !id.contains("realtime") && !id.contains("tts") && !id.contains("image") && !id.contains("transcribe")).collect()
            } else {
                all.collect()
            }
        }
        Provider::Gemini => {
            let v = send(c.get("https://generativelanguage.googleapis.com/v1beta/models?pageSize=200").header("x-goog-api-key", key), key)?;
            v.get("models")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|m| {
                    m.get("supportedGenerationMethods")
                        .and_then(Value::as_array)
                        .is_some_and(|a| a.iter().any(|x| x.as_str() == Some("generateContent")))
                })
                .filter_map(|m| m.get("name")?.as_str().map(|n| n.trim_start_matches("models/").to_string()))
                .collect()
        }
    };
    models.sort();
    models.dedup();
    Ok(models)
}

/// Ask the provider for a category for one bookmark (nothing is saved).
pub fn suggest(conn_cats: &[Category], cfg: &AiConfig, key: &str, draft: &Draft) -> Result<Suggestion> {
    let (system, user, ids) = build_prompt(draft, conn_cats, cfg);
    let schema = response_schema(&ids, cfg.allow_new_categories);
    let text = complete(cfg, key, &system, &user, &schema)?;
    parse_result(&text, &ids, cfg.allow_new_categories, cfg.provider, &cfg.model)
}

// ------------------------------------------------------------------ storage

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredSuggestion {
    pub bookmark: Bookmark,
    pub category_id: Option<String>,
    pub new_category: Option<String>,
    pub tags: Vec<String>,
    pub confidence: f64,
    pub reason: Option<String>,
    pub provider: String,
    pub model: String,
    pub created_at: String,
}

pub fn store_suggestion(conn: &Connection, bookmark_id: &str, s: &Suggestion) -> Result<()> {
    conn.execute(
        "INSERT INTO ai_suggestions (bookmark_id, category_id, new_category_name, tags, confidence, reason, provider, model, status, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'pending', ?9)
         ON CONFLICT(bookmark_id) DO UPDATE SET category_id = excluded.category_id, new_category_name = excluded.new_category_name,
            tags = excluded.tags, confidence = excluded.confidence, reason = excluded.reason, provider = excluded.provider,
            model = excluded.model, status = 'pending', created_at = excluded.created_at",
        params![bookmark_id, s.category_id, s.new_category, s.tags.join(","), s.confidence, s.reason, s.provider, s.model, now()],
    )?;
    bump_usage(conn, |u| {
        u.requests += 1;
        u.suggestions += 1;
    })
}

/// Pending suggestions that point somewhere (existing or new category).
pub fn list_suggestions(conn: &Connection) -> Result<Vec<StoredSuggestion>> {
    let rows: Vec<(String, Option<String>, Option<String>, Option<String>, f64, Option<String>, String, String, String)> = {
        let mut stmt = conn.prepare(
            "SELECT s.bookmark_id, s.category_id, s.new_category_name, s.tags, s.confidence, s.reason, s.provider, s.model, s.created_at
             FROM ai_suggestions s JOIN bookmarks b ON b.id = s.bookmark_id
             WHERE s.status = 'pending' AND b.deleted_at IS NULL AND (s.category_id IS NOT NULL OR s.new_category_name IS NOT NULL)
             ORDER BY s.confidence DESC, s.created_at DESC",
        )?;
        let it = stmt.query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?))
        })?;
        it.collect::<rusqlite::Result<Vec<_>>>()?
    };
    rows.into_iter()
        .map(|(id, category_id, new_category, tags, confidence, reason, provider, model, created_at)| {
            Ok(StoredSuggestion {
                bookmark: bookmarks::get(conn, &id)?,
                category_id,
                new_category,
                tags: tags.map(|t| t.split(',').filter(|x| !x.is_empty()).map(String::from).collect()).unwrap_or_default(),
                confidence,
                reason,
                provider,
                model,
                created_at,
            })
        })
        .collect()
}

const PALETTE: &[&str] = &["#2563EB", "#7C3AED", "#DB2777", "#EA580C", "#16A34A", "#0D9488", "#D97706", "#475569"];

/// Apply suggestions: set the category (creating a new one only if the
/// suggestion carries a name, which only happens when allowed), add tags,
/// record confidence. Returns how many bookmarks changed.
pub fn accept(conn: &Connection, bookmark_ids: &[String]) -> Result<usize> {
    // Each bookmark is updated atomically by bookmarks::update.
    let tx = conn;
    let mut n = 0;
    for id in bookmark_ids {
        let row: Option<(Option<String>, Option<String>, Option<String>, f64)> = tx
            .query_row(
                "SELECT category_id, new_category_name, tags, confidence FROM ai_suggestions WHERE bookmark_id = ?1 AND status = 'pending'",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?;
        let Some((category_id, new_name, tags, confidence)) = row else { continue };
        let category_id = match (category_id, new_name) {
            (Some(c), _) => Some(c),
            (None, Some(name)) => {
                let existing: Option<String> = tx
                    .query_row("SELECT id FROM categories WHERE parent_id IS NULL AND name = ?1 COLLATE NOCASE", [&name], |r| r.get(0))
                    .optional()?;
                match existing {
                    Some(e) => Some(e),
                    None => {
                        let count: i64 = tx.query_row("SELECT COUNT(*) FROM categories", [], |r| r.get(0))?;
                        let color = PALETTE[count as usize % PALETTE.len()].to_string();
                        Some(categories::create(tx, NewCategory { name, description: None, color, icon: None, parent_id: None })?.id)
                    }
                }
            }
            (None, None) => None,
        };
        let current = bookmarks::get(tx, id)?;
        let mut all_tags = current.tags.clone();
        for t in tags.unwrap_or_default().split(',').filter(|t| !t.is_empty()) {
            if !all_tags.iter().any(|x| x.eq_ignore_ascii_case(t)) {
                all_tags.push(t.to_string());
            }
        }
        bookmarks::update(
            tx,
            crate::models::UpdateBookmark {
                id: id.clone(),
                url: current.url.clone(),
                title: current.title.clone(),
                description: current.description.clone(),
                category_id: category_id.or(current.category_id.clone()),
                is_favourite: current.is_favourite,
                tags: all_tags,
            },
        )?;
        tx.execute("UPDATE bookmarks SET ai_category_confidence = ?2 WHERE id = ?1", params![id, confidence])?;
        tx.execute("DELETE FROM ai_suggestions WHERE bookmark_id = ?1", [id])?;
        n += 1;
    }
    bump_usage(conn, |u| u.accepted += n as u64)?;
    Ok(n)
}

pub fn reject(conn: &Connection, bookmark_ids: &[String]) -> Result<usize> {
    let mut n = 0;
    for id in bookmark_ids {
        n += conn.execute("UPDATE ai_suggestions SET status = 'rejected' WHERE bookmark_id = ?1", [id])?;
    }
    Ok(n)
}

/// Bookmarks a batch run should look at: the given ids, or every
/// uncategorized bookmark without a pending/rejected suggestion.
pub fn batch_targets(conn: &Connection, ids: Option<&[String]>, cfg: &AiConfig, limit: usize) -> Result<Vec<Bookmark>> {
    let list: Vec<Bookmark> = match ids {
        Some(ids) => ids.iter().filter_map(|id| bookmarks::get(conn, id).ok()).filter(|b| b.deleted_at.is_none()).collect(),
        None => {
            let mut stmt = conn.prepare(
                "SELECT id FROM bookmarks b WHERE b.deleted_at IS NULL AND b.category_id IS NULL
                 AND NOT EXISTS (SELECT 1 FROM ai_suggestions s WHERE s.bookmark_id = b.id)
                 ORDER BY b.created_at DESC LIMIT ?1",
            )?;
            let ids = stmt.query_map([limit as i64], |r| r.get::<_, String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
            ids.iter().filter_map(|id| bookmarks::get(conn, id).ok()).collect()
        }
    };
    // Bookmarks the user already filed themselves are left alone by default.
    Ok(list.into_iter().filter(|b| !(cfg.never_overwrite_manual && b.category_id.is_some() && ids.is_none())).take(limit).collect())
}

/// Automatic mode: file a bookmark straight away when the AI picked an
/// existing category and the bookmark has none (or overwriting is allowed).
pub fn auto_apply(conn: &Connection, cfg: &AiConfig, bookmark: &Bookmark, s: &Suggestion) -> Result<bool> {
    if cfg.mode != Mode::Automatic || s.category_id.is_none() {
        return Ok(false);
    }
    if bookmark.category_id.is_some() && cfg.never_overwrite_manual {
        return Ok(false);
    }
    accept(conn, &[bookmark.id.clone()]).map(|n| n > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;
    use crate::models::NewBookmark;

    fn cat(conn: &Connection, name: &str, parent: Option<&str>) -> Category {
        categories::create(conn, NewCategory { name: name.into(), description: Some(format!("{name} things")), color: "#2563EB".into(), icon: None, parent_id: parent.map(String::from) }).unwrap()
    }

    #[test]
    fn config_round_trip_and_defaults() {
        let c = open_in_memory().unwrap();
        let d = get_config(&c).unwrap();
        assert!(!d.enabled && d.mode == Mode::Suggestion && !d.allow_new_categories && d.never_overwrite_manual);
        assert_eq!(d.model, DEFAULT_ANTHROPIC_MODEL);
        let mut cfg = d.clone();
        cfg.provider = Provider::Custom;
        cfg.endpoint = " http://localhost:11434/v1/ ".into();
        let saved = save_config(&c, &cfg).unwrap();
        assert_eq!(saved.endpoint, "http://localhost:11434/v1");
        assert_eq!(get_config(&c).unwrap(), saved);
        cfg.endpoint = "ftp://x".into();
        assert!(save_config(&c, &cfg).is_err());
    }

    #[test]
    fn prompt_hides_query_strings_and_excluded_categories() {
        let c = open_in_memory().unwrap();
        let design = cat(&c, "Design", None);
        let kits = cat(&c, "UI Kits", Some(&design.id));
        let reading = cat(&c, "Reading", None);
        let cats = categories::list(&c).unwrap();
        let cfg = AiConfig { excluded_category_ids: vec![reading.id.clone()], ..Default::default() };
        let draft = Draft { url: "https://figma.com/community?token=SECRET#x".into(), domain: "figma.com".into(), title: "Figma".into(), description: None };
        let (system, user, ids) = build_prompt(&draft, &cats, &cfg);
        assert!(!user.contains("SECRET") && !user.contains("#x"));
        assert!(user.contains("Design › UI Kits"));
        assert!(!user.contains("Reading"));
        assert!(ids.contains(&kits.id) && !ids.contains(&reading.id));
        assert!(system.contains("new_category must be null"));
        let schema = response_schema(&ids, false);
        assert_eq!(schema["additionalProperties"], json!(false));
        assert_eq!(schema["properties"]["new_category"], json!({ "type": "null" }));
    }

    #[test]
    fn results_are_validated() {
        let ids = vec!["c1".to_string()];
        let ok = parse_result(r##"{"category_id":"c1","new_category":null,"tags":["#UI","design","", "x","y","z"],"confidence":1.7,"reason":"UI kits."}"##, &ids, false, Provider::Openai, "m").unwrap();
        assert_eq!(ok.category_id.as_deref(), Some("c1"));
        assert_eq!(ok.tags, vec!["ui", "design", "x", "y"]);
        assert_eq!(ok.confidence, 1.0);
        // Unknown category ids and garbage are rejected.
        assert!(parse_result(r#"{"category_id":"nope","new_category":null,"tags":[],"confidence":0.5,"reason":""}"#, &ids, false, Provider::Openai, "m").is_err());
        assert!(parse_result("Sure! Here's the category: Design", &ids, false, Provider::Openai, "m").is_err());
        // New categories only when allowed.
        let raw = r#"```json
{"category_id":"none","new_category":"Photography","tags":[],"confidence":0.8,"reason":"Photo site."}
```"#;
        assert_eq!(parse_result(raw, &ids, false, Provider::Gemini, "m").unwrap().new_category, None);
        assert_eq!(parse_result(raw, &ids, true, Provider::Gemini, "m").unwrap().new_category.as_deref(), Some("Photography"));
    }

    #[test]
    fn suggestions_accept_and_reject() {
        let c = open_in_memory().unwrap();
        let design = cat(&c, "Design", None);
        let a = bookmarks::create(&c, NewBookmark { url: "https://a.com".into(), tags: vec!["mine".into()], ..Default::default() }).unwrap();
        let b = bookmarks::create(&c, NewBookmark { url: "https://b.com".into(), ..Default::default() }).unwrap();
        let x = bookmarks::create(&c, NewBookmark { url: "https://x.com".into(), ..Default::default() }).unwrap();
        let s = |cat: Option<String>, new: Option<String>| Suggestion { category_id: cat, new_category: new, tags: vec!["ui".into()], confidence: 0.9, reason: "r".into(), provider: "anthropic".into(), model: "m".into() };
        store_suggestion(&c, &a.id, &s(Some(design.id.clone()), None)).unwrap();
        store_suggestion(&c, &b.id, &s(None, Some("Photography".into()))).unwrap();
        store_suggestion(&c, &x.id, &s(None, None)).unwrap();
        assert_eq!(list_suggestions(&c).unwrap().len(), 2, "suggestions with no target aren't listed");

        let cfg = AiConfig::default();
        let targets = batch_targets(&c, None, &cfg, 50).unwrap();
        assert!(targets.is_empty(), "bookmarks with a suggestion aren't re-sent");

        assert_eq!(accept(&c, &[a.id.clone(), b.id.clone()]).unwrap(), 2);
        let a2 = bookmarks::get(&c, &a.id).unwrap();
        assert_eq!(a2.category_id.as_deref(), Some(design.id.as_str()));
        assert_eq!(a2.tags, vec!["mine", "ui"]);
        let b2 = bookmarks::get(&c, &b.id).unwrap();
        let photo = categories::get(&c, b2.category_id.as_deref().unwrap()).unwrap();
        assert_eq!(photo.name, "Photography");
        assert!(list_suggestions(&c).unwrap().is_empty());
        assert_eq!(get_usage(&c).unwrap().accepted, 2);

        store_suggestion(&c, &x.id, &s(Some(design.id.clone()), None)).unwrap();
        assert_eq!(reject(&c, &[x.id.clone()]).unwrap(), 1);
        assert!(list_suggestions(&c).unwrap().is_empty());
    }

    #[test]
    fn automatic_mode_respects_manual_categories() {
        let c = open_in_memory().unwrap();
        let design = cat(&c, "Design", None);
        let dev = cat(&c, "Dev", None);
        let filed = bookmarks::create(&c, NewBookmark { url: "https://a.com".into(), category_id: Some(dev.id.clone()), ..Default::default() }).unwrap();
        let loose = bookmarks::create(&c, NewBookmark { url: "https://b.com".into(), ..Default::default() }).unwrap();
        let cfg = AiConfig { mode: Mode::Automatic, ..Default::default() };
        let s = Suggestion { category_id: Some(design.id.clone()), new_category: None, tags: vec![], confidence: 0.9, reason: "r".into(), provider: "openai".into(), model: "m".into() };
        store_suggestion(&c, &filed.id, &s).unwrap();
        assert!(!auto_apply(&c, &cfg, &filed, &s).unwrap());
        store_suggestion(&c, &loose.id, &s).unwrap();
        assert!(auto_apply(&c, &cfg, &loose, &s).unwrap());
        assert_eq!(bookmarks::get(&c, &loose.id).unwrap().category_id.as_deref(), Some(design.id.as_str()));
        assert_eq!(bookmarks::get(&c, &filed.id).unwrap().category_id.as_deref(), Some(dev.id.as_str()));
    }

    #[test]
    fn errors_never_contain_the_key() {
        let e = http_error(400, r#"{"error":{"message":"Incorrect API key provided: sk-test-1234567890"}}"#, "sk-test-1234567890");
        assert!(!e.to_string().contains("sk-test-1234567890"));
        assert!(http_error(401, "", "k").to_string().contains("rejected"));
    }
}
