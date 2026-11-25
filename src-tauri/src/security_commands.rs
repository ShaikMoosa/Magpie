//! Settings › Security and external URL scanning. Provider keys live only in
//! Windows Credential Manager and are never returned to the UI.

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use magpie_core::Error;
use magpie_core::models::SecurityStatus;
use magpie_core::scanners::{self, Consent, ScanProvider, ScanUsage, SecurityConfig, Target, Verdict};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::commands::AppState;

type CmdResult<T> = Result<T, Error>;

const SERVICE: &str = "com.shaikmoosa.magpie.security";
/// Bookmarks per run (the rest are picked up by the next one).
const BATCH_LIMIT: usize = 1000;
const CHUNK: usize = 50;
const PARALLEL: usize = 4;
/// VirusTotal's free tier allows 4 lookups a minute, so it only joins small scans.
const VT_MAX: usize = 10;
const VT_SPACING: Duration = Duration::from_secs(16);

fn entry(p: ScanProvider) -> CmdResult<keyring::Entry> {
    keyring::Entry::new(SERVICE, p.as_str()).map_err(|_| Error::Validation("Windows Credential Manager isn't available.".into()))
}

fn read_key(p: ScanProvider) -> Option<String> {
    entry(p).ok()?.get_password().ok()
}

pub(crate) fn remove_all_keys() {
    for p in ScanProvider::ALL {
        if let Ok(e) = entry(p) {
            let _ = e.delete_credential();
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecuritySettings {
    pub config: SecurityConfig,
    pub keys: Vec<ScanProvider>,
    pub usage: ScanUsage,
    pub running: bool,
}

#[tauri::command]
pub async fn get_security_settings(state: State<'_, AppState>) -> CmdResult<SecuritySettings> {
    let keys = ScanProvider::ALL.into_iter().filter(|p| read_key(*p).is_some()).collect();
    let db = state.db();
    Ok(SecuritySettings {
        config: scanners::get_config(&db)?,
        keys,
        usage: scanners::get_usage(&db)?,
        running: state.scan_worker.load(Ordering::SeqCst),
    })
}

#[tauri::command]
pub async fn save_security_config(state: State<'_, AppState>, config: SecurityConfig) -> CmdResult<SecurityConfig> {
    scanners::save_config(&state.db(), &config)
}

/// Check a key with a harmless lookup, then store it.
#[tauri::command]
pub async fn set_scan_key(provider: ScanProvider, key: String) -> CmdResult<()> {
    let key = key.trim().to_string();
    if key.is_empty() || key.len() > 400 || key.chars().any(char::is_whitespace) {
        return Err(Error::Validation("That doesn't look like an API key.".into()));
    }
    let probe = key.clone();
    tauri::async_runtime::spawn_blocking(move || scanners::test_key(provider, &probe))
        .await
        .map_err(|_| Error::Network("The request stopped unexpectedly.".into()))??;
    entry(provider)?.set_password(&key).map_err(|_| Error::Validation("Couldn't save the key to Windows Credential Manager.".into()))
}

#[tauri::command]
pub async fn remove_scan_key(provider: ScanProvider) -> CmdResult<()> {
    match entry(provider)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err(Error::Validation("Couldn't remove the key from Windows Credential Manager.".into())),
    }
}

/// Run a user-requested scan. With consent "ask" the UI confirms first and
/// passes `confirmed`. Returns how many bookmarks were queued.
#[tauri::command]
pub async fn scan_bookmarks(app: AppHandle, ids: Option<Vec<String>>, confirmed: bool) -> CmdResult<usize> {
    let cfg = scanners::get_config(&app.state::<AppState>().db())?;
    if !cfg.enabled {
        return Err(Error::Validation("Turn on URL scanning in Settings › Security first.".into()));
    }
    if cfg.consent == Consent::Never {
        return Err(Error::Validation("Sending URLs to scanners is turned off in Settings › Security.".into()));
    }
    if cfg.consent == Consent::Ask && !confirmed {
        return Err(Error::Validation("Confirm before URLs are sent to scanners.".into()));
    }
    start(&app, ids, cfg)
}

/// Automatic scan of new bookmarks (only with consent "always").
pub fn scan_saved(app: &AppHandle, ids: Option<Vec<String>>) {
    let Ok(cfg) = scanners::get_config(&app.state::<AppState>().db()) else { return };
    if cfg.automatic() {
        let _ = start(app, ids, cfg);
    }
}

/// Startup refresh of results older than the expiry ("On save + weekly").
pub fn scan_stale_at_startup(app: &AppHandle) {
    let Ok(cfg) = scanners::get_config(&app.state::<AppState>().db()) else { return };
    if cfg.automatic() && cfg.schedule == scanners::Schedule::OnSaveWeekly {
        let _ = start(app, None, cfg);
    }
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    done: usize,
    total: usize,
    dangerous: usize,
    suspicious: usize,
    failed: usize,
    finished: bool,
    error: Option<String>,
}

fn start(app: &AppHandle, ids: Option<Vec<String>>, cfg: SecurityConfig) -> CmdResult<usize> {
    let state = app.state::<AppState>();
    let providers: Vec<(ScanProvider, String)> = cfg.providers().into_iter().filter_map(|p| read_key(p).map(|k| (p, k))).collect();
    if providers.is_empty() {
        return Err(Error::Validation("Add an API key for at least one provider in Settings › Security.".into()));
    }
    let targets = scanners::targets(&state.db(), ids.as_deref(), &cfg, BATCH_LIMIT)?;
    if targets.is_empty() {
        return Ok(0);
    }
    if state.scan_worker.swap(true, Ordering::SeqCst) {
        return Err(Error::Validation("A security scan is already running.".into()));
    }
    let total = targets.len();
    let (app, db_path) = (app.clone(), state.db_path.clone());
    std::thread::spawn(move || {
        let mut p = Progress { total, ..Default::default() };
        if let Err(e) = run(&app, &db_path, &cfg, providers, &targets, &mut p) {
            p.error = Some(e.to_string());
        }
        p.finished = true;
        app.state::<AppState>().scan_worker.store(false, Ordering::SeqCst);
        let _ = app.emit("security-progress", p);
    });
    Ok(total)
}

fn run(
    app: &AppHandle,
    db_path: &std::path::Path,
    cfg: &SecurityConfig,
    mut providers: Vec<(ScanProvider, String)>,
    targets: &[Target],
    p: &mut Progress,
) -> Result<(), Error> {
    let conn = magpie_core::db::open(db_path)?;
    if targets.len() > VT_MAX {
        providers.retain(|(pr, _)| *pr != ScanProvider::Virustotal);
    }
    let mut last_vt: Option<Instant> = None;
    for chunk in targets.chunks(CHUNK) {
        if providers.is_empty() {
            break;
        }
        let sent: Vec<String> = chunk.iter().map(|t| scanners::sanitize(&t.url, cfg.strip_tracking).unwrap_or_default()).collect();
        let mut results: HashMap<usize, Vec<(ScanProvider, Verdict)>> = HashMap::new();
        let mut dropped: Vec<ScanProvider> = Vec::new();

        for (provider, key) in &providers {
            let outcome: Result<Vec<(usize, Verdict)>, Error> = match provider {
                ScanProvider::Google => {
                    let urls: Vec<String> = sent.iter().filter(|u| !u.is_empty()).cloned().collect();
                    scanners::google_lookup(key, &urls)
                        .map(|m| sent.iter().enumerate().filter_map(|(i, u)| m.get(u).map(|v| (i, v.clone()))).collect())
                }
                ScanProvider::Urlhaus => {
                    let mut all = Vec::new();
                    let mut err = None;
                    for (n, group) in sent.chunks(PARALLEL).enumerate() {
                        let out: Vec<Result<Verdict, Error>> = std::thread::scope(|s| {
                            let hs: Vec<_> = group.iter().map(|u| s.spawn(move || scanners::urlhaus_lookup(key, u))).collect();
                            hs.into_iter().map(|h| h.join().unwrap_or_else(|_| Err(Error::Network("Request failed.".into())))).collect()
                        });
                        for (j, r) in out.into_iter().enumerate() {
                            match r {
                                Ok(v) => all.push((n * PARALLEL + j, v)),
                                Err(e) => err = Some(e),
                            }
                        }
                        if err.is_some() && all.is_empty() {
                            break;
                        }
                    }
                    match err {
                        Some(e) if all.is_empty() => Err(e),
                        _ => Ok(all),
                    }
                }
                ScanProvider::Virustotal => {
                    let mut all = Vec::new();
                    let mut err = None;
                    for (i, u) in sent.iter().enumerate() {
                        if let Some(t) = last_vt {
                            std::thread::sleep(VT_SPACING.saturating_sub(t.elapsed()));
                        }
                        last_vt = Some(Instant::now());
                        match scanners::virustotal_lookup(key, u) {
                            Ok(v) => all.push((i, v)),
                            Err(e) => {
                                err = Some(e);
                                break;
                            }
                        }
                    }
                    match err {
                        Some(e) if all.is_empty() => Err(e),
                        _ => Ok(all),
                    }
                }
            };
            match outcome {
                Ok(list) => {
                    let _ = scanners::bump_usage(&conn, *provider, list.len() as u64);
                    for (i, v) in list {
                        results.entry(i).or_default().push((*provider, v));
                    }
                }
                Err(e) => {
                    // Don't keep hammering a provider that rejected the key or is rate-limiting.
                    let msg = e.to_string();
                    let _ = scanners::record_error(&conn, &msg);
                    p.error = Some(msg);
                    dropped.push(*provider);
                }
            }
        }
        providers.retain(|(pr, _)| !dropped.contains(pr));

        for (i, t) in chunk.iter().enumerate() {
            p.done += 1;
            // No provider answered: leave the existing (local) result alone.
            let Some(res) = results.get(&i) else {
                p.failed += 1;
                continue;
            };
            let combined = scanners::combine(&scanners::local(t), res);
            scanners::store(&conn, &t.id, &combined, res)?;
            match combined.status {
                SecurityStatus::Dangerous => p.dangerous += 1,
                SecurityStatus::Suspicious => p.suspicious += 1,
                _ => {}
            }
        }
        let _ = app.emit("security-progress", p.clone());
    }
    Ok(())
}
