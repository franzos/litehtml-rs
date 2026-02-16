//! Email-specific defaults for HTML rendering.
//!
//! Provides an email user-agent stylesheet and a convenience pipeline
//! that wraps [`crate::html::prepare_html`] with email-specific defaults.

use crate::html;

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
// Email preprocessing pipeline
// ---------------------------------------------------------------------------

/// Convenience wrapper around [`html::prepare_html`] for email content.
///
/// Identical to `prepare_html` but returns [`PreparedEmail`] for clarity
/// in email-specific codebases.
pub fn prepare_email_html(
    raw: &[u8],
    cid_resolver: Option<&dyn Fn(&str) -> Option<Vec<u8>>>,
    url_fetcher: Option<&dyn Fn(&str) -> Option<Vec<u8>>>,
) -> PreparedEmail {
    let prepared = html::prepare_html(raw, cid_resolver, url_fetcher);
    PreparedEmail {
        html: prepared.html,
        images: prepared.images,
    }
}

/// Preprocessed email ready for rendering.
///
/// Wraps [`html::PreparedHtml`] with an email-specific name.
#[derive(Debug, Clone)]
pub struct PreparedEmail {
    /// Sanitized, UTF-8 HTML with image URIs intact.
    pub html: String,
    /// Resolved images: `(original_uri, decoded_bytes)`.
    pub images: Vec<(String, Vec<u8>)>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn master_css_is_valid() {
        assert!(EMAIL_MASTER_CSS.contains("margin: 0"));
        assert!(EMAIL_MASTER_CSS.contains("word-wrap: break-word"));
        assert!(EMAIL_MASTER_CSS.contains("max-width: 100%"));
        assert!(EMAIL_MASTER_CSS.contains("border-collapse"));
    }

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

        assert!(!prepared.html.contains("script"));
        assert!(prepared.html.contains("<p>Hello</p>"));
        assert_eq!(prepared.images.len(), 2);
    }
}
