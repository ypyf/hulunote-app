#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum WikiToken {
    Text(String),
    Link(String),
}

/// Parse `[[Wiki Links]]` from plain text.
///
/// Rules (MVP):
/// - Only `[[...]]` is recognized.
/// - No nesting; the first `]]` closes the link.
/// - Unclosed `[[` is treated as plain text.
pub(crate) fn parse_wiki_tokens(input: &str) -> Vec<WikiToken> {
    let mut out: Vec<WikiToken> = Vec::new();
    let mut i = 0;
    let bytes = input.as_bytes();

    while i < bytes.len() {
        // Find next `[[`
        let mut start = None;
        let mut j = i;
        while j + 1 < bytes.len() {
            if bytes[j] == b'[' && bytes[j + 1] == b'[' {
                start = Some(j);
                break;
            }
            j += 1;
        }

        let Some(link_start) = start else {
            if i < bytes.len() {
                out.push(WikiToken::Text(input[i..].to_string()));
            }
            break;
        };

        if link_start > i {
            out.push(WikiToken::Text(input[i..link_start].to_string()));
        }

        // Find closing `]]`
        let mut end = None;
        let mut k = link_start + 2;
        while k + 1 < bytes.len() {
            if bytes[k] == b']' && bytes[k + 1] == b']' {
                end = Some(k);
                break;
            }
            k += 1;
        }

        let Some(link_end) = end else {
            // Unclosed link: treat the rest as text.
            out.push(WikiToken::Text(input[link_start..].to_string()));
            break;
        };

        let label = input[link_start + 2..link_end].to_string();
        out.push(WikiToken::Link(label));
        i = link_end + 2;
    }

    out
}

pub(crate) fn extract_wiki_links(input: &str) -> Vec<String> {
    parse_wiki_tokens(input)
        .into_iter()
        .filter_map(|t| match t {
            WikiToken::Link(s) => {
                // Roam-style: treat the title inside [[...]] as-is (case-sensitive, whitespace-sensitive).
                if s.is_empty() {
                    None
                } else {
                    Some(s)
                }
            }
            _ => None,
        })
        .collect()
}

pub(crate) fn normalize_roam_page_title(s: &str) -> String {
    // Roam-style uniqueness key (MVP): exact string.
    // Note: Roam historically treats leading/trailing whitespace as distinct (see issue #378).
    s.to_string()
}
