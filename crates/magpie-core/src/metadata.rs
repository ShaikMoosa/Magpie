//! Link previews: fetch a page's title, description, preview image and icon.
//!
//! Safety rules (PRD 5.10):
//! - Only http(s). Every hop is resolved and must point to a public IP; the
//!   connection is pinned to that IP so DNS can't be swapped mid-request
//!   (rebinding). Redirects are followed manually and re-checked.
//! - HTML is read as text only; no scripts run. Bodies are size-capped.
//! - Images are stored only if their bytes are a known raster format (no SVG).

use std::io::Read;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use sha2::{Digest, Sha256};
use url::Url;

use crate::error::{Error, Result};
use crate::html;

const MAX_HTML: usize = 1_500_000;
const MAX_IMAGE: usize = 5_000_000;
const MAX_REDIRECTS: usize = 5;
const TIMEOUT: Duration = Duration::from_secs(10);
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Magpie/0.1 (link preview)";

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PageMeta {
    pub title: Option<String>,
    pub description: Option<String>,
    pub site_name: Option<String>,
    /// Cached preview image file name (see [`is_cache_name`]).
    pub image: Option<String>,
    /// Cached favicon file name.
    pub favicon: Option<String>,
    pub final_url: Option<String>,
}

/// File names written to the preview cache: `<sha256>.<ext>`.
pub fn is_cache_name(name: &str) -> bool {
    let Some((hash, ext)) = name.split_once('.') else { return false };
    hash.len() == 64
        && hash.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        && matches!(ext, "png" | "jpg" | "gif" | "webp" | "ico" | "avif")
}

fn net_err(msg: impl Into<String>) -> Error {
    Error::Network(msg.into())
}

/// True for addresses on the public internet.
pub fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            !(v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_unspecified()
                || v4.is_multicast()
                || o[0] == 0
                || (o[0] == 100 && (64..128).contains(&o[1])) // carrier-grade NAT
                || (o[0] == 192 && o[1] == 0 && o[2] == 0)
                || (o[0] == 198 && (o[1] == 18 || o[1] == 19)) // benchmarking
                || o[0] >= 240)
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_public_ip(IpAddr::V4(v4));
            }
            let seg = v6.segments();
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (seg[0] & 0xfe00) == 0xfc00 // unique local
                || (seg[0] & 0xffc0) == 0xfe80 // link local
                || seg[0] == 0x2001 && seg[1] == 0x0db8) // documentation
        }
    }
}

/// Resolve the URL's host and require every answer to be public.
fn resolve_public(url: &Url) -> Result<SocketAddr> {
    let host = url.host_str().ok_or_else(|| net_err("The address has no host."))?;
    let port = url.port_or_known_default().unwrap_or(443);
    let addrs: Vec<SocketAddr> = (host, port)
        .to_socket_addrs()
        .map_err(|_| net_err("The website's domain couldn't be found."))?
        .collect();
    if addrs.is_empty() {
        return Err(net_err("The website's domain couldn't be found."));
    }
    if addrs.iter().any(|a| !is_public_ip(a.ip())) {
        return Err(net_err("Previews aren't fetched for local or private network addresses."));
    }
    Ok(addrs[0])
}

pub struct Fetcher {
    cache_dir: PathBuf,
}

struct Response {
    url: Url,
    content_type: String,
    body: Vec<u8>,
}

impl Fetcher {
    pub fn new(cache_dir: impl Into<PathBuf>) -> Self {
        Self { cache_dir: cache_dir.into() }
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    fn get(&self, start: &Url, accept: &str, max: usize) -> Result<Response> {
        let mut url = start.clone();
        for _ in 0..=MAX_REDIRECTS {
            if !matches!(url.scheme(), "http" | "https") {
                return Err(net_err("The website redirected to an unsupported address."));
            }
            let addr = resolve_public(&url)?;
            let host = url.host_str().unwrap_or_default().to_string();
            let client = reqwest::blocking::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(TIMEOUT)
                .connect_timeout(Duration::from_secs(6))
                .user_agent(USER_AGENT)
                .resolve(&host, addr)
                .build()
                .map_err(|_| net_err("Couldn't start a network request."))?;
            let resp = client
                .get(url.clone())
                .header(reqwest::header::ACCEPT, accept)
                .header(reqwest::header::ACCEPT_LANGUAGE, "en;q=0.9, *;q=0.5")
                .send()
                .map_err(|e| net_err(if e.is_timeout() { "The website took too long to respond." } else { "The website couldn't be reached." }))?;
            let status = resp.status();
            if status.is_redirection() {
                let loc = resp
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|v| v.to_str().ok())
                    .ok_or_else(|| net_err("The website sent a broken redirect."))?;
                url = url.join(loc).map_err(|_| net_err("The website sent a broken redirect."))?;
                continue;
            }
            if !status.is_success() {
                return Err(net_err(format!("The website responded with error {}.", status.as_u16())));
            }
            let content_type = resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_ascii_lowercase();
            let mut body = Vec::new();
            resp.take(max as u64 + 1).read_to_end(&mut body).map_err(|_| net_err("The download was interrupted."))?;
            if body.len() > max {
                body.truncate(max);
            }
            return Ok(Response { url, content_type, body });
        }
        Err(net_err("The website redirected too many times."))
    }

