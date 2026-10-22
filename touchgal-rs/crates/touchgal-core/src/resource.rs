//! `GET /api/patch/resource?patchId=…` and the download-counter endpoint.
//!
//! `app/api/patch/resource/route.ts` -> `getPatchResource` returns every public
//! resource (`status = 0`) for a patch, each with its `links[]`. The resource
//! list is what the download tab renders, and `links[].storage` decides where a
//! link actually points:
//!
//! | storage   | meaning                                                        |
//! |-----------|----------------------------------------------------------------|
//! | `touchgal`| official pan link (`https://pan.touchgal.net/s/<key>`)          |
//! | `s3`      | object storage; `content` is a direct presigned-ish file URL    |
//! | `user`    | community mirror (Quark / Baidu / 139 / …), `content` as-is     |
//!
//! `PUT /api/patch/resource/download` is the counter the UI fires when a link is
//! clicked; it is anonymous-safe and deduplicated per identity + link for
//! `DOWNLOAD_DEDUP_CACHE_DURATION`, so repeated calls are free.

use crate::client::{Origin, TouchgalClient};
use crate::models::PatchResource;
use crate::util::{is_hostname_excluded, is_redirectable_url, sanitize_user_href};
use crate::Error;

pub const STORAGE_TOUCHGAL: &str = "touchgal";
/// Site origin used to build the `/redirect` interstitial URL.
pub const SITE_BASE: &str = "https://www.touchgal.ink";
pub const STORAGE_S3: &str = "s3";
pub const STORAGE_USER: &str = "user";

/// Fetches the public resource list of a patch.
pub async fn list(client: &TouchgalClient, patch_id: i64) -> crate::Result<Vec<PatchResource>> {
    if patch_id <= 0 {
        return Err(Error::InvalidArgument("patchId 必须为正数".into()));
    }
    let path = format!("/api/patch/resource?patchId={patch_id}");
    client.json(client.get(Origin::Site, &path)).await
}

/// Increments the download counter (anonymous is fine).
pub async fn count_download(
    client: &TouchgalClient,
    patch_id: i64,
    resource_id: i64,
    link_id: i64,
) -> crate::Result<()> {
    let body = serde_json::json!({
        "patchId": patch_id,
        "resourceId": resource_id,
        "linkId": link_id,
    });
    let _: serde_json::Value = client
        .json(
            client
                .put(Origin::Site, "/api/patch/resource/download")
                .json(&body),
        )
        .await?;
    Ok(())
}

/// Only links whose `storage` is the official pan.
pub fn official_resources(resources: &[PatchResource]) -> Vec<PatchResource> {
    resources
        .iter()
        .filter(|resource| {
            resource
                .links
                .iter()
                .any(|link| link.storage == STORAGE_TOUCHGAL)
        })
        .cloned()
        .collect()
}

/// Where a resource link resolves to, mirroring `components/kun/external-link`.
///
/// Official pan links are bounced through the interstitial too (verified in the
/// browser: clicking the 8.4 GB link navigates to
/// `/redirect?url=https%3A%2F%2Fpan.touchgal.net%2Fs%2F5yqBTE`), so both http
/// variants carry the redirect decision.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LinkTarget {
    /// Official pan share; `key` is the Cloudreve share key.
    PanShare {
        key: String,
        /// The pan URL itself.
        url: String,
        /// The `/redirect?url=…` hop the UI inserts, when enabled.
        #[serde(skip_serializing_if = "Option::is_none")]
        interstitial_url: Option<String>,
        /// Seconds the interstitial blocks its button (`delaySeconds`).
        delay_seconds: u64,
    },
    /// Any other http(s) target.
    External {
        url: String,
        via_redirect: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        interstitial_url: Option<String>,
        delay_seconds: u64,
    },
    /// Non-http schemes the site still allows (magnet:, ed2k:, …).
    Scheme { url: String },
    /// Rejected by `sanitizeUserHref`.
    Blocked { raw: String },
}

impl LinkTarget {
    /// Final URL the user ends up at after any interstitial.
    pub fn final_url(&self) -> Option<&str> {
        match self {
            LinkTarget::PanShare { url, .. } => Some(url),
            LinkTarget::External { url, .. } => Some(url),
            LinkTarget::Scheme { url } => Some(url),
            LinkTarget::Blocked { .. } => None,
        }
    }
}

