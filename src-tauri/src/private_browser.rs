//! "Open in private window": find the default browser for https links and
//! start it with its private-browsing flag. The URL is passed as a separate
//! argument (no shell), and has already been validated as http(s).

use std::path::PathBuf;
use std::process::Command;

use magpie_core::Error;

/// Private-browsing flag for a browser ProgId (or exe name).
fn private_flag(prog_id: &str, exe: &str) -> Option<&'static str> {
    let id = prog_id.to_ascii_lowercase();
    let exe = exe.to_ascii_lowercase();
    let has = |s: &str| id.contains(s) || exe.ends_with(&format!("{s}.exe"));
    if id.starts_with("msedgehtm") || has("msedge") {
        Some("--inprivate")
    } else if id.starts_with("firefoxurl") || has("firefox") {
        Some("-private-window")
    } else if id.starts_with("chromehtml") || id.starts_with("bravehtml") || id.starts_with("vivaldihtm") || has("chrome") || has("brave") || has("vivaldi") {
        Some("--incognito")
    } else if id.starts_with("opera") || has("opera") {
        Some("--private")
    } else {
        None
    }
}

/// The executable path from a registry command like `"C:\…\chrome.exe" --single-argument %1`.
fn exe_from_command(cmd: &str) -> Option<PathBuf> {
    let cmd = cmd.trim();
    let path = if let Some(rest) = cmd.strip_prefix('"') {
        rest.split('"').next()?
    } else {
        let lower = cmd.to_ascii_lowercase();
        let end = lower.find(".exe")? + 4;
        &cmd[..end]
    };
    let p = PathBuf::from(path);
    p.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")).then_some(p)
}

#[cfg(windows)]
fn default_browser() -> Result<(String, PathBuf), Error> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CLASSES_ROOT, HKEY_CURRENT_USER};
    let unknown = || Error::Validation("Couldn't find your default browser.".into());
    let choice = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Microsoft\Windows\Shell\Associations\UrlAssociations\https\UserChoice")
        .map_err(|_| unknown())?;
    let prog_id: String = choice.get_value("ProgId").map_err(|_| unknown())?;
    let cmd: String = RegKey::predef(HKEY_CLASSES_ROOT)
        .open_subkey(format!(r"{prog_id}\shell\open\command"))
        .and_then(|k| k.get_value(""))
        .map_err(|_| unknown())?;
    let exe = exe_from_command(&cmd).ok_or_else(unknown)?;
    Ok((prog_id, exe))
}

#[cfg(not(windows))]
fn default_browser() -> Result<(String, PathBuf), Error> {
    Err(Error::Validation("Private windows are only supported on Windows.".into()))
}

pub fn open(url: &str) -> Result<(), Error> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(Error::Validation("Only web addresses can be opened.".into()));
    }
    let (prog_id, exe) = default_browser()?;
    let flag = private_flag(&prog_id, &exe.to_string_lossy())
        .ok_or_else(|| Error::Validation("Your default browser can't be opened in a private window from Magpie.".into()))?;
    Command::new(&exe)
        .arg(flag)
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|_| Error::Validation("Couldn't start your browser.".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_for_common_browsers() {
        assert_eq!(private_flag("MSEdgeHTM", r"C:\x\msedge.exe"), Some("--inprivate"));
        assert_eq!(private_flag("ChromeHTML", r"C:\x\chrome.exe"), Some("--incognito"));
        assert_eq!(private_flag("FirefoxURL-308046B0AF4A39CB", r"C:\x\firefox.exe"), Some("-private-window"));
        assert_eq!(private_flag("BraveHTML", ""), Some("--incognito"));
        assert_eq!(private_flag("SomethingElse", r"C:\x\other.exe"), None);
    }

    #[test]
    fn exe_parsing() {
        assert_eq!(exe_from_command(r#""C:\Program Files\Google\Chrome\Application\chrome.exe" --single-argument %1"#), Some(PathBuf::from(r"C:\Program Files\Google\Chrome\Application\chrome.exe")));
        assert_eq!(exe_from_command(r"C:\Apps\firefox.exe -osint -url %1"), Some(PathBuf::from(r"C:\Apps\firefox.exe")));
        assert_eq!(exe_from_command("rundll32 url.dll"), None);
    }

    #[test]
    fn refuses_non_web_urls() {
        assert!(open("file:///C:/Windows/System32/calc.exe").is_err());
        assert!(open("--evil").is_err());
    }
}