    /// Fetch and parse a page. Images are downloaded into the cache.
    pub fn fetch(&self, page_url: &str) -> Result<PageMeta> {
        let start = crate::urls::parse(page_url)?.url;
        let resp = self.get(&start, "text/html,application/xhtml+xml;q=0.9,*/*;q=0.5", MAX_HTML)?;
        let mut meta = PageMeta { final_url: Some(resp.url.to_string()), ..Default::default() };
        let mut icon_url = resp.url.join("/favicon.ico").ok();

        if resp.content_type.is_empty() || resp.content_type.contains("html") {
            let text = String::from_utf8_lossy(&resp.body);
            let head = parse_head(&text);
            meta.title = head.title;
            meta.description = head.description;
            meta.site_name = head.site_name;
            if let Some(img) = head.image.and_then(|u| resp.url.join(&u).ok()) {
                meta.image = self.download_image(&img);
            }
            if let Some(icon) = head.icon.and_then(|u| resp.url.join(&u).ok()) {
                icon_url = Some(icon);
            }
        } else if resp.content_type.starts_with("image/") {
            meta.image = self.store_image(&resp.body);
        }
        if let Some(icon) = icon_url {
            meta.favicon = self.download_image(&icon);
        }
        Ok(meta)
    }

    fn download_image(&self, url: &Url) -> Option<String> {
        let resp = self.get(url, "image/avif,image/webp,image/png,image/jpeg,image/*;q=0.8", MAX_IMAGE).ok()?;
        if resp.body.len() >= MAX_IMAGE {
            return None;
        }
        self.store_image(&resp.body)
    }

    /// Save raster images by content hash. Returns the cache file name.
    fn store_image(&self, bytes: &[u8]) -> Option<String> {
        let ext = sniff_image(bytes)?;
        let hash = Sha256::digest(bytes);
        let name = format!("{}.{ext}", hash.iter().map(|b| format!("{b:02x}")).collect::<String>());
        let path = self.cache_dir.join(&name);
        if !path.exists() {
            std::fs::create_dir_all(&self.cache_dir).ok()?;
            std::fs::write(&path, bytes).ok()?;
        }
        Some(name)
    }
}

