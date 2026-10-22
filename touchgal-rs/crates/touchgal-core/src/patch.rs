//! `GET /api/patch?uniqueId=…` — the patch (galgame) detail payload.
//!
//! Route handler: `app/api/patch/route.ts` -> `getPatchById` (Redis single-flight
//! cache, falls through to Prisma). The `uniqueId` is exactly 8 characters and is
//! what the site uses in page URLs (`/{uniqueId}`).

use crate::client::{Origin, TouchgalClient};
use crate::models::Patch;
use crate::Error;

/// Fetches a patch by its 8-character `uniqueId`.
pub async fn get(client: &TouchgalClient, unique_id: &str) -> crate::Result<Patch> {
    let unique_id = unique_id.trim();
    if unique_id.chars().count() != 8 {
        return Err(Error::InvalidArgument("uniqueId 必须为 8 个字符".into()));
    }
    let path = format!("/api/patch?uniqueId={}", unique_id);
    client.json(client.get(Origin::Site, &path)).await
}

/// Extracts the `uniqueId` from any of the URL shapes the site uses:
/// `/{uniqueId}`, `/{uniqueId}/resource/{id}`, or a full URL.
pub fn unique_id_from_url(url: &str) -> Option<String> {
    let candidate = if url.starts_with("http://") || url.starts_with("https://") {
        url::Url::parse(url).ok()?.path().to_string()
    } else {
        url.to_string()
    };
    for segment in candidate.split('/') {
        let segment = segment.split(['?', '#']).next().unwrap_or("");
        if segment.len() == 8 && segment.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Some(segment.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_unique_ids() {
        assert_eq!(unique_id_from_url("a555829b").as_deref(), Some("a555829b"));
        assert_eq!(
            unique_id_from_url("https://www.touchgal.ink/a555829b?tab=resources").as_deref(),
            Some("a555829b")
        );
        assert_eq!(
            unique_id_from_url("https://www.touchgal.ink/a555829b/resource/9094").as_deref(),
            Some("a555829b")
        );
        assert_eq!(unique_id_from_url("https://www.touchgal.ink/galgame"), None);
    }
}
