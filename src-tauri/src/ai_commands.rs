//! AI categorisation commands. API keys live only in Windows Credential
//! Manager; they are read here when a request is made and never returned to
//! the UI, written to the database, or logged.

use std::sync::atomic::Ordering;

use magpie_core::ai::{self, AiConfig, AiUsage, Draft, Provider, StoredSuggestion, Suggestion};
use magpie_core::{Error, bookmarks, categories, urls};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::commands::AppState;

type CmdResult<T> = Result<T, Error>;

const SERVICE: &str = "com.shaikmoosa.magpie.ai";
const BATCH_LIMIT: usize = 200;
const PARALLEL: usize = 4;

fn entry(provider: Provider) -> CmdResult<keyring::Entry> {
    keyring::Entry::new(SERVICE, provider.as_str()).map_err(|_| Error::Validation("Windows Credential Manager isn't available.".into()))
}

fn read_key(provider: Provider) -> CmdResult<Option<String>> {
    match entry(provider)?.get_password() {
        Ok(k) => Ok(Some(k)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(Error::Validation("Couldn't read the API key from Windows Credential Manager.".into())),
    }
}

/// The key for a provider; custom endpoints may run without one.
fn require_key(provider: Provider) -> CmdResult<String> {
    match read_key(provider)? {
        Some(k) => Ok(k),
        None if provider == Provider::Custom => Ok(String::new()),
        None => Err(Error::Validation(format!("Add your {} API key in Settings › AI.", provider.label()))),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSettings {
    pub config: AiConfig,
    /// Which providers have a key stored (never the key itself).
    pub keys: Vec<Provider>,
    pub usage: AiUsage,
    pub running: bool,
}

#[tauri::command]
pub async fn get_ai_settings(state: State<'_, AppState>) -> CmdResult<AiSettings> {
    let keys = [Provider::Openai, Provider::Anthropic, Provider::Gemini, Provider::Custom]
        .into_iter()
        .filter(|p| matches!(read_key(*p), Ok(Some(_))))
        .collect();
    let db = state.db();
    Ok(AiSettings { config: ai::get_config(&db)?, keys, usage: ai::get_usage(&db)?, running: state.ai_worker.load(Ordering::SeqCst) })
}

#[tauri::command]
pub async fn save_ai_config(state: State<'_, AppState>, config: AiConfig) -> CmdResult<AiConfig> {
    ai::save_config(&state.db(), &config)
}

/// Store a key in Windows Credential Manager.
#[tauri::command]
pub async fn set_ai_key(provider: Provider, key: String) -> CmdResult<()> {
    let key = key.trim();
    if key.is_empty() || key.len() > 400 || key.chars().any(char::is_whitespace) {
        return Err(Error::Validation("That doesn't look like an API key.".into()));
    }
    entry(provider)?.set_password(key).map_err(|_| Error::Validation("Couldn't save the key to Windows Credential Manager.".into()))
}

#[tauri::command]
pub async fn remove_ai_key(provider: Provider) -> CmdResult<()> {
    match entry(provider)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err(Error::Validation("Couldn't remove the key from Windows Credential Manager.".into())),
    }
}

/// List models for the given (unsaved) settings. Also used by "Test connection".
#[tauri::command]
pub async fn list_ai_models(config: AiConfig) -> CmdResult<Vec<String>> {
    let key = require_key(config.provider)?;
    tauri::async_runtime::spawn_blocking(move || ai::list_models(&config, &key))
        .await
        .map_err(|_| Error::Network("The request stopped unexpectedly.".into()))?
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftInput {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
}

/// Suggest a category for a bookmark that isn't saved yet (Add dialog).
#[tauri::command]
pub async fn suggest_category(state: State<'_, AppState>, draft: DraftInput) -> CmdResult<Suggestion> {
    let (cfg, cats) = {
        let db = state.db();
        (ai::get_config(&db)?, categories::list(&db)?)
    };
    if !cfg.enabled || cfg.mode == ai::Mode::Manual {
        return Err(Error::Validation("AI suggestions are turned off.".into()));
    }
    let parsed = urls::parse(&draft.url)?;
    let d = Draft {
        url: parsed.url.to_string(),
        domain: parsed.domain.clone(),
        title: draft.title.filter(|t| !t.trim().is_empty()).unwrap_or(parsed.domain),
        description: draft.description.filter(|t| !t.trim().is_empty()),
    };
    let key = require_key(cfg.provider)?;
    let result = tauri::async_runtime::spawn_blocking(move || ai::suggest(&cats, &cfg, &key, &d))
        .await
        .map_err(|_| Error::Network("The request stopped unexpectedly.".into()))?;
    if let Err(e) = &result {
        let _ = ai::record_error(&state.db(), &e.to_string());
    }
    result
}

#[tauri::command]
pub async fn list_ai_suggestions(state: State<'_, AppState>) -> CmdResult<Vec<StoredSuggestion>> {
    ai::list_suggestions(&state.db())
}

#[tauri::command]
pub async fn accept_ai_suggestions(state: State<'_, AppState>, ids: Vec<String>) -> CmdResult<usize> {
    ai::accept(&state.db(), &ids)
}

#[tauri::command]
pub async fn reject_ai_suggestions(state: State<'_, AppState>, ids: Vec<String>) -> CmdResult<usize> {
    ai::reject(&state.db(), &ids)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AiProgress {
    done: usize,
    total: usize,
    suggested: usize,
    applied: usize,
    failed: usize,
    finished: bool,
    error: Option<String>,
}

/// Categorise bookmarks in the background (`ids` = None: uncategorized ones).
/// Emits `ai-progress`. Returns how many bookmarks were queued.
#[tauri::command]
pub async fn categorize_bookmarks(app: AppHandle, state: State<'_, AppState>, ids: Option<Vec<String>>) -> CmdResult<usize> {
    let (cfg, targets) = {
        let db = state.db();
        let cfg = ai::get_config(&db)?;
        if !cfg.enabled {
            return Err(Error::Validation("Turn on AI in Settings › AI first.".into()));
        }
        let targets = ai::batch_targets(&db, ids.as_deref(), &cfg, BATCH_LIMIT)?;
        (cfg, targets)
    };
    if targets.is_empty() {
        return Ok(0);
    }
    let key = require_key(cfg.provider)?;
    if state.ai_worker.swap(true, Ordering::SeqCst) {
        return Err(Error::Validation("AI is already organising bookmarks.".into()));
    }
    let total = targets.len();
    let db_path = state.db_path.clone();
    std::thread::spawn(move || {
        let mut p = AiProgress { done: 0, total, suggested: 0, applied: 0, failed: 0, finished: false, error: None };
        let mut run = || -> Result<(), Error> {
            let conn = magpie_core::db::open(&db_path)?;
            let cats = categories::list(&conn)?;
            for chunk in targets.chunks(PARALLEL) {
                let (cats_r, cfg_r, key_r) = (&cats, &cfg, &key);
                let results: Vec<_> = std::thread::scope(|s| {
                    let hs: Vec<_> = chunk.iter().map(|b| s.spawn(move || ai::suggest(cats_r, cfg_r, key_r, &Draft::from(b)))).collect();
                    hs.into_iter().map(|h| h.join().unwrap_or_else(|_| Err(Error::Network("Request failed.".into())))).collect()
                });
                for (b, r) in chunk.iter().zip(results) {
                    p.done += 1;
                    match r {
                        Ok(s) => {
                            ai::store_suggestion(&conn, &b.id, &s)?;
                            if s.category_id.is_some() || s.new_category.is_some() {
                                p.suggested += 1;
                            }
                            if ai::auto_apply(&conn, &cfg, &bookmarks::get(&conn, &b.id)?, &s)? {
                                p.applied += 1;
                            }
                        }
                        Err(e) => {
                            p.failed += 1;
                            let msg = e.to_string();
                            let _ = ai::record_error(&conn, &msg);
                            p.error = Some(msg.clone());
                            // Stop early when retrying can't help (bad key, quota).
                            if msg.contains("rejected") || msg.contains("rate-limiting") || msg.contains("isn't allowed") {
                                return Ok(());
                            }
                        }
                    }
                }
                let _ = app.emit("ai-progress", p.clone());
            }
            Ok(())
        };
        if let Err(e) = run() {
            p.error = Some(e.to_string());
        }
        p.finished = true;
        app.state::<AppState>().ai_worker.store(false, Ordering::SeqCst);
        let _ = app.emit("ai-progress", p);
    });
    Ok(total)
}
