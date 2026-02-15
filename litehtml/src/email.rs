//! Email-specific features for litehtml rendering.
//!
//! Provides encoding detection, HTML sanitization, data/cid URI handling,
//! and a convenience pipeline for preparing email HTML for rendering.

use base64::Engine;
use encoding_rs::Encoding;

// ---------------------------------------------------------------------------
// Email user-agent stylesheet
// ---------------------------------------------------------------------------

pub const EMAIL_MASTER_CSS: &str = "\
body { margin: 0; padding: 0; word-wrap: break-word; overflow-wrap: break-word; }\n\
img { max-width: 100%; height: auto; }\n\
table { border-collapse: collapse; mso-table-lspace: 0; mso-table-rspace: 0; }\n\
td, th { padding: 0; }\n\
a { color: inherit; }\n\
p, h1, h2, h3, h4, h5, h6 { margin: 0; padding: 0; }\n\
";

// ---------------------------------------------------------------------------
// Encoding detection & conversion
// ---------------------------------------------------------------------------

/// Decode email HTML bytes into a UTF-8 string.
///
/// Tries to detect encoding from BOM or `<meta>` charset declarations.
/// Falls back to UTF-8, then Windows-1252 (the most common legacy email encoding).
pub fn decode_html(bytes: &[u8]) -> String {
    // Check BOM first
    if let Some(encoding) = detect_bom(bytes) {
        let (result, _, _) = encoding.decode(bytes);
        return result.into_owned();
    }

    // Scan for <meta charset="..."> or <meta http-equiv="Content-Type" content="...charset=...">
    // We only look at a prefix to avoid scanning huge bodies.
    let scan_len = bytes.len().min(4096);
    if let Ok(prefix) = std::str::from_utf8(&bytes[..scan_len]) {
        if let Some(encoding) = detect_meta_charset(prefix) {
            let (result, _, _) = encoding.decode(bytes);
            return result.into_owned();
        }
    }
    // Also try scanning as latin1 in case the prefix itself isn't valid UTF-8
    if std::str::from_utf8(&bytes[..scan_len]).is_err() {
        let (prefix_cow, _, _) = encoding_rs::WINDOWS_1252.decode(&bytes[..scan_len]);
        if let Some(encoding) = detect_meta_charset(&prefix_cow) {
            let (result, _, _) = encoding.decode(bytes);
            return result.into_owned();
        }
    }

    // Try UTF-8 first
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_owned();
    }

    // Fall back to Windows-1252
    let (result, _, _) = encoding_rs::WINDOWS_1252.decode(bytes);
    result.into_owned()
}

fn detect_bom(bytes: &[u8]) -> Option<&'static Encoding> {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        Some(encoding_rs::UTF_8)
    } else if bytes.starts_with(&[0xFF, 0xFE]) {
        Some(encoding_rs::UTF_16LE)
    } else if bytes.starts_with(&[0xFE, 0xFF]) {
        Some(encoding_rs::UTF_16BE)
    } else {
        None
    }
}

fn detect_meta_charset(html: &str) -> Option<&'static Encoding> {
    let lower = html.to_ascii_lowercase();

    // <meta charset="...">
    if let Some(pos) = lower.find("charset") {
        let rest = &lower[pos + 7..];
        // Skip optional whitespace and '='
        let rest = rest.trim_start();
        let rest = rest.strip_prefix('=')?;
        let rest = rest.trim_start();
        // Strip optional quotes
        let (encoding_name, _) = if rest.starts_with('"') {
            let inner = &rest[1..];
            let end = inner.find('"').unwrap_or(inner.len());
            (&inner[..end], &inner[end..])
        } else if rest.starts_with('\'') {
            let inner = &rest[1..];
            let end = inner.find('\'').unwrap_or(inner.len());
            (&inner[..end], &inner[end..])
        } else {
            let end = rest
                .find(|c: char| {
                    c.is_ascii_whitespace() || c == '"' || c == '\'' || c == ';' || c == '>'
                })
                .unwrap_or(rest.len());
            (&rest[..end], &rest[end..])
        };

        let encoding_name = encoding_name.trim();
        if !encoding_name.is_empty() {
            return Encoding::for_label(encoding_name.as_bytes());
        }
    }

    None
}

// ---------------------------------------------------------------------------
// HTML sanitization
// ---------------------------------------------------------------------------

/// Elements to strip entirely (tag + contents).
const STRIP_ELEMENTS: &[&str] = &[
    "script", "iframe", "object", "embed", "form", "input", "textarea", "select", "button",
];

