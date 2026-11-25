//! External threat-intelligence lookups (Google Safe Browsing, URLhaus,
//! VirusTotal) combined with local heuristics.
//!
//! Rules from the PRD: a failed or unavailable lookup is never a "safe"
//! result, URLs are only sent to providers the user enabled, and fragments,
//! login details and (optionally) tracking or secret-looking query parameters
//! are removed before anything leaves the PC.

use std::collections::HashMap;
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::db::{new_id, now};
use crate::error::{Error, Result};
use crate::models::SecurityStatus;
use crate::security::{self, Assessment};

const TIMEOUT: Duration = Duration::from_secs(20);
const REASON_SEP: char = '\u{1f}';

// ------------------------------------------------------------------ config

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScanProvider {
    Google,
    Urlhaus,
    Virustotal,
}

impl ScanProvider {
    pub const ALL: [ScanProvider; 3] = [ScanProvider::Google, ScanProvider::Urlhaus, ScanProvider::Virustotal];
    pub fn as_str(self) -> &'static str {
        match self {
            ScanProvider::Google => "google",
            ScanProvider::Urlhaus => "urlhaus",
            ScanProvider::Virustotal => "virustotal",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            ScanProvider::Google => "Google Safe Browsing",
            ScanProvider::Urlhaus => "URLhaus",
            ScanProvider::Virustotal => "VirusTotal",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Schedule {
    /// Only when the user runs a scan.
    Manual,
    OnSave,
    /// New bookmarks, plus results older than `expiry_days` refreshed at startup.
    #[default]
    OnSaveWeekly,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Consent {
    /// Nothing is sent automatically; the user confirms each scan.
    #[default]
    Ask,
    Always,
    Never,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SecurityConfig {
    /// External scanning. Local checks always run.
    pub enabled: bool,
    pub google: bool,
    pub urlhaus: bool,
    pub virustotal: bool,
    pub schedule: Schedule,
    pub consent: Consent,
    pub strip_tracking: bool,
    pub warn_suspicious: bool,
    pub block_dangerous: bool,
    pub expiry_days: u32,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            google: false,
            urlhaus: false,
            virustotal: false,
            schedule: Schedule::OnSaveWeekly,
            consent: Consent::Ask,
            strip_tracking: true,
            warn_suspicious: true,
            block_dangerous: true,
            expiry_days: 7,
        }
    }
}

impl SecurityConfig {
    pub fn providers(&self) -> Vec<ScanProvider> {
        ScanProvider::ALL
            .into_iter()
            .filter(|p| match p {
                ScanProvider::Google => self.google,
                ScanProvider::Urlhaus => self.urlhaus,
                ScanProvider::Virustotal => self.virustotal,
            })
            .collect()
    }
    /// May URLs be sent without the user confirming this scan?
    pub fn automatic(&self) -> bool {
        self.enabled && self.consent == Consent::Always && self.schedule != Schedule::Manual
    }
    /// May URLs be sent at all (after confirmation when consent is "ask")?
    pub fn external_allowed(&self) -> bool {
        self.enabled && self.consent != Consent::Never && !self.providers().is_empty()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScanUsage {
    /// "2026-11": counters reset each month.
    pub month: String,
    pub google: u64,
    pub urlhaus: u64,
    pub virustotal: u64,
    pub last_scan_at: Option<String>,
    pub last_error: Option<String>,
}

fn get_setting<T: for<'de> Deserialize<'de> + Default>(conn: &Connection, key: &str) -> Result<T> {
    let raw: Option<String> =
        conn.query_row("SELECT value FROM application_settings WHERE key = ?1", [key], |r| r.get(0)).optional()?;
    Ok(raw.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default())
}

fn put_setting<T: Serialize>(conn: &Connection, key: &str, value: &T) -> Result<()> {
    let json = serde_json::to_string(value).map_err(|_| Error::Validation("Couldn't save settings.".into()))?;
    conn.execute(
        "INSERT INTO application_settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, json],
    )?;
    Ok(())
}

pub fn get_config(conn: &Connection) -> Result<SecurityConfig> {
    get_setting(conn, "security_config")
}

pub fn save_config(conn: &Connection, cfg: &SecurityConfig) -> Result<SecurityConfig> {
    let mut cfg = cfg.clone();
    cfg.expiry_days = cfg.expiry_days.clamp(1, 90);
    put_setting(conn, "security_config", &cfg)?;
    Ok(cfg)
}

fn this_month() -> String {
    chrono::Local::now().format("%Y-%m").to_string()
}

pub fn get_usage(conn: &Connection) -> Result<ScanUsage> {
    let mut u: ScanUsage = get_setting(conn, "security_usage")?;
    if u.month != this_month() {
        u = ScanUsage { month: this_month(), last_scan_at: u.last_scan_at, last_error: u.last_error, ..Default::default() };
    }
    Ok(u)
}

pub fn bump_usage(conn: &Connection, provider: ScanProvider, n: u64) -> Result<()> {
    let mut u = get_usage(conn)?;
    match provider {
        ScanProvider::Google => u.google += n,
        ScanProvider::Urlhaus => u.urlhaus += n,
        ScanProvider::Virustotal => u.virustotal += n,
    }
    u.last_scan_at = Some(now());
    put_setting(conn, "security_usage", &u)
}

pub fn record_error(conn: &Connection, msg: &str) -> Result<()> {
    let mut u = get_usage(conn)?;
    u.last_error = Some(msg.chars().take(300).collect());
    put_setting(conn, "security_usage", &u)
}

// ------------------------------------------------------------------ privacy

const TRACKING: &[&str] = &["fbclid", "gclid", "dclid", "msclkid", "mc_cid", "mc_eid", "igshid", "yclid", "_ga", "_gl", "ref_src", "spm"];
const SECRETS: &[&str] = &[
    "token", "access_token", "id_token", "refresh_token", "auth", "authorization", "key", "api_key", "apikey", "password",
    "pass", "pwd", "secret", "session", "sessionid", "sid", "sig", "signature", "code", "otp", "email", "jwt",
];

/// The URL as it will be sent to a provider. Login details and fragments are
/// always removed; with `strip` on, tracking and secret-looking parameters too.
pub fn sanitize(input: &str, strip: bool) -> Result<String> {
    let mut url = url::Url::parse(input).map_err(|_| Error::InvalidUrl("That address can't be checked.".into()))?;
    let _ = url.set_username("");
    let _ = url.set_password(None);
    url.set_fragment(None);
    if strip && url.query().is_some() {
        let kept: Vec<(String, String)> = url
            .query_pairs()
            .filter(|(k, _)| {
                let k = k.to_ascii_lowercase();
                !k.starts_with("utm_") && !TRACKING.contains(&k.as_str()) && !SECRETS.contains(&k.as_str())
            })
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        if kept.is_empty() {
            url.set_query(None);
        } else {
            url.query_pairs_mut().clear().extend_pairs(kept);
        }
    }
    Ok(url.to_string())
}

// ------------------------------------------------------------------ verdicts

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "detail", rename_all = "lowercase")]
pub enum Verdict {
    Malicious(String),
    Suspicious(String),
    Clean,
    /// The provider has no data on this URL. Not a clean result.
    Unknown(String),
}

fn client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(TIMEOUT)
        .user_agent("Magpie/0.1")
        .build()
        .map_err(|_| Error::Network("Couldn't start a network request.".into()))
}

/// Friendly error that never echoes the request (Google puts the key in the URL).
fn http_error(provider: ScanProvider, status: u16) -> Error {
    let p = provider.label();
    Error::Network(match status {
        400 => format!("{p} rejected the request."),
        401 | 403 => format!("{p} rejected the API key. Check it in Settings › Security."),
        429 => format!("{p} is rate-limiting requests or the daily quota is used up. Try again later."),
        500..=599 => format!("{p} is having problems right now. Try again later."),
        _ => format!("{p} returned error {status}."),
    })
}

fn send(provider: ScanProvider, req: reqwest::blocking::RequestBuilder) -> Result<(u16, String)> {
    let resp = req.send().map_err(|e| {
        Error::Network(if e.is_timeout() {
            format!("{} took too long to answer.", provider.label())
        } else {
            format!("Couldn't reach {}.", provider.label())
        })
    })?;
    let status = resp.status().as_u16();
    Ok((status, resp.text().unwrap_or_default()))
}

fn unreadable(provider: ScanProvider) -> Error {
    Error::Network(format!("{} sent an unreadable response.", provider.label()))
}

// --- Google Safe Browsing (v4 Lookup API) ---

fn google_threat(t: &str) -> &'static str {
    match t {
        "MALWARE" => "malware",
        "SOCIAL_ENGINEERING" => "phishing or a deceptive site",
        "UNWANTED_SOFTWARE" => "unwanted software",
        "POTENTIALLY_HARMFUL_APPLICATION" => "a harmful app",
        _ => "a known threat",
    }
}