/// Resolves one link the way the browser would.
///
/// `enable_redirect` / `excluded_domains` / `delay_seconds` come from the user's
/// `UserState` (`config/redirect.json` defaults: enabled, `["touchgal.ink",
/// "kungal.com"]`, 5 s). The site only shows the interstitial for http(s) links
/// whose host is not excluded.
pub fn resolve_link(
    content: &str,
    enable_redirect: bool,
    excluded_domains: &[String],
    delay_seconds: u64,
) -> LinkTarget {
    let Some(safe) = sanitize_user_href(content) else {
        return LinkTarget::Blocked {
            raw: content.to_string(),
        };
    };
    if !is_redirectable_url(&safe) {
        return LinkTarget::Scheme { url: safe };
    }
    let hostname = crate::util::http_url_hostname(&safe);
    let excluded = hostname
        .as_deref()
        .map(|host| is_hostname_excluded(host, excluded_domains))
        .unwrap_or(false);
    let via_redirect = enable_redirect && !excluded;
    let interstitial_url =
        via_redirect.then(|| format!("{}/redirect?url={}", SITE_BASE, percent_encode(&safe)));
    if let Some(key) = crate::util::pan_share_key(&safe) {
        return LinkTarget::PanShare {
            key,
            url: safe,
            interstitial_url,
            delay_seconds,
        };
    }
    LinkTarget::External {
        url: safe,
        via_redirect,
        interstitial_url,
        delay_seconds,
    }
}

/// `encodeURIComponent`-compatible encoder for the `/redirect?url=` parameter.
pub fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.as_bytes() {
        let c = *byte as char;
        let unreserved = c.is_ascii_alphanumeric()
            || matches!(c, '-' | '_' | '.' | '!' | '~' | '*' | '\'' | '(' | ')');
        if unreserved {
            out.push(c);
        } else {
            out.push_str(&format!("%{:02X}", byte));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn excluded() -> Vec<String> {
        vec!["touchgal.ink".into(), "kungal.com".into()]
    }

    #[test]
    fn official_pan_link_keeps_its_interstitial() {
        // Captured from the browser: the 8.4 GB official link navigates to
        // /redirect?url=https%3A%2F%2Fpan.touchgal.net%2Fs%2F5yqBTE
        let target = resolve_link("https://pan.touchgal.net/s/5yqBTE", true, &excluded(), 5);
        match target {
            LinkTarget::PanShare {
                key,
                url,
                interstitial_url,
                delay_seconds,
            } => {
                assert_eq!(key, "5yqBTE");
                assert_eq!(url, "https://pan.touchgal.net/s/5yqBTE");
                assert_eq!(
                    interstitial_url.as_deref(),
                    Some("https://www.touchgal.ink/redirect?url=https%3A%2F%2Fpan.touchgal.net%2Fs%2F5yqBTE")
                );
                assert_eq!(delay_seconds, 5);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn community_links_go_through_the_interstitial() {
        let target = resolve_link("https://pan.quark.cn/s/4fab5cb304f8", true, &excluded(), 5);
        match target {
            LinkTarget::External {
                url,
                via_redirect,
                interstitial_url,
                ..
            } => {
                assert!(via_redirect);
                assert_eq!(url, "https://pan.quark.cn/s/4fab5cb304f8");
                assert_eq!(
                    interstitial_url.as_deref(),
                    Some("https://www.touchgal.ink/redirect?url=https%3A%2F%2Fpan.quark.cn%2Fs%2F4fab5cb304f8")
                );
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn own_domains_and_disabled_redirect_skip_the_interstitial() {
        match resolve_link("https://www.touchgal.ink/doc", true, &excluded(), 5) {
            LinkTarget::External {
                url,
                via_redirect,
                interstitial_url,
                ..
            } => {
                assert!(!via_redirect);
                assert_eq!(url, "https://www.touchgal.ink/doc");
                assert!(interstitial_url.is_none());
            }
            other => panic!("unexpected {other:?}"),
        }
        match resolve_link("https://pan.quark.cn/s/abc", false, &excluded(), 5) {
            LinkTarget::External {
                via_redirect,
                interstitial_url,
                ..
            } => {
                assert!(!via_redirect);
                assert!(interstitial_url.is_none());
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn unsafe_schemes_are_blocked() {
        assert!(matches!(
            resolve_link("javascript:alert(1)", true, &excluded(), 5),
            LinkTarget::Blocked { .. }
        ));
        assert!(matches!(
            resolve_link("magnet:?xt=urn:btih:abc", true, &excluded(), 5),
            LinkTarget::Scheme { .. }
        ));
    }
}