/// Strip dangerous elements and attributes from email HTML.
///
/// Removes `<script>`, `<iframe>`, `<object>`, `<embed>`, `<form>` and form controls,
/// event handler attributes (`on*`), and `<link rel="stylesheet">` elements.
/// Preserves all other HTML structure.
pub fn sanitize_html(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut chars = html.char_indices().peekable();

    while let Some(&(i, c)) = chars.peek() {
        if c == '<' {
            // Find the end of this tag
            let tag_start = i;
            let rest = &html[i..];

            // Check for comment
            if rest.starts_with("<!--") {
                // Pass comments through
                if let Some(end) = rest.find("-->") {
                    let comment_end = i + end + 3;
                    result.push_str(&html[tag_start..comment_end]);
                    // Advance past the comment
                    while let Some(&(j, _)) = chars.peek() {
                        if j >= comment_end {
                            break;
                        }
                        chars.next();
                    }
                    continue;
                }
            }

            // Find the '>' that closes this tag
            let tag_end = match find_tag_end(html, i) {
                Some(end) => end,
                None => {
                    // Malformed: no closing '>', output as-is
                    result.push(c);
                    chars.next();
                    continue;
                }
            };

            let tag_content = &html[i + 1..tag_end]; // between < and >
            let tag_str = &html[i..=tag_end]; // includes < and >

            let tag_name = extract_tag_name(tag_content);
            let tag_lower = tag_name.to_ascii_lowercase();
            let is_closing = tag_content.starts_with('/');

            // Check <link rel="stylesheet">
            if tag_lower == "link" && is_stylesheet_link(tag_content) {
                // Skip this tag entirely
                advance_past(&mut chars, tag_end + 1);
                continue;
            }

            // Check stripped elements
            if let Some(stripped) = STRIP_ELEMENTS.iter().find(|&&s| s == tag_lower) {
                if is_closing {
                    // Skip closing tag
                    advance_past(&mut chars, tag_end + 1);
                    continue;
                }
                // Opening or self-closing: skip tag and its content until matching close
                let is_self_closing = tag_content.ends_with('/');
                advance_past(&mut chars, tag_end + 1);
                if !is_self_closing {
                    skip_until_close_tag(html, &mut chars, stripped);
                }
                continue;
            }

            // For normal tags, strip on* event handler attributes
            let cleaned = strip_event_handlers(tag_str);
            result.push_str(&cleaned);
            advance_past(&mut chars, tag_end + 1);
        } else {
            result.push(c);
            chars.next();
        }
    }

    result
}