/// Matches by URL. URLs not in the map were checked and not listed.
pub fn parse_google(body: &str) -> Result<HashMap<String, String>> {
    let v: Value = serde_json::from_str(body).map_err(|_| unreadable(ScanProvider::Google))?;
    let mut out = HashMap::new();
    for m in v.get("matches").and_then(Value::as_array).into_iter().flatten() {
        if let Some(url) = m.pointer("/threat/url").and_then(Value::as_str) {
            let t = google_threat(m.get("threatType").and_then(Value::as_str).unwrap_or(""));
            out.entry(url.to_string()).or_insert_with(|| format!("Google Safe Browsing lists this as {t}"));
        }
    }
    Ok(out)
}

pub fn google_lookup(key: &str, urls: &[String]) -> Result<HashMap<String, Verdict>> {
    let mut out = HashMap::new();
    for chunk in urls.chunks(500) {
        let body = json!({
            "client": { "clientId": "magpie", "clientVersion": env!("CARGO_PKG_VERSION") },
            "threatInfo": {
                "threatTypes": ["MALWARE", "SOCIAL_ENGINEERING", "UNWANTED_SOFTWARE", "POTENTIALLY_HARMFUL_APPLICATION"],
                "platformTypes": ["ANY_PLATFORM"],
                "threatEntryTypes": ["URL"],
                "threatEntries": chunk.iter().map(|u| json!({ "url": u })).collect::<Vec<_>>(),
            }
        });
        let req = client()?
            .post("https://safebrowsing.googleapis.com/v4/threatMatches:find")
            .query(&[("key", key)])
            .json(&body);
        let (status, text) = send(ScanProvider::Google, req)?;
        if status != 200 {
            return Err(http_error(ScanProvider::Google, status));
        }
        let matches = parse_google(&text)?;
        for u in chunk {
            out.insert(u.clone(), matches.get(u).map(|r| Verdict::Malicious(r.clone())).unwrap_or(Verdict::Clean));
        }
    }
    Ok(out)
}

