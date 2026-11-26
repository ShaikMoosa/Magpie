//! Tiny, dependency-free HTML helpers. Never executes scripts; only reads
//! tags and attributes as text.

/// Find attribute `name` (case-insensitive) in a raw tag body like
/// `a href="x" ADD_DATE=1`. Returns the decoded value.
pub fn attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(pos) = lower[from..].find(name) {
        let at = from + pos;
        from = at + name.len();
        let before_ok = at == 0 || lower.as_bytes()[at - 1].is_ascii_whitespace();
        let rest = lower[from..].trim_start();
        if !before_ok || !rest.starts_with('=') {
            continue;
        }
        let value_start = tag.len() - rest.len() + 1;
        let v = tag[value_start..].trim_start();
        let (quote, body) = match v.chars().next() {
            Some(q @ ('"' | '\'')) => (Some(q), &v[1..]),
            _ => (None, v),
        };
        let value = match quote {
            Some(q) => body.split(q).next().unwrap_or(""),
            None => body.split(char::is_whitespace).next().unwrap_or(""),
        };
        return Some(decode_entities(value));
    }
    None
}

/// Strip tags, collapse whitespace and decode entities.
pub fn clean_text(s: &str) -> String {
    let mut no_tags = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => no_tags.push(c),
            _ => {}
        }
    }
    decode_entities(no_tags.split_whitespace().collect::<Vec<_>>().join(" ").as_str())
}

pub fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(p) = rest.find('&') {
        out.push_str(&rest[..p]);
        rest = &rest[p..];
        // Entities are short; only look a few bytes ahead, but never cut a
        // multi-byte character in half (that would panic).
        let mut cut = rest.len().min(10);
        while !rest.is_char_boundary(cut) {
            cut -= 1;
        }
        let window = &rest[..cut];
        let Some(semi) = window.find(';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let ent = &rest[1..semi];
        let decoded = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            "nbsp" => Some(' '),
            "mdash" => Some('—'),
            "ndash" => Some('–'),
            "hellip" => Some('…'),
            "rsquo" => Some('’'),
            "lsquo" => Some('‘'),
            "rdquo" => Some('”'),
            "ldquo" => Some('“'),
            _ if ent.starts_with("#x") || ent.starts_with("#X") => u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32),
            _ if ent.starts_with('#') => ent[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Iterate tags as (lowercase name, raw tag body, byte offset after `>`).
pub fn tags(html: &str) -> impl Iterator<Item = (String, &str, usize)> {
    let mut i = 0;
    std::iter::from_fn(move || {
        loop {
            let off = html[i..].find('<')?;
            let start = i + off;
            let end = start + html[start..].find('>')?;
            i = end + 1;
            let body = &html[start + 1..end];
            if body.starts_with('!') {
                continue;
            }
            let name: String = body
                .trim_start_matches('/')
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect::<String>()
                .to_ascii_lowercase();
            let name = if body.starts_with('/') { format!("/{name}") } else { name };
            return Some((name, body, end + 1));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_entities_handles_multibyte_text_after_ampersand() {
        // Regression: the look-ahead window used to cut '—' in half and panic,
        // which aborted the app during background preview fetches.
        assert_eq!(decode_entities("Tom & Jerry — cartoons"), "Tom & Jerry — cartoons");
        assert_eq!(decode_entities("A &🙂🙂🙂 b"), "A &🙂🙂🙂 b");
        assert_eq!(decode_entities("&नमस्ते दुनिया"), "&नमस्ते दुनिया");
        assert_eq!(decode_entities("é&amp;—&hellip;"), "é&—…");
        // Every cut position around multi-byte characters stays safe.
        let s = "&a—b—c—d—e—f&amp;";
        for i in 0..s.len() {
            if s.is_char_boundary(i) {
                let _ = decode_entities(&s[i..]);
            }
        }
    }

    #[test]
    fn parse_head_survives_multibyte_entities() {
        let html = "<head><title>Q&A — ঢাকা</title><meta name=\"description\" content=\"R&D 🚀 notes\"></head>";
        let head = crate::metadata::parse_head(html);
        assert_eq!(head.title.as_deref(), Some("Q&A — ঢাকা"));
        assert_eq!(head.description.as_deref(), Some("R&D 🚀 notes"));
    }

    #[test]
    fn attributes() {
        assert_eq!(attr(r#"a HREF="x?a=1&amp;b=2" add_date=17"#, "href").as_deref(), Some("x?a=1&b=2"));
        assert_eq!(attr("a add_date=17 x", "add_date").as_deref(), Some("17"));
        assert_eq!(attr("meta data-href='no' href='yes'", "href").as_deref(), Some("yes"));
        assert_eq!(attr("link rel=icon href=/img/f.ico", "href").as_deref(), Some("/img/f.ico"));
    }

    #[test]
    fn entities_and_text() {
        assert_eq!(clean_text("  React &amp;\n <b>friends</b> &#8212; &#x2019; "), "React & friends — ’");
        assert_eq!(decode_entities("a & b &unknown; c"), "a & b &unknown; c");
    }

    #[test]
    fn tag_iteration() {
        let names: Vec<_> = tags("<!doctype html><html><head><title>x</title></head>").map(|(n, _, _)| n).collect();
        assert_eq!(names, ["html", "head", "title", "/title", "/head"]);
    }
}