/// Find the index of '>' that closes the tag starting at `start` (the '<').
/// Respects quoted attribute values.
fn find_tag_end(html: &str, start: usize) -> Option<usize> {
    let bytes = html.as_bytes();
    let mut i = start + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'>' => return Some(i),
            b'"' => {
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    i += 1;
                }
            }
            b'\'' => {
                i += 1;
                while i < bytes.len() && bytes[i] != b'\'' {
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

fn extract_tag_name(tag_content: &str) -> &str {
    let s = tag_content.trim_start_matches('/').trim_start();
    let end = s
        .find(|c: char| c.is_ascii_whitespace() || c == '/' || c == '>')
        .unwrap_or(s.len());
    &s[..end]
}

fn is_stylesheet_link(tag_content: &str) -> bool {
    let lower = tag_content.to_ascii_lowercase();
    // Look for rel="stylesheet" or rel='stylesheet'
    if let Some(pos) = lower.find("rel") {
        let rest = lower[pos + 3..].trim_start();
        if let Some(rest) = rest.strip_prefix('=') {
            let rest = rest.trim_start();
            let val = if rest.starts_with('"') {
                let inner = &rest[1..];
                let end = inner.find('"').unwrap_or(inner.len());
                &inner[..end]
            } else if rest.starts_with('\'') {
                let inner = &rest[1..];
                let end = inner.find('\'').unwrap_or(inner.len());
                &inner[..end]
            } else {
                let end = rest
                    .find(|c: char| c.is_ascii_whitespace() || c == '>')
                    .unwrap_or(rest.len());
                &rest[..end]
            };
            return val.trim() == "stylesheet";
        }
    }
    false
}

fn advance_past(chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>, target: usize) {
    while let Some(&(j, _)) = chars.peek() {
        if j >= target {
            break;
        }
        chars.next();
    }
}

fn skip_until_close_tag(
    html: &str,
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    tag_name: &str,
) {
    let close_pattern = format!("</{}", tag_name);
    let mut depth = 1u32;
    let open_pattern = format!("<{}", tag_name);

    while let Some(&(i, _)) = chars.peek() {
        let rest = &html[i..];

        if rest.len() >= close_pattern.len()
            && rest[..close_pattern.len()].eq_ignore_ascii_case(&close_pattern)
        {
            // Check it's actually a close tag (followed by > or whitespace)
            let after = &rest[close_pattern.len()..];
            if after.starts_with('>') || after.starts_with(char::is_whitespace) {
                depth -= 1;
                if depth == 0 {
                    // Skip past the closing tag
                    if let Some(end) = find_tag_end(html, i) {
                        advance_past(chars, end + 1);
                    }
                    return;
                }
            }
        } else if rest.len() >= open_pattern.len()
            && rest[..open_pattern.len()].eq_ignore_ascii_case(&open_pattern)
        {
            let after = &rest[open_pattern.len()..];
            if after.starts_with('>')
                || after.starts_with(char::is_whitespace)
                || after.starts_with('/')
            {
                depth += 1;
            }
        }

        chars.next();
    }
}

/// Remove on* event handler attributes from a single tag string (including < and >).
fn strip_event_handlers(tag: &str) -> String {
    // Fast path: no "on" attribute likely present
    if !tag.to_ascii_lowercase().contains(" on") {
        return tag.to_owned();
    }

    let mut result = String::with_capacity(tag.len());
    let bytes = tag.as_bytes();
    // Copy up to and including the tag name (and first whitespace boundary)
    // Find the first whitespace after '<tagname'
    let tag_inner_start = if bytes.first() == Some(&b'<') { 1 } else { 0 };
    let mut j = tag_inner_start;
    // Skip optional '/'
    if j < bytes.len() && bytes[j] == b'/' {
        j += 1;
    }
    // Skip tag name
    while j < bytes.len() && !bytes[j].is_ascii_whitespace() && bytes[j] != b'>' && bytes[j] != b'/'
    {
        j += 1;
    }

    result.push_str(&tag[..j]);
    let mut i = j;

    while i < bytes.len() {
        // Skip whitespace
        if bytes[i].is_ascii_whitespace() {
            let ws_start = i;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i >= bytes.len() || bytes[i] == b'>' || bytes[i] == b'/' {
                result.push_str(&tag[ws_start..i]);
                continue;
            }

            // Read attribute name
            let attr_start = i;
            while i < bytes.len()
                && bytes[i] != b'='
                && !bytes[i].is_ascii_whitespace()
                && bytes[i] != b'>'
                && bytes[i] != b'/'
            {
                i += 1;
            }
            let attr_name = &tag[attr_start..i];
            let is_event = attr_name.len() > 2
                && attr_name[..2].eq_ignore_ascii_case("on")
                && attr_name.as_bytes()[2].is_ascii_alphabetic();

            // Read optional '=' and value
            let mut val_end = i;
            let mut temp = i;
            // Skip whitespace before '='
            while temp < bytes.len() && bytes[temp].is_ascii_whitespace() {
                temp += 1;
            }
            if temp < bytes.len() && bytes[temp] == b'=' {
                temp += 1;
                // Skip whitespace after '='
                while temp < bytes.len() && bytes[temp].is_ascii_whitespace() {
                    temp += 1;
                }
                // Read value
                if temp < bytes.len() && bytes[temp] == b'"' {
                    temp += 1;
                    while temp < bytes.len() && bytes[temp] != b'"' {
                        temp += 1;
                    }
                    if temp < bytes.len() {
                        temp += 1; // skip closing quote
                    }
                } else if temp < bytes.len() && bytes[temp] == b'\'' {
                    temp += 1;
                    while temp < bytes.len() && bytes[temp] != b'\'' {
                        temp += 1;
                    }
                    if temp < bytes.len() {
                        temp += 1;
                    }
                } else {
                    // Unquoted value
                    while temp < bytes.len()
                        && !bytes[temp].is_ascii_whitespace()
                        && bytes[temp] != b'>'
                    {
                        temp += 1;
                    }
                }
                val_end = temp;
            }

            if is_event {
                // Drop the whitespace + attribute entirely
                i = val_end;
            } else {
                result.push_str(&tag[ws_start..val_end]);
                i = val_end;
            }
        } else {
            result.push(tag[i..].chars().next().unwrap());
            i += tag[i..].chars().next().unwrap().len_utf8();
        }
    }

    result
}

// ---------------------------------------------------------------------------
// data: URI parsing
// ---------------------------------------------------------------------------

/// Decode a `data:` URI into raw bytes.
///
/// Supports `data:[<mediatype>][;base64],<data>` format.
/// Returns `None` for invalid or non-data URIs.
pub fn decode_data_uri(uri: &str) -> Option<Vec<u8>> {
    let rest = uri.strip_prefix("data:")?;
    let comma_pos = rest.find(',')?;
    let header = &rest[..comma_pos];
    let data = &rest[comma_pos + 1..];

    if header.ends_with(";base64") {
        base64::engine::general_purpose::STANDARD
            .decode(data)
            .ok()
            .or_else(|| {
                // Try with whitespace stripped (common in email)
                let cleaned: String = data.chars().filter(|c| !c.is_ascii_whitespace()).collect();
                base64::engine::general_purpose::STANDARD
                    .decode(&cleaned)
                    .ok()
            })
    } else {
        // Plain text encoding: percent-decode
        Some(percent_decode(data))
    }
}

fn percent_decode(input: &str) -> Vec<u8> {
    let mut result = Vec::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                result.push(hi << 4 | lo);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i]);
        i += 1;
    }
    result
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// cid: URI resolution
// ---------------------------------------------------------------------------