/// Identify raster image formats by magic bytes. SVG is deliberately not accepted.
pub fn sniff_image(b: &[u8]) -> Option<&'static str> {
    if b.len() < 12 {
        return None;
    }
    if b.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("png")
    } else if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if b.starts_with(b"GIF8") {
        Some("gif")
    } else if b.starts_with(b"RIFF") && &b[8..12] == b"WEBP" {
        Some("webp")
    } else if b.starts_with(&[0, 0, 1, 0]) {
        Some("ico")
    } else if &b[4..12] == b"ftypavif" {
        Some("avif")
    } else {
        None
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct Head {
    pub title: Option<String>,
    pub description: Option<String>,
    pub site_name: Option<String>,
    pub image: Option<String>,
    pub icon: Option<String>,
}

fn non_empty(s: Option<String>) -> Option<String> {
    s.map(|s| html::clean_text(&s)).filter(|s| !s.is_empty()).map(|s| s.chars().take(500).collect())
}

/// Read `<head>` metadata: Open Graph first, then standard tags.
pub fn parse_head(text: &str) -> Head {
    let mut title_tag = None;
    let (mut og_title, mut og_desc, mut og_site, mut og_image, mut tw_image, mut desc) = (None, None, None, None, None, None);
    let mut icons: Vec<(i32, String)> = Vec::new();
    let lower = text.to_ascii_lowercase();
    for (name, body, after) in html::tags(text) {
        match name.as_str() {
            "title" if title_tag.is_none() => {
                let end = lower[after..].find("</title").map(|e| after + e).unwrap_or(after);
                title_tag = Some(text[after..end].to_string());
            }
            "meta" => {
                let key = html::attr(body, "property").or_else(|| html::attr(body, "name")).unwrap_or_default().to_ascii_lowercase();
                let content = html::attr(body, "content");
                match key.as_str() {
                    "og:title" => og_title = og_title.or(content),
                    "og:description" => og_desc = og_desc.or(content),
                    "og:site_name" => og_site = og_site.or(content),
                    "og:image" | "og:image:url" | "og:image:secure_url" => og_image = og_image.or(content),
                    "twitter:image" | "twitter:image:src" => tw_image = tw_image.or(content),
                    "description" => desc = desc.or(content),
                    "twitter:description" => desc = desc.or(content),
                    _ => {}
                }
            }
            "link" => {
                let rel = html::attr(body, "rel").unwrap_or_default().to_ascii_lowercase();
                let Some(href) = html::attr(body, "href").filter(|h| !h.is_empty()) else { continue };
                if href.to_ascii_lowercase().ends_with(".svg") || html::attr(body, "type").is_some_and(|t| t.contains("svg")) {
                    continue;
                }
                let size = html::attr(body, "sizes")
                    .and_then(|s| s.split(['x', 'X']).next().and_then(|n| n.parse::<i32>().ok()))
                    .unwrap_or(16);
                let score = if rel.contains("apple-touch-icon") {
                    1000 + size
                } else if rel.split_whitespace().any(|r| r == "icon") {
                    size
                } else {
                    continue;
                };
                icons.push((score, href));
            }
            "body" | "/head" => break,
            _ => {}
        }
    }
    icons.sort_by(|a, b| b.0.cmp(&a.0));
    Head {
        title: non_empty(og_title).or_else(|| non_empty(title_tag)),
        description: non_empty(og_desc).or_else(|| non_empty(desc)),
        site_name: non_empty(og_site),
        image: og_image.or(tw_image).filter(|s| !s.trim().is_empty()),
        icon: icons.into_iter().next().map(|(_, h)| h),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn private_addresses_are_blocked() {
        for ip in ["127.0.0.1", "10.0.0.5", "172.16.3.4", "192.168.1.1", "169.254.169.254", "100.64.0.1", "0.0.0.0", "255.255.255.255"] {
            assert!(!is_public_ip(ip.parse::<Ipv4Addr>().unwrap().into()), "{ip}");
        }
        for ip in ["::1", "fd00::1", "fe80::1", "::ffff:127.0.0.1"] {
            assert!(!is_public_ip(ip.parse::<Ipv6Addr>().unwrap().into()), "{ip}");
        }
        assert!(is_public_ip("93.184.216.34".parse::<Ipv4Addr>().unwrap().into()));
        assert!(is_public_ip("2606:4700::1111".parse::<Ipv6Addr>().unwrap().into()));
    }

    #[test]
    fn local_hosts_are_refused_before_connecting() {
        let f = Fetcher::new(std::env::temp_dir());
        for url in ["http://localhost:1/", "http://127.0.0.1:9/", "http://192.168.0.1/"] {
            let err = f.fetch(url).unwrap_err();
            assert!(err.to_string().contains("private"), "{url}: {err}");
        }
    }

    #[test]
    fn parses_open_graph_and_icons() {
        let page = r#"<!doctype html><html><head>
            <title>Fallback &amp; title</title>
            <meta name="description" content="Plain description">
            <meta property="og:title" content="Figma Community — UI Kits">
            <meta property="og:description" content="Thousands of free UI kits.">
            <meta property="og:image" content="/og.png">
            <link rel="icon" href="/favicon-16.png" sizes="16x16">
            <link rel="icon" href="/favicon-32.png" sizes="32x32">
            <link rel="icon" href="/logo.svg" type="image/svg+xml">
            <script>document.title = "evil"</script>
            </head><body><meta property="og:title" content="ignored"></body></html>"#;
        let h = parse_head(page);
        assert_eq!(h.title.as_deref(), Some("Figma Community — UI Kits"));
        assert_eq!(h.description.as_deref(), Some("Thousands of free UI kits."));
        assert_eq!(h.image.as_deref(), Some("/og.png"));
        assert_eq!(h.icon.as_deref(), Some("/favicon-32.png"));

        let plain = parse_head("<html><head><title> Hello\n World </title><link rel='apple-touch-icon' href='/a.png'></head>");
        assert_eq!(plain.title.as_deref(), Some("Hello World"));
        assert_eq!(plain.icon.as_deref(), Some("/a.png"));
    }

    #[test]
    fn images_are_sniffed_and_cached_by_hash() {
        let dir = tempfile::tempdir().unwrap();
        let f = Fetcher::new(dir.path());
        let png = [&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A][..], &[0u8; 16]].concat();
        let name = f.store_image(&png).unwrap();
        assert!(is_cache_name(&name) && name.ends_with(".png"));
        assert!(dir.path().join(&name).exists());
        assert!(f.store_image(b"<svg xmlns='http://www.w3.org/2000/svg'></svg>").is_none());
        assert!(!is_cache_name("../evil.png"));
        assert!(!is_cache_name(&format!("{}.svg", "a".repeat(64))));
    }
}
