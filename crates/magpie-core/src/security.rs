//! Local URL heuristics. These never mark a URL as safe: without an external
//! threat-intelligence result the best outcome is `NotChecked`.

use std::net::IpAddr;

use url::{Host, Url};

use crate::models::SecurityStatus;

pub const LOCAL_SOURCE: &str = "Local heuristics";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assessment {
    pub status: SecurityStatus,
    /// Human-readable findings. May be non-empty even when not suspicious
    /// (e.g. "Uses an insecure HTTP connection").
    pub reasons: Vec<String>,
}

const BRANDS: &[&str] = &[
    "paypal", "google", "microsoft", "apple", "amazon", "facebook", "instagram", "netflix",
    "github", "chase", "wellsfargo", "bankofamerica", "outlook", "office365", "linkedin",
    "dropbox", "whatsapp", "binance", "coinbase", "icloud", "steam", "adobe", "docusign",
];

const ABUSED_TLDS: &[&str] = &[
    "zip", "mov", "top", "tk", "ml", "ga", "cf", "gq", "click", "country", "kim", "work", "rest",
    "cam", "icu",
];

const EXECUTABLE_EXT: &[&str] = &[".exe", ".msi", ".scr", ".bat", ".cmd", ".apk", ".jar", ".vbs", ".ps1"];

const SECOND_LEVEL_SUFFIXES: &[&str] = &["co", "com", "org", "net", "ac", "gov", "edu"];

pub fn assess(url: &Url) -> Assessment {
    let mut strong: Vec<String> = Vec::new();
    let mut weak: Vec<String> = Vec::new();

    if !url.username().is_empty() || url.password().is_some() {
        strong.push("Contains login details inside the address".into());
    }

    match url.host() {
        Some(Host::Ipv4(ip)) => check_ip(IpAddr::V4(ip), &mut strong),
        Some(Host::Ipv6(ip)) => check_ip(IpAddr::V6(ip), &mut strong),
        Some(Host::Domain(host)) => check_domain(&host.to_ascii_lowercase(), &mut strong, &mut weak),
        None => {}
    }

    if url.scheme() == "http" {
        weak.push("Uses an insecure HTTP connection".into());
    }

    let path = url.path().to_ascii_lowercase();
    if EXECUTABLE_EXT.iter().any(|ext| path.ends_with(ext)) {
        strong.push("Links directly to a program download".into());
    }

    if url.as_str().len() > 200 {
        weak.push("Unusually long address".into());
    }

    let status = if !strong.is_empty() || weak.len() >= 2 {
        SecurityStatus::Suspicious
    } else {
        SecurityStatus::NotChecked
    };
    strong.extend(weak);
    Assessment { status, reasons: strong }
}

fn check_ip(ip: IpAddr, strong: &mut Vec<String>) {
    let private = match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => v6.is_loopback(),
    };
    if !private {
        strong.push("Uses a raw IP address instead of a domain name".into());
    }
}

fn check_domain(host: &str, strong: &mut Vec<String>, weak: &mut Vec<String>) {
    if host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local") {
        return;
    }
    let labels: Vec<&str> = host.split('.').collect();

    if labels.iter().any(|l| l.starts_with("xn--")) {
        strong.push("Uses look-alike international characters (punycode)".into());
    }

    let (registrable_label, tld) = registrable(&labels);
    if let Some(tld) = tld
        && ABUSED_TLDS.contains(&tld)
    {
        weak.push(format!("Uses the .{tld} domain ending, which is often abused"));
    }

    if labels.len() > 5 {
        weak.push("Has an unusually deep chain of subdomains".into());
    }
    if registrable_label.matches('-').count() >= 3 {
        weak.push("Domain name is made of many hyphenated words".into());
    }

    // Look-alike brand checks.
    let deconfused = deconfuse(registrable_label);
    for brand in BRANDS {
        if registrable_label == *brand {
            continue;
        }
        let digit_trick = deconfused != registrable_label && deconfused.contains(brand);
        let as_token = registrable_label.split('-').any(|t| t == *brand);
        if digit_trick || as_token {
            strong.push(format!("Looks like {brand} but is a different site"));
            break;
        }
        let in_subdomain = labels
            .iter()
            .take(labels.len().saturating_sub(2))
            .any(|l| deconfuse(l).split('-').any(|t| t == *brand));
        if in_subdomain {
            strong.push(format!("Mentions {brand} in the address but is hosted elsewhere"));
            break;
        }
    }
}

/// Returns (registrable label, tld) — e.g. `accounts.paypal.co.uk` → ("paypal", "uk").
fn registrable<'a>(labels: &[&'a str]) -> (&'a str, Option<&'a str>) {
    match labels {
        [] => ("", None),
        [only] => (*only, None),
        [.., sld, tld] if labels.len() >= 3 && tld.len() == 2 && SECOND_LEVEL_SUFFIXES.contains(sld) => {
            (labels[labels.len() - 3], Some(*tld))
        }
        [.., label, tld] => (*label, Some(*tld)),
    }
}

fn deconfuse(s: &str) -> String {
    s.replace("rn", "m")
        .replace("vv", "w")
        .chars()
        .map(|c| match c {
            '0' => 'o',
            '1' => 'l',
            '3' => 'e',
            '4' => 'a',
            '5' => 's',
            '7' => 't',
            '8' => 'b',
            _ => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(s: &str) -> SecurityStatus {
        assess(&Url::parse(s).unwrap()).status
    }

    #[test]
    fn ordinary_sites_are_not_checked_not_safe() {
        for ok in [
            "https://figma.com/community",
            "https://docs.google.com/document/d/1",
            "https://github.com/tauri-apps/tauri",
            "https://googleblog.com",
            "https://paypal.com",
            "https://www.bbc.co.uk/news",
            "http://localhost:5173",
        ] {
            assert_eq!(status(ok), SecurityStatus::NotChecked, "{ok}");
        }
    }

    #[test]
    fn lookalikes_are_suspicious() {
        for bad in [
            "https://paypa1-secure-login.com/verify",
            "https://accounts-google.com/signin",
            "https://paypal.com.account-verify.io/login",
            "https://rnicrosoft.com",
            "https://xn--pple-43d.com",
        ] {
            assert_eq!(status(bad), SecurityStatus::Suspicious, "{bad}");
        }
    }

    #[test]
    fn other_strong_signals() {
        assert_eq!(status("https://user:pass@example.com"), SecurityStatus::Suspicious);
        assert_eq!(status("http://45.33.12.9/login"), SecurityStatus::Suspicious);
        assert_eq!(status("https://example.com/setup.exe"), SecurityStatus::Suspicious);
    }

    #[test]
    fn weak_signals_need_two() {
        assert_eq!(status("http://example.com"), SecurityStatus::NotChecked);
        assert_eq!(status("http://free-stuff.top"), SecurityStatus::Suspicious);
        let a = assess(&Url::parse("http://example.com").unwrap());
        assert!(a.reasons.iter().any(|r| r.contains("HTTP")));
    }
}