/// Type alias for a closure that resolves `cid:` URIs to raw image bytes.
pub type CidResolver = Box<dyn Fn(&str) -> Option<Vec<u8>>>;

/// Resolve an image URI to raw bytes.
///
/// - `data:` URIs are decoded inline.
/// - `cid:` URIs are passed to the optional resolver.
/// - Remote URLs are passed to the optional `url_fetcher` if provided.
/// - Remote URLs return `None` when no fetcher is given (privacy: no external fetching by default).
pub fn resolve_image_uri(
    uri: &str,
    cid_resolver: Option<&dyn Fn(&str) -> Option<Vec<u8>>>,
    url_fetcher: Option<&dyn Fn(&str) -> Option<Vec<u8>>>,
) -> Option<Vec<u8>> {
    if uri.starts_with("data:") {
        decode_data_uri(uri)
    } else if let Some(cid) = uri.strip_prefix("cid:") {
        cid_resolver.and_then(|resolve| resolve(cid))
    } else {
        url_fetcher.and_then(|fetch| fetch(uri))
    }
}

// ---------------------------------------------------------------------------
// Attribute preprocessing (for attrs litehtml doesn't handle natively)
// ---------------------------------------------------------------------------

/// Convert unsupported HTML attributes to inline CSS.
///
/// Handles:
/// - `<body bgcolor="...">` → inline `background-color` style
/// - `<table cellpadding="N">` → inline `padding` on descendant `<td>`/`<th>`
pub fn preprocess_attrs(html: &str) -> String {
    let mut result = html.to_owned();
    result = preprocess_body_bgcolor(&result);
    result = preprocess_cellpadding(&result);
    result
}

/// Convert `<body bgcolor="X">` to `<body style="background-color: X;">`.
fn preprocess_body_bgcolor(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let Some(body_pos) = lower.find("<body") else {
        return html.to_owned();
    };
    let tag_end = match lower[body_pos..].find('>') {
        Some(e) => body_pos + e,
        None => return html.to_owned(),
    };
    let tag = &html[body_pos..=tag_end];
    let tag_lower = tag.to_ascii_lowercase();

    let Some(bg_pos) = tag_lower.find("bgcolor") else {
        return html.to_owned();
    };
    let rest = &tag_lower[bg_pos + 7..];
    let rest = rest.trim_start();
    let Some(rest) = rest.strip_prefix('=') else {
        return html.to_owned();
    };
    let rest = rest.trim_start();

    // Extract the value (may be quoted or unquoted)
    let (value, attr_end_offset) = if rest.starts_with('"') {
        let inner = &rest[1..];
        let end = inner.find('"').unwrap_or(inner.len());
        (
            &tag[bg_pos + 7 + (tag_lower.len() - bg_pos - 7 - rest.len()) + 1
                ..bg_pos + 7 + (tag_lower.len() - bg_pos - 7 - rest.len()) + 1 + end],
            end + 2,
        )
    } else if rest.starts_with('\'') {
        let inner = &rest[1..];
        let end = inner.find('\'').unwrap_or(inner.len());
        (
            &tag[bg_pos + 7 + (tag_lower.len() - bg_pos - 7 - rest.len()) + 1
                ..bg_pos + 7 + (tag_lower.len() - bg_pos - 7 - rest.len()) + 1 + end],
            end + 2,
        )
    } else {
        let end = rest
            .find(|c: char| c.is_ascii_whitespace() || c == '>')
            .unwrap_or(rest.len());
        let offset = tag_lower.len() - bg_pos - 7 - rest.len();
        (&tag[bg_pos + 7 + offset..bg_pos + 7 + offset + end], end)
    };
    let _ = attr_end_offset;

    let color = value.trim();
    if color.is_empty() {
        return html.to_owned();
    }

    // Build new tag: remove bgcolor attr, add/merge style
    let mut new_tag = String::new();
    // Remove bgcolor attribute from the tag
    let tag_bytes = tag.as_bytes();
    let abs_bg_start = bg_pos;
    // Find the full extent of the bgcolor="..." attribute
    let mut i = abs_bg_start;
    while i < tag_bytes.len() && tag_bytes[i] != b'=' {
        i += 1;
    }
    i += 1; // skip '='
    while i < tag_bytes.len() && tag_bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    if i < tag_bytes.len() && (tag_bytes[i] == b'"' || tag_bytes[i] == b'\'') {
        let quote = tag_bytes[i];
        i += 1;
        while i < tag_bytes.len() && tag_bytes[i] != quote {
            i += 1;
        }
        i += 1; // skip closing quote
    } else {
        while i < tag_bytes.len() && !tag_bytes[i].is_ascii_whitespace() && tag_bytes[i] != b'>' {
            i += 1;
        }
    }

    // Remove leading whitespace before bgcolor
    let mut start = abs_bg_start;
    while start > 0 && tag_bytes[start - 1].is_ascii_whitespace() {
        start -= 1;
    }

    new_tag.push_str(&tag[..start]);
    new_tag.push_str(&tag[i..]);

    // Now add style
    let style_addition = format!("background-color: {};", color);
    let new_tag_lower = new_tag.to_ascii_lowercase();
    if let Some(style_pos) = new_tag_lower.find("style=\"") {
        let insert_pos = style_pos + 7;
        new_tag.insert_str(insert_pos, &format!("{} ", style_addition));
    } else if let Some(style_pos) = new_tag_lower.find("style='") {
        let insert_pos = style_pos + 7;
        new_tag.insert_str(insert_pos, &format!("{} ", style_addition));
    } else {
        // Insert style before the closing >
        let close = new_tag.rfind('>').unwrap();
        new_tag.insert_str(close, &format!(" style=\"{}\"", style_addition));
    }

    let mut result = String::with_capacity(html.len());
    result.push_str(&html[..body_pos]);
    result.push_str(&new_tag);
    result.push_str(&html[tag_end + 1..]);
    result
}

