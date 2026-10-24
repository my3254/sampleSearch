use url::Url;

/// Query parameters that never change the target document.
const TRACKING: &[&str] = &[
    "utm_source",
    "utm_medium",
    "utm_campaign",
    "utm_term",
    "utm_content",
    "spm",
    "from",
    "ref",
    "referer",
    "fbclid",
    "gclid",
    "share_token",
    "scene",
    "sa",
];

/// Canonical form used as the dedupe key: host lowercased by the url crate,
/// fragment dropped, tracking params removed, remaining params re-encoded.
pub fn canonical(raw: &str) -> Option<String> {
    let mut url = Url::parse(raw.trim()).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    if url.host_str().is_none_or(str::is_empty) {
        return None;
    }
    url.set_fragment(None);

    let kept: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(k, _)| !TRACKING.contains(&k.to_lowercase().as_str()))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();

    if kept.is_empty() {
        url.set_query(None);
    } else {
        let mut ser = url::form_urlencoded::Serializer::new(String::new());
        for (k, v) in &kept {
            ser.append_pair(k, v);
        }
        let encoded = ser.finish();
        url.set_query(Some(&encoded));
    }
    Some(url.to_string())
}

pub fn domain_of(url: &str) -> String {
    Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_tracking_and_fragment() {
        let c = canonical("https://Example.com/a/b?utm_source=x&id=7&spm=abc#frag").unwrap();
        assert_eq!(c, "https://example.com/a/b?id=7");
    }

    #[test]
    fn rejects_non_http() {
        assert!(canonical("ftp://example.com/x").is_none());
        assert!(canonical("javascript:void(0)").is_none());
        assert!(canonical("not a url").is_none());
    }

    #[test]
    fn domain_extraction() {
        assert_eq!(domain_of("https://blog.csdn.net/x/y"), "blog.csdn.net");
    }
}
