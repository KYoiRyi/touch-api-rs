//! Pure helpers that mirror the TypeScript implementation.

/// FNV-1a over UTF-16 code units, exactly as `capjs-core/src/prng.js` does it.
///
/// JavaScript keeps the accumulator in a double, so every step is
/// "ToInt32 -> XOR -> add shifted int32 terms -> ToUint32". Working on `u32`
/// bit patterns reproduces that modulo-2^32 arithmetic.
pub fn fnv1a(s: &str) -> u32 {
    fnv1a_resume(2166136261, s)
}

pub fn fnv1a_resume(seed: u32, s: &str) -> u32 {
    let mut state = seed;
    for unit in s.encode_utf16() {
        state ^= unit as u32;
        let shifted = (state.wrapping_shl(1) as i32) as i64
            + (state.wrapping_shl(4) as i32) as i64
            + (state.wrapping_shl(7) as i32) as i64
            + (state.wrapping_shl(8) as i32) as i64
            + (state.wrapping_shl(24) as i32) as i64;
        let sum = state as i64 + shifted;
        state = sum.rem_euclid(1_i64 << 32) as u32;
    }
    state
}

/// `prngFromHash`: xorshift32 rendered as lowercase hex, truncated to `len`.
pub fn prng_from_hash(initial: u32, len: usize) -> String {
    let mut state = initial;
    let mut out = String::with_capacity(len + 8);
    while out.len() < len {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        out.push_str(&format!("{state:08x}"));
    }
    out.truncate(len);
    out
}

/// Port of `utils/safeUrl.ts::sanitizeUserUrl`.
pub const SAFE_LINK_PROTOCOLS: [&str; 9] = [
    "http:", "https:", "mailto:", "irc:", "ircs:", "xmpp:", "magnet:", "ed2k:", "thunder:",
];

fn explicit_protocol(value: &str) -> Option<String> {
    let idx = value.find(':')?;
    let candidate = &value[..idx];
    if candidate.is_empty() {
        return None;
    }
    let mut chars = candidate.chars();
    let first = chars.next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '.' || c == '-') {
        return None;
    }
    Some(format!("{}:", candidate.to_ascii_lowercase()))
}

/// Returns `None` when the URL must be rejected, otherwise the trimmed URL with
/// its protocol lower-cased (matching `sanitizeUserUrl`).
pub fn sanitize_user_href(url: &str) -> Option<String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed
        .chars()
        .any(|c| (c as u32) < 0x20 || (c as u32) == 0x7f)
    {
        return None;
    }
    match explicit_protocol(trimmed) {
        None => Some(trimmed.to_string()),
        Some(protocol) => {
            if SAFE_LINK_PROTOCOLS.contains(&protocol.as_str()) {
                Some(format!("{}{}", protocol, &trimmed[protocol.len()..]))
            } else {
                None
            }
        }
    }
}

/// `isHttpLikeUrl`: only `http:`, `https:` or protocol-relative URLs can be
/// bounced through the `/redirect` interstitial.
pub fn is_redirectable_url(url: &str) -> bool {
    match explicit_protocol(url) {
        Some(p) => p == "http:" || p == "https:",
        None => url.starts_with("//"),
    }
}

pub fn http_url_hostname(url: &str) -> Option<String> {
    if !is_redirectable_url(url) {
        return None;
    }
    let base = url::Url::parse("https://kun.local").ok()?;
    let parsed = base.join(url).ok()?;
    parsed.host_str().map(|h| h.to_ascii_lowercase())
}

pub fn normalize_domain(domain: &str) -> String {
    let lowered = domain.trim().to_ascii_lowercase();
    let without_scheme = if let Some(rest) = lowered.strip_prefix("https://") {
        rest
    } else if let Some(rest) = lowered.strip_prefix("http://") {
        rest
    } else {
        &lowered
    };
    let without_path = without_scheme.split('/').next().unwrap_or("");
    without_path.trim_start_matches('.').to_string()
}

/// `isHostnameExcluded`: the site keeps its own domains off the interstitial.
pub fn is_hostname_excluded(hostname: &str, domains: &[String]) -> bool {
    domains.iter().any(|domain| {
        let normalized = normalize_domain(domain);
        !normalized.is_empty()
            && (hostname == normalized || hostname.ends_with(&format!(".{normalized}")))
    })
}

/// Host that serves the official TouchGal pan.
pub const PAN_HOST: &str = "pan.touchgal.net";

/// Extracts the share key from an official `https://pan.touchgal.net/s/<key>` link.
///
/// The host check matters: community mirrors such as `pan.quark.cn/s/<id>` use the
/// same `/s/` path shape but must not be treated as pan shares.
pub fn pan_share_key(content: &str) -> Option<String> {
    let parsed = url::Url::parse(content).ok()?;
    if parsed.host_str()? != PAN_HOST {
        return None;
    }
    let mut segments = parsed.path_segments()?;
    if segments.next()? != "s" {
        return None;
    }
    let key = segments.next()?.trim().to_string();
    if key.is_empty() {
        None
    } else {
        Some(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a_matches_javascript_reference_vector() {
        // Produced by running capjs-core's prng.js on the same token.
        let token = concat!(
            "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.",
            "eyJuIjoiODI0MGIxNmQwN2RiNWVhNzY2OTMxMDVmMjUyM2ZlZDY3MDBkNTM4YzQ4YTc2NzlhYTEiLCJjIjo1MCwicyI6MzIsImQiOjQsImV4cCI6MTc5MjY3ODQ4NjY2MywiaWF0IjoxNzkyNjc3ODg2NjYzLCJzayI6Imt1bi1jYXB0Y2hhIn0.",
            "WA-iRUHn_Xpr1fBM-Pt1xJt8idYb-huE8GMrRe4SnHU"
        );
        assert_eq!(fnv1a(token), 3316059769);
        let salt_seed = fnv1a_resume(3316059769, "1");
        assert_eq!(salt_seed, 1845197656);
        assert_eq!(
            prng_from_hash(salt_seed, 32),
            "509f38104431181e816f9b67326b0ea6"
        );
        let target_seed = fnv1a_resume(salt_seed, "d");
        assert_eq!(target_seed, 1591934836);
        assert_eq!(prng_from_hash(target_seed, 4), "2083");
    }

    #[test]
    fn sanitize_and_redirect_rules() {
        assert_eq!(
            sanitize_user_href("  HTTPS://Example.com/x "),
            Some("https://Example.com/x".into())
        );
        assert_eq!(sanitize_user_href("javascript:alert(1)"), None);
        assert_eq!(
            sanitize_user_href("magnet:?xt=urn:btih:abc"),
            Some("magnet:?xt=urn:btih:abc".into())
        );
        assert!(is_redirectable_url("https://pan.touchgal.net/s/abc"));
        assert!(!is_redirectable_url("magnet:?xt=urn:btih:abc"));
        assert!(is_hostname_excluded(
            "www.touchgal.ink",
            &["touchgal.ink".to_string()]
        ));
        assert!(!is_hostname_excluded(
            "pan.touchgal.net",
            &["touchgal.ink".to_string()]
        ));
    }

    #[test]
    fn pan_key_extraction() {
        assert_eq!(
            pan_share_key("https://pan.touchgal.net/s/5yqBTE").as_deref(),
            Some("5yqBTE")
        );
        assert_eq!(pan_share_key("https://pan.quark.cn/s/4fab5cb304f8"), None);
    }
}