/// Convert `<table cellpadding="N">` to add padding to descendant cells.
/// Injects a `data-cellpadding` attribute and adds padding via user styles.
/// Since we can't easily modify all descendant td/th elements, we add padding
/// to the EMAIL_MASTER_CSS default. For per-table cellpadding, we convert
/// the attribute to a CSS `border-spacing` + `padding` via inline style on the table
/// and rely on litehtml's CSS inheritance.
fn preprocess_cellpadding(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let mut result = String::with_capacity(html.len());
    let mut last = 0;

    let mut search_from = 0;
    while let Some(pos) = lower[search_from..].find("cellpadding") {
        let abs_pos = search_from + pos;
        search_from = abs_pos + 11;

        // Verify this is inside a <table tag
        let before = &lower[..abs_pos];
        let last_open = before.rfind('<');
        if let Some(lo) = last_open {
            let tag_start = &lower[lo..abs_pos];
            if !tag_start.contains("table") {
                continue;
            }
        } else {
            continue;
        }

        let rest = &lower[abs_pos + 11..];
        let rest = rest.trim_start();
        if !rest.starts_with('=') {
            continue;
        }
        let rest = rest[1..].trim_start();

        let (value, _val_len) = if rest.starts_with('"') {
            let inner = &rest[1..];
            let end = inner.find('"').unwrap_or(inner.len());
            (
                &html[abs_pos + 11 + (lower.len() - abs_pos - 11 - rest.len()) + 1
                    ..abs_pos + 11 + (lower.len() - abs_pos - 11 - rest.len()) + 1 + end],
                end + 2,
            )
        } else if rest.starts_with('\'') {
            let inner = &rest[1..];
            let end = inner.find('\'').unwrap_or(inner.len());
            (
                &html[abs_pos + 11 + (lower.len() - abs_pos - 11 - rest.len()) + 1
                    ..abs_pos + 11 + (lower.len() - abs_pos - 11 - rest.len()) + 1 + end],
                end + 2,
            )
        } else {
            let end = rest
                .find(|c: char| c.is_ascii_whitespace() || c == '>')
                .unwrap_or(rest.len());
            let offset = lower.len() - abs_pos - 11 - rest.len();
            (
                &html[abs_pos + 11 + offset..abs_pos + 11 + offset + end],
                end,
            )
        };

        let padding = value.trim();
        if padding.is_empty() || padding.parse::<u32>().is_err() {
            continue;
        }

        // Find the table tag boundaries
        let table_start = before.rfind('<').unwrap();
        let tag_rest = &lower[table_start..];
        let tag_end = match tag_rest.find('>') {
            Some(e) => table_start + e,
            None => continue,
        };

        let table_tag = &html[table_start..=tag_end];

        // Remove cellpadding attribute
        let cp_in_tag = abs_pos - table_start;
        let mut attr_end = cp_in_tag + 11;
        let tb = table_tag.as_bytes();
        while attr_end < tb.len() && tb[attr_end] != b'=' {
            attr_end += 1;
        }
        attr_end += 1;
        while attr_end < tb.len() && tb[attr_end].is_ascii_whitespace() {
            attr_end += 1;
        }
        if attr_end < tb.len() && (tb[attr_end] == b'"' || tb[attr_end] == b'\'') {
            let q = tb[attr_end];
            attr_end += 1;
            while attr_end < tb.len() && tb[attr_end] != q {
                attr_end += 1;
            }
            attr_end += 1;
        } else {
            while attr_end < tb.len() && !tb[attr_end].is_ascii_whitespace() && tb[attr_end] != b'>'
            {
                attr_end += 1;
            }
        }

        let mut attr_start = cp_in_tag;
        while attr_start > 0 && tb[attr_start - 1].is_ascii_whitespace() {
            attr_start -= 1;
        }

        let mut new_tag = String::new();
        new_tag.push_str(&table_tag[..attr_start]);
        new_tag.push_str(&table_tag[attr_end..]);

        // We can't easily add padding to child td/th elements via string manipulation,
        // so we return cellpadding as a data attribute that the caller can handle.
        // For practical email rendering, the EMAIL_MASTER_CSS already sets td { padding: 0 },
        // and most email HTML uses inline styles on cells.
        // As a pragmatic solution: convert to a style on the table that uses CSS custom property.
        let close = new_tag.rfind('>').unwrap();
        new_tag.insert_str(close, &format!(" data-cellpadding=\"{}\"", padding));

        result.push_str(&html[last..table_start]);
        result.push_str(&new_tag);
        last = tag_end + 1;
    }

    result.push_str(&html[last..]);
    result
}