// --- URLhaus (abuse.ch) ---

pub fn parse_urlhaus(body: &str) -> Result<Verdict> {
    let v: Value = serde_json::from_str(body).map_err(|_| unreadable(ScanProvider::Urlhaus))?;
    let threat = v.get("threat").and_then(Value::as_str).unwrap_or("malware").replace('_', " ");
    Ok(match v.get("query_status").and_then(Value::as_str) {
        Some("no_results") => Verdict::Clean,
        Some("ok") => match v.get("url_status").and_then(Value::as_str) {
            Some("online") => Verdict::Malicious(format!("URLhaus lists this as an active {threat} site")),
            _ => Verdict::Suspicious(format!("URLhaus listed this as a {threat} site (now offline)")),
        },
        Some("invalid_url") => Verdict::Unknown("URLhaus couldn't check this address".into()),
        _ => return Err(unreadable(ScanProvider::Urlhaus)),
    })
}

pub fn urlhaus_lookup(key: &str, url: &str) -> Result<Verdict> {
    let req = client()?.post("https://urlhaus-api.abuse.ch/v1/url/").header("Auth-Key", key).form(&[("url", url)]);
    let (status, text) = send(ScanProvider::Urlhaus, req)?;
    if status != 200 {
        return Err(http_error(ScanProvider::Urlhaus, status));
    }
    parse_urlhaus(&text)
}

