//! URL validation and conservative normalisation for duplicate detection.

use url::Url;

use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedUrl {
    /// The URL as it will be stored and opened.
    pub url: Url,
    /// Key used for duplicate detection.
    pub normalized: String,
    /// Host without a leading `www.`.
    pub domain: String,
}

/// Validate user input and return the canonical URL plus its duplicate key.
///
/// Bare domains (`example.com/docs`) get `https://` added. Only http(s) URLs
/// with a host are accepted.
pub fn parse(input: &str) -> Result<ParsedUrl> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(Error::InvalidUrl("Enter a website address.".into()));
    }
    if trimmed.chars().any(char::is_whitespace) {
        return Err(Error::InvalidUrl("Web addresses can't contain spaces.".into()));
    }

    let candidate = if trimmed.contains("://") {
        trimmed.to_string()
    } else if looks_like_host(trimmed) {
        format!("https://{trimmed}")
    } else {
        return Err(Error::InvalidUrl("That doesn't look like a web address.".into()));
    };

    let url = Url::parse(&candidate)
        .map_err(|_| Error::InvalidUrl("That doesn't look like a web address.".into()))?;

    if !matches!(url.scheme(), "http" | "https") {
        return Err(Error::InvalidUrl("Only http and https links can be saved.".into()));
    }
    let host = url
        .host_str()
        .filter(|h| !h.is_empty())
        .ok_or_else(|| Error::InvalidUrl("The address is missing a domain.".into()))?
        .to_ascii_lowercase();

    let domain = host.strip_prefix("www.").unwrap_or(&host).to_string();
    let normalized = normalize(&url, &domain);
    Ok(ParsedUrl { url, normalized, domain })
}

fn looks_like_host(s: &str) -> bool {
    let host = s.split(['/', '?', '#']).next().unwrap_or("");
    let host = host.split(':').next().unwrap_or("");
    host == "localhost" || (host.contains('.') && !host.starts_with('.') && !host.ends_with('.'))
}

/// Conservative normalisation: ignores scheme, `www.`, default ports, a
/// trailing slash and the fragment. Paths and query strings are kept.
fn normalize(url: &Url, domain: &str) -> String {
    let mut key = String::from(domain);
    if let Some(port) = url.port() {
        key.push(':');
        key.push_str(&port.to_string());
    }
    let path = url.path();
    let path = if path == "/" { "" } else { path.strip_suffix('/').unwrap_or(path) };
    key.push_str(path);
    if let Some(q) = url.query().filter(|q| !q.is_empty()) {
        key.push('?');
        key.push_str(q);
    }
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(s: &str) -> String {
        parse(s).unwrap().normalized
    }

    #[test]
    fn trailing_slash_scheme_and_www_are_equivalent() {
        assert_eq!(key("https://example.com/"), key("https://example.com"));
        assert_eq!(key("http://www.example.com"), key("https://example.com"));
        assert_eq!(key("https://example.com/docs/"), key("https://example.com/docs"));
        assert_eq!(key("https://example.com/#top"), key("https://example.com"));
    }

    #[test]
    fn meaningful_paths_and_queries_stay_distinct() {
        assert_ne!(key("https://example.com/products"), key("https://example.com/pricing"));
        assert_ne!(key("https://example.com/?page=1"), key("https://example.com/?page=2"));
        assert_ne!(key("https://example.com:8080"), key("https://example.com"));
    }

    #[test]
    fn bare_domains_get_https() {
        let p = parse("figma.com/community").unwrap();
        assert_eq!(p.url.as_str(), "https://figma.com/community");
        assert_eq!(p.domain, "figma.com");
    }

    #[test]
    fn rejects_bad_input() {
        for bad in ["", "   ", "hello world", "javascript:alert(1)", "ftp://x.com", "file:///C:/x", "nodot"] {
            assert!(parse(bad).is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn hosts_are_lowercased_and_idn_is_punycode() {
        let p = parse("https://ExAmPle.COM/Path").unwrap();
        assert_eq!(p.domain, "example.com");
        assert_eq!(p.url.path(), "/Path");
        let idn = parse("https://bücher.de").unwrap();
        assert_eq!(idn.domain, "xn--bcher-kva.de");
    }
}