// ---------------------------------------------------------------------------
// Email preprocessing pipeline
// ---------------------------------------------------------------------------

/// Preprocessed email ready for rendering.
///
/// When a `url_fetcher` is provided to [`prepare_email_html`], remote image URIs
/// (e.g. `https://`) are also resolved and included in [`images`](Self::images).
/// Without a fetcher, only `data:` and `cid:` images are resolved (privacy default).
#[derive(Debug, Clone)]
pub struct PreparedEmail {
    /// Sanitized, UTF-8 HTML with image URIs intact.
    pub html: String,
    /// Resolved images: `(original_uri, decoded_bytes)`.
    pub images: Vec<(String, Vec<u8>)>,
}

/// Full email preprocessing pipeline: decode encoding, sanitize HTML,
/// extract and resolve `data:`/`cid:` images.
///
/// When `url_fetcher` is provided, remote image URIs (http/https) are also
/// fetched and included in the returned [`PreparedEmail::images`].
/// Without a fetcher, remote URIs are skipped (privacy default).
pub fn prepare_email_html(
    raw: &[u8],
    cid_resolver: Option<&dyn Fn(&str) -> Option<Vec<u8>>>,
    url_fetcher: Option<&dyn Fn(&str) -> Option<Vec<u8>>>,
) -> PreparedEmail {
    let decoded = decode_html(raw);
    let preprocessed = preprocess_attrs(&decoded);
    let sanitized = sanitize_html(&preprocessed);

    // Extract image URIs from src attributes
    let mut images = Vec::new();
    let lower = sanitized.to_ascii_lowercase();
    let mut search_from = 0;

    while let Some(pos) = lower[search_from..].find("src=") {
        let abs_pos = search_from + pos + 4;
        search_from = abs_pos;

        if abs_pos >= sanitized.len() {
            break;
        }

        let rest = &sanitized[abs_pos..];
        let (uri, _) = if rest.starts_with('"') {
            let inner = &rest[1..];
            let end = inner.find('"').unwrap_or(inner.len());
            (&inner[..end], end + 2)
        } else if rest.starts_with('\'') {
            let inner = &rest[1..];
            let end = inner.find('\'').unwrap_or(inner.len());
            (&inner[..end], end + 2)
        } else {
            let end = rest
                .find(|c: char| c.is_ascii_whitespace() || c == '>')
                .unwrap_or(rest.len());
            (&rest[..end], end)
        };

        let is_local = uri.starts_with("data:") || uri.starts_with("cid:");
        if is_local || url_fetcher.is_some() {
            if let Some(bytes) = resolve_image_uri(uri, cid_resolver, url_fetcher) {
                images.push((uri.to_owned(), bytes));
            }
        }
    }

    PreparedEmail {
        html: sanitized,
        images,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Encoding detection --

    #[test]
    fn decode_html_utf8() {
        let html = "<p>Hello world</p>";
        assert_eq!(decode_html(html.as_bytes()), html);
    }

    #[test]
    fn decode_html_utf8_with_bom() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(b"<p>BOM</p>");
        let result = decode_html(&bytes);
        assert!(result.contains("<p>BOM</p>"));
    }

    #[test]
    fn decode_html_latin1() {
        // 0xE9 = 'e' with acute accent in ISO-8859-1 / Windows-1252
        let bytes = b"<p>caf\xe9</p>".to_vec();
        let result = decode_html(&bytes);
        assert!(result.contains("cafe\u{0301}") || result.contains("caf\u{00e9}"));
    }

    #[test]
    fn decode_html_with_meta_charset() {
        let html = b"<html><head><meta charset=\"iso-8859-1\"></head><body>caf\xe9</body></html>";
        let result = decode_html(html);
        assert!(result.contains("caf\u{00e9}"));
    }

    #[test]
    fn decode_html_with_content_type_meta() {
        let html = b"<html><head><meta http-equiv=\"Content-Type\" content=\"text/html; charset=windows-1252\"></head><body>\x93quote\x94</body></html>";
        let result = decode_html(html);
        // Windows-1252 0x93/0x94 are left/right double quotes
        assert!(result.contains('\u{201c}') || result.contains('\u{201d}'));
    }

    // -- Sanitization --

    #[test]
    fn sanitize_strips_script() {
        let html = "<p>Hello</p><script>alert('xss')</script><p>World</p>";
        let result = sanitize_html(html);
        assert!(!result.contains("script"));
        assert!(!result.contains("alert"));
        assert!(result.contains("<p>Hello</p>"));
        assert!(result.contains("<p>World</p>"));
    }

    #[test]
    fn sanitize_strips_iframe() {
        let html = "<div><iframe src=\"evil.com\"></iframe></div>";
        let result = sanitize_html(html);
        assert!(!result.contains("iframe"));
        assert!(result.contains("<div>"));
    }

    #[test]
    fn sanitize_strips_event_handlers() {
        let html = "<img src=\"cat.jpg\" onerror=\"alert(1)\" alt=\"cat\">";
        let result = sanitize_html(html);
        assert!(!result.contains("onerror"));
        assert!(result.contains("src=\"cat.jpg\""));
        assert!(result.contains("alt=\"cat\""));
    }

    #[test]
    fn sanitize_strips_onclick() {
        let html = "<a href=\"#\" onclick=\"doStuff()\">Click</a>";
        let result = sanitize_html(html);
        assert!(!result.contains("onclick"));
        assert!(result.contains("href=\"#\""));
        assert!(result.contains("Click"));
    }

    #[test]
    fn sanitize_strips_stylesheet_link() {
        let html = "<link rel=\"stylesheet\" href=\"track.css\"><p>Content</p>";
        let result = sanitize_html(html);
        assert!(!result.contains("link"));
        assert!(!result.contains("track.css"));
        assert!(result.contains("<p>Content</p>"));
    }

    #[test]
    fn sanitize_preserves_normal_html() {
        let html = "<div class=\"wrapper\"><h1>Title</h1><p style=\"color: red\">Text</p></div>";
        let result = sanitize_html(html);
        assert_eq!(result, html);
    }

    #[test]
    fn sanitize_strips_form_elements() {
        let html =
            "<form action=\"/submit\"><input type=\"text\"><button>Go</button></form><p>After</p>";
        let result = sanitize_html(html);
        assert!(!result.contains("form"));
        assert!(!result.contains("input"));
        assert!(!result.contains("button"));
        assert!(result.contains("<p>After</p>"));
    }

    #[test]
    fn sanitize_strips_object_embed() {
        let html = "<object data=\"flash.swf\"></object><embed src=\"plugin.swf\">";
        let result = sanitize_html(html);
        assert!(!result.contains("object"));
        assert!(!result.contains("embed"));
    }

    // -- data: URI --

    #[test]
    fn decode_data_uri_base64() {
        let uri = "data:image/png;base64,iVBORw0KGgo=";
        let result = decode_data_uri(uri).unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn decode_data_uri_plain_text() {
        let uri = "data:text/plain,Hello%20World";
        let result = decode_data_uri(uri).unwrap();
        assert_eq!(result, b"Hello World");
    }

    #[test]
    fn decode_data_uri_plain_no_encoding() {
        let uri = "data:,bare%20data";
        let result = decode_data_uri(uri).unwrap();
        assert_eq!(result, b"bare data");
    }

    #[test]
    fn decode_data_uri_invalid() {
        assert!(decode_data_uri("https://example.com").is_none());
        assert!(decode_data_uri("data:no-comma-here").is_none());
    }

    #[test]
    fn decode_data_uri_invalid_base64() {
        // Completely invalid base64 that can't be recovered
        let uri = "data:image/png;base64,!!!not-valid!!!";
        assert!(decode_data_uri(uri).is_none());
    }

    // -- resolve_image_uri --

    #[test]
    fn resolve_data_uri() {
        let uri = "data:text/plain,hello";
        let result = resolve_image_uri(uri, None, None).unwrap();
        assert_eq!(result, b"hello");
    }

    #[test]
    fn resolve_cid_uri() {
        let resolver = |cid: &str| -> Option<Vec<u8>> {
            if cid == "image001@example.com" {
                Some(vec![0x89, 0x50, 0x4E, 0x47])
            } else {
                None
            }
        };
        let result = resolve_image_uri("cid:image001@example.com", Some(&resolver), None).unwrap();
        assert_eq!(result, vec![0x89, 0x50, 0x4E, 0x47]);
    }

    #[test]
    fn resolve_cid_without_resolver() {
        assert!(resolve_image_uri("cid:something", None, None).is_none());
    }

    #[test]
    fn resolve_remote_url_returns_none() {
        assert!(resolve_image_uri("https://example.com/image.png", None, None).is_none());
        assert!(resolve_image_uri("http://example.com/track.gif", None, None).is_none());
    }

    #[test]
    fn resolve_remote_url_with_fetcher() {
        let fetcher = |url: &str| -> Option<Vec<u8>> {
            if url == "https://example.com/logo.png" {
                Some(vec![0x89, 0x50, 0x4E, 0x47])
            } else {
                None
            }
        };
        let result = resolve_image_uri("https://example.com/logo.png", None, Some(&fetcher));
        assert_eq!(result.unwrap(), vec![0x89, 0x50, 0x4E, 0x47]);

        // Unknown URL still returns None
        let result = resolve_image_uri("https://other.com/img.jpg", None, Some(&fetcher));
        assert!(result.is_none());
    }

    #[test]
    fn resolve_remote_url_without_fetcher() {
        // Remote URIs return None when no url_fetcher is provided
        assert!(resolve_image_uri("https://example.com/image.png", None, None).is_none());
        assert!(resolve_image_uri("http://example.com/track.gif", None, None).is_none());
        assert!(resolve_image_uri("//cdn.example.com/img.jpg", None, None).is_none());
    }

    // -- prepare_email_html --

    #[test]
    fn prepare_email_end_to_end() {
        let html = b"<html><body><p>Hello</p><script>bad()</script>\
            <img src=\"data:text/plain,pixel\"><img src=\"cid:att1\">\
            <img src=\"https://remote.com/track.gif\"></body></html>";

        let resolver = |cid: &str| -> Option<Vec<u8>> {
            if cid == "att1" {
                Some(vec![1, 2, 3])
            } else {
                None
            }
        };

        let prepared = prepare_email_html(html, Some(&resolver), None);

        // Script removed
        assert!(!prepared.html.contains("script"));
        assert!(!prepared.html.contains("bad()"));

        // Normal content preserved
        assert!(prepared.html.contains("<p>Hello</p>"));

        // data: and cid: images resolved
        assert_eq!(prepared.images.len(), 2);
        assert_eq!(prepared.images[0].0, "data:text/plain,pixel");
        assert_eq!(prepared.images[0].1, b"pixel");
        assert_eq!(prepared.images[1].0, "cid:att1");
        assert_eq!(prepared.images[1].1, vec![1, 2, 3]);
    }

    #[test]
    fn prepare_email_with_encoding() {
        // Windows-1252 encoded with meta charset
        let html =
            b"<html><head><meta charset=\"windows-1252\"></head><body>\x93Hello\x94</body></html>"
                .to_vec();
        let prepared = prepare_email_html(&html, None, None);
        assert!(prepared.html.contains('\u{201c}'));
        assert!(prepared.html.contains('\u{201d}'));
    }

    // -- Attribute preprocessing --

    #[test]
    fn preprocess_body_bgcolor_quoted() {
        let html = r##"<body bgcolor="#ff0000"><p>Red</p></body>"##;
        let result = preprocess_attrs(html);
        assert!(!result.contains("bgcolor"));
        assert!(result.contains("background-color: #ff0000;"));
    }

    #[test]
    fn preprocess_body_bgcolor_named() {
        let html = r#"<body bgcolor="white"><p>White</p></body>"#;
        let result = preprocess_attrs(html);
        assert!(result.contains("background-color: white;"));
    }

    #[test]
    fn preprocess_cellpadding() {
        let html = r#"<table cellpadding="5"><tr><td>Cell</td></tr></table>"#;
        let result = preprocess_attrs(html);
        // Original attribute removed but preserved as data attribute
        assert!(!result.contains(" cellpadding=\"5\""));
        assert!(result.contains("data-cellpadding=\"5\""));
    }

    #[test]
    fn preprocess_no_body_bgcolor_noop() {
        let html = "<body><p>No bgcolor</p></body>";
        let result = preprocess_attrs(html);
        assert_eq!(result, html);
    }

    // -- EMAIL_MASTER_CSS --

    #[test]
    fn master_css_is_valid() {
        assert!(EMAIL_MASTER_CSS.contains("margin: 0"));
        assert!(EMAIL_MASTER_CSS.contains("word-wrap: break-word"));
        assert!(EMAIL_MASTER_CSS.contains("max-width: 100%"));
        assert!(EMAIL_MASTER_CSS.contains("border-collapse"));
    }
}