// --- VirusTotal (v3, lookup only: URLs are never submitted for analysis) ---

/// URL identifier: unpadded URL-safe base64 of the URL.
pub fn vt_id(url: &str) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let b = url.as_bytes();
    let mut out = String::with_capacity(b.len().div_ceil(3) * 4);
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..=c.len() {
            out.push(A[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}

pub fn parse_virustotal(body: &str) -> Result<Verdict> {
    let v: Value = serde_json::from_str(body).map_err(|_| unreadable(ScanProvider::Virustotal))?;
    let stats = v.pointer("/data/attributes/last_analysis_stats").ok_or_else(|| unreadable(ScanProvider::Virustotal))?;
    let n = |k: &str| stats.get(k).and_then(Value::as_u64).unwrap_or(0);
    let (mal, sus) = (n("malicious"), n("suspicious"));
    let total = mal + sus + n("harmless") + n("undetected");
    Ok(if mal >= 2 {
        Verdict::Malicious(format!("{mal} of {total} VirusTotal engines flag this site"))
    } else if mal == 1 || sus >= 2 {
        Verdict::Suspicious(format!("{} of {total} VirusTotal engines flag this site", mal + sus))
    } else if total == 0 {
        Verdict::Unknown("VirusTotal has no analysis for this address".into())
    } else {
        Verdict::Clean
    })
}

pub fn virustotal_lookup(key: &str, url: &str) -> Result<Verdict> {
    let req = client()?.get(format!("https://www.virustotal.com/api/v3/urls/{}", vt_id(url))).header("x-apikey", key);
    let (status, text) = send(ScanProvider::Virustotal, req)?;
    match status {
        200 => parse_virustotal(&text),
        404 => Ok(Verdict::Unknown("VirusTotal hasn't seen this address".into())),
        s => Err(http_error(ScanProvider::Virustotal, s)),
    }
}

/// Check a key with a harmless lookup.
pub fn test_key(provider: ScanProvider, key: &str) -> Result<()> {
    let probe = "https://example.com/".to_string();
    match provider {
        ScanProvider::Google => google_lookup(key, std::slice::from_ref(&probe)).map(|_| ()),
        ScanProvider::Urlhaus => urlhaus_lookup(key, &probe).map(|_| ()),
        ScanProvider::Virustotal => virustotal_lookup(key, &probe).map(|_| ()),
    }
}

// ------------------------------------------------------------------ combine & store

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Combined {
    pub status: SecurityStatus,
    pub reasons: Vec<String>,
    pub source: String,
}

/// Merge local heuristics with provider verdicts. Only a provider's clean
/// answer can produce "no threat"; failures and unknowns never do.
pub fn combine(local: &Assessment, results: &[(ScanProvider, Verdict)]) -> Combined {
    let mut reasons = Vec::new();
    let mut malicious = false;
    let mut suspicious = matches!(local.status, SecurityStatus::Suspicious | SecurityStatus::Dangerous);
    let mut clean = false;
    let mut sources = Vec::new();
    for (p, v) in results {
        match v {
            Verdict::Malicious(r) => {
                malicious = true;
                reasons.push(r.clone());
            }
            Verdict::Suspicious(r) => {
                suspicious = true;
                reasons.push(r.clone());
            }
            Verdict::Clean => clean = true,
            Verdict::Unknown(_) => {}
        }
        if !matches!(v, Verdict::Unknown(_)) {
            sources.push(p.label());
        }
    }
    reasons.extend(local.reasons.iter().cloned());
    sources.push(security::LOCAL_SOURCE);
    let status = if malicious {
        SecurityStatus::Dangerous
    } else if suspicious {
        SecurityStatus::Suspicious
    } else if clean {
        SecurityStatus::NoThreat
    } else {
        local.status
    };
    Combined { status, reasons, source: sources.join(" · ") }
}

/// Save a combined result on the bookmark and in scan history.
pub fn store(conn: &Connection, bookmark_id: &str, combined: &Combined, results: &[(ScanProvider, Verdict)]) -> Result<()> {
    let ts = now();
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "UPDATE bookmarks SET security_status = ?2, security_reasons = ?3, security_source = ?4, security_checked_at = ?5 WHERE id = ?1",
        params![bookmark_id, combined.status.as_str(), combined.reasons.join(&REASON_SEP.to_string()), combined.source, ts],
    )?;
    for (p, v) in results {
        let (status, detail) = match v {
            Verdict::Malicious(d) => ("dangerous", d.as_str()),
            Verdict::Suspicious(d) => ("suspicious", d.as_str()),
            Verdict::Clean => ("no_threat", ""),
            Verdict::Unknown(d) => ("not_checked", d.as_str()),
        };
        tx.execute(
            "INSERT INTO security_scans (id, bookmark_id, provider, status, details, scanned_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![new_id(), bookmark_id, p.as_str(), status, detail, ts],
        )?;
    }
    tx.commit()?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Target {
    pub id: String,
    pub url: String,
    pub domain: String,
}

/// Bookmarks to send for an external scan. With `ids`, those bookmarks;
/// otherwise live bookmarks with no external result newer than the expiry.
/// Domains the user marked as safe are never sent.
pub fn targets(conn: &Connection, ids: Option<&[String]>, cfg: &SecurityConfig, limit: usize) -> Result<Vec<Target>> {
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(cfg.expiry_days as i64)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let mut stmt = conn.prepare(
        "SELECT b.id, b.url, b.domain,
            EXISTS(SELECT 1 FROM security_scans s WHERE s.bookmark_id = b.id AND s.provider IN ('google','urlhaus','virustotal') AND s.scanned_at >= ?1)
         FROM bookmarks b
         WHERE b.deleted_at IS NULL AND b.domain NOT IN (SELECT domain FROM security_overrides)
         ORDER BY b.created_at DESC",
    )?;
    let rows = stmt.query_map([cutoff], |r| Ok((Target { id: r.get(0)?, url: r.get(1)?, domain: r.get(2)? }, r.get::<_, bool>(3)?)))?;
    let mut out = Vec::new();
    for row in rows {
        let (t, fresh) = row?;
        let wanted = match ids {
            Some(ids) => ids.contains(&t.id),
            None => !fresh,
        };
        if wanted {
            out.push(t);
            if out.len() >= limit {
                break;
            }
        }
    }
    Ok(out)
}

/// The most recent external results for a bookmark (one scan's worth).
pub fn latest_results(conn: &Connection, bookmark_id: &str) -> Result<Vec<(ScanProvider, Verdict)>> {
    let mut stmt = conn.prepare(
        "SELECT provider, status, COALESCE(details, '') FROM security_scans
         WHERE bookmark_id = ?1 AND provider IN ('google','urlhaus','virustotal')
           AND scanned_at = (SELECT MAX(scanned_at) FROM security_scans
                             WHERE bookmark_id = ?1 AND provider IN ('google','urlhaus','virustotal'))",
    )?;
    let rows = stmt.query_map([bookmark_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?;
    let mut out = Vec::new();
    for row in rows {
        let (p, status, detail) = row?;
        let Some(p) = ScanProvider::ALL.into_iter().find(|x| x.as_str() == p) else { continue };
        let v = match status.as_str() {
            "dangerous" => Verdict::Malicious(detail),
            "suspicious" => Verdict::Suspicious(detail),
            "no_threat" => Verdict::Clean,
            _ => Verdict::Unknown(detail),
        };
        out.push((p, v));
    }
    Ok(out)
}

/// Local heuristics for a target (re-run so the combined reasons are current).
pub fn local(target: &Target) -> Assessment {
    url::Url::parse(&target.url)
        .map(|u| security::assess(&u))
        .unwrap_or(Assessment { status: SecurityStatus::NotChecked, reasons: vec![] })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bookmarks;
    use crate::db::open_in_memory;
    use crate::models::NewBookmark;

    #[test]
    fn sanitize_removes_private_parts() {
        let s = sanitize("https://user:pw@shop.example/p?id=7&utm_source=x&token=abc&fbclid=1#frag", true).unwrap();
        assert_eq!(s, "https://shop.example/p?id=7");
        let s = sanitize("https://a.example/?token=abc#x", false).unwrap();
        assert_eq!(s, "https://a.example/?token=abc");
        assert_eq!(sanitize("https://a.example/?utm_medium=x", true).unwrap(), "https://a.example/");
    }

    #[test]
    fn google_parsing() {
        assert!(parse_google("{}").unwrap().is_empty());
        let m = parse_google(r#"{"matches":[{"threatType":"SOCIAL_ENGINEERING","threat":{"url":"http://bad.example/"}}]}"#).unwrap();
        assert!(m["http://bad.example/"].contains("phishing"));
        assert!(parse_google("<html>").is_err());
    }

    #[test]
    fn urlhaus_parsing() {
        assert_eq!(parse_urlhaus(r#"{"query_status":"no_results"}"#).unwrap(), Verdict::Clean);
        assert!(matches!(parse_urlhaus(r#"{"query_status":"ok","url_status":"online","threat":"malware_download"}"#).unwrap(), Verdict::Malicious(r) if r.contains("malware download")));
        assert!(matches!(parse_urlhaus(r#"{"query_status":"ok","url_status":"offline"}"#).unwrap(), Verdict::Suspicious(_)));
        assert!(parse_urlhaus(r#"{"query_status":"unknown_auth_key"}"#).is_err(), "errors are never clean");
    }

    #[test]
    fn virustotal_parsing_and_id() {
        assert_eq!(vt_id("http://example.com/"), "aHR0cDovL2V4YW1wbGUuY29tLw");
        assert_eq!(vt_id("ab"), "YWI");
        let body = |m: u64, s: u64| format!(r#"{{"data":{{"attributes":{{"last_analysis_stats":{{"malicious":{m},"suspicious":{s},"harmless":60,"undetected":10}}}}}}}}"#);
        assert!(matches!(parse_virustotal(&body(3, 0)).unwrap(), Verdict::Malicious(_)));
        assert!(matches!(parse_virustotal(&body(1, 0)).unwrap(), Verdict::Suspicious(_)));
        assert_eq!(parse_virustotal(&body(0, 1)).unwrap(), Verdict::Clean);
    }

    #[test]
    fn combine_rules() {
        let local_ok = Assessment { status: SecurityStatus::NotChecked, reasons: vec![] };
        let local_http = Assessment { status: SecurityStatus::NotChecked, reasons: vec!["Uses an insecure HTTP connection".into()] };
        let local_sus = Assessment { status: SecurityStatus::Suspicious, reasons: vec!["Looks like paypal".into()] };

        // Nothing usable from providers: never "no threat".
        let c = combine(&local_ok, &[(ScanProvider::Virustotal, Verdict::Unknown("x".into()))]);
        assert_eq!(c.status, SecurityStatus::NotChecked);
        assert_eq!(c.source, "Local heuristics");

        let c = combine(&local_http, &[(ScanProvider::Google, Verdict::Clean), (ScanProvider::Urlhaus, Verdict::Clean)]);
        assert_eq!(c.status, SecurityStatus::NoThreat);
        assert_eq!(c.source, "Google Safe Browsing · URLhaus · Local heuristics");
        assert_eq!(c.reasons, vec!["Uses an insecure HTTP connection"]);

        // A clean provider answer doesn't hide local warnings.
        assert_eq!(combine(&local_sus, &[(ScanProvider::Google, Verdict::Clean)]).status, SecurityStatus::Suspicious);

        let c = combine(&local_ok, &[(ScanProvider::Google, Verdict::Clean), (ScanProvider::Urlhaus, Verdict::Malicious("bad".into()))]);
        assert_eq!(c.status, SecurityStatus::Dangerous);
        assert_eq!(c.reasons[0], "bad");
    }

    #[test]
    fn store_targets_and_overrides() {
        let c = open_in_memory().unwrap();
        let a = bookmarks::create(&c, NewBookmark { url: "https://a.example".into(), ..Default::default() }).unwrap();
        let b = bookmarks::create(&c, NewBookmark { url: "https://b.example".into(), ..Default::default() }).unwrap();
        let cfg = SecurityConfig::default();
        assert_eq!(targets(&c, None, &cfg, 10).unwrap().len(), 2);

        let results = vec![(ScanProvider::Google, Verdict::Malicious("bad".into()))];
        store(&c, &a.id, &combine(&Assessment { status: SecurityStatus::NotChecked, reasons: vec![] }, &results), &results).unwrap();
        let a2 = bookmarks::get(&c, &a.id).unwrap();
        assert_eq!(a2.security_status, SecurityStatus::Dangerous);
        assert_eq!(a2.security_source.as_deref(), Some("Google Safe Browsing · Local heuristics"));
        let left = targets(&c, None, &cfg, 10).unwrap();
        assert_eq!(left.len(), 1, "fresh results aren't rescanned");
        assert_eq!(left[0].id, b.id);

        bookmarks::mark_safe(&c, std::slice::from_ref(&b.id)).unwrap();
        assert!(targets(&c, None, &cfg, 10).unwrap().is_empty(), "trusted domains are never sent");
        assert_eq!(targets(&c, Some(std::slice::from_ref(&a.id)), &cfg, 10).unwrap().len(), 1);
    }

    #[test]
    fn local_rescan_keeps_external_verdicts() {
        let c = open_in_memory().unwrap();
        let a = bookmarks::create(&c, NewBookmark { url: "https://a.example".into(), ..Default::default() }).unwrap();
        let results = vec![(ScanProvider::Urlhaus, Verdict::Malicious("bad".into()))];
        store(&c, &a.id, &combine(&local(&Target { id: a.id.clone(), url: a.url.clone(), domain: a.domain.clone() }), &results), &results).unwrap();
        bookmarks::rescan(&c, None).unwrap();
        assert_eq!(bookmarks::get(&c, &a.id).unwrap().security_status, SecurityStatus::Dangerous);
        // Trusting the site overrides it.
        bookmarks::mark_safe(&c, std::slice::from_ref(&a.id)).unwrap();
        assert_eq!(bookmarks::get(&c, &a.id).unwrap().security_status, SecurityStatus::NotChecked);
    }

    #[test]
    fn config_and_usage() {
        let c = open_in_memory().unwrap();
        let cfg = get_config(&c).unwrap();
        assert!(!cfg.enabled && !cfg.automatic() && !cfg.external_allowed());
        let on = SecurityConfig { enabled: true, google: true, consent: Consent::Always, ..cfg };
        assert!(on.automatic());
        assert!(!SecurityConfig { consent: Consent::Never, ..on.clone() }.external_allowed());
        save_config(&c, &on).unwrap();
        bump_usage(&c, ScanProvider::Google, 3).unwrap();
        assert_eq!(get_usage(&c).unwrap().google, 3);
    }

    #[test]
    fn errors_never_contain_request_details() {
        let e = http_error(ScanProvider::Google, 403).to_string();
        assert!(e.contains("Google Safe Browsing") && !e.contains("key="));
    }
}
