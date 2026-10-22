//! `pan.touchgal.net` — the official TouchGal resource pan.
//!
//! The pan is a Cloudreve v3 deployment. Anonymous share endpoints were verified
//! directly against the live host:
//!
//! * `GET  /api/v3/share/info/{key}`      -> share metadata (`is_dir`, `locked`, …)
//! * `GET  /api/v3/share/list/{key}%2F`   -> root directory listing (`objects`)
//! * `PUT  /api/v3/share/download/{key}?path=<encoded>` -> presigned S3 URL
//!
//! The download endpoint accepts both single-file shares (`is_dir = false`, no
//! `path`) and one file inside a folder share, and answers with a signed
//! `touchgaldownload.xyz` URL valid for one hour.

use crate::client::{Origin, TouchgalClient};
use crate::Error;

/// Share metadata (`data` of `/api/v3/share/info`).
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ShareInfo {
    pub key: String,
    #[serde(default)]
    pub locked: bool,
    #[serde(default, rename = "is_dir")]
    pub is_dir: bool,
    #[serde(default, rename = "create_date")]
    pub create_date: String,
    #[serde(default)]
    pub downloads: i64,
    #[serde(default)]
    pub views: i64,
    #[serde(default)]
    pub expire: i64,
    #[serde(default)]
    pub preview: bool,
    #[serde(default)]
    pub creator: Option<ShareCreator>,
    #[serde(default)]
    pub source: Option<ShareSource>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ShareCreator {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub nick: String,
    #[serde(default, rename = "group_name")]
    pub group_name: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ShareSource {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub size: i64,
}

/// One entry of a share directory listing.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PanObject {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub size: i64,
    #[serde(default)]
    pub r#type: String,
    #[serde(default)]
    pub date: String,
    #[serde(default, rename = "create_date")]
    pub create_date: String,
    #[serde(default)]
    pub key: String,
}

impl PanObject {
    pub fn is_dir(&self) -> bool {
        self.r#type == "dir"
    }

    /// Absolute path used by the download endpoint.
    pub fn full_path(&self) -> String {
        if self.path == "/" || self.path.is_empty() {
            format!("/{}", self.name)
        } else {
            format!("{}/{}", self.path.trim_end_matches('/'), self.name)
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ShareList {
    #[serde(default)]
    pub objects: Vec<PanObject>,
}

/// `GET /api/v3/share/info/{key}`.
pub async fn share_info(client: &TouchgalClient, key: &str) -> crate::Result<ShareInfo> {
    let path = format!("/api/v3/share/info/{}", encode_key(key));
    client.json_envelope(client.get(Origin::Pan, &path)).await
}

/// `GET /api/v3/share/list/{key}/?path=…`; `path` defaults to the share root.
pub async fn list(
    client: &TouchgalClient,
    key: &str,
    path: Option<&str>,
) -> crate::Result<ShareList> {
    let mut url = format!("/api/v3/share/list/{}/", encode_key(key));
    if let Some(p) = path {
        if !p.is_empty() && p != "/" {
            url.push_str(&format!("?path={}", encode_path(p)));
        }
    }
    client.json_envelope(client.get(Origin::Pan, &url)).await
}

/// `PUT /api/v3/share/download/{key}?path=…` -> presigned download URL.
pub async fn download_url(
    client: &TouchgalClient,
    key: &str,
    path: Option<&str>,
) -> crate::Result<String> {
    let mut url = format!("/api/v3/share/download/{}", encode_key(key));
    if let Some(p) = path {
        if !p.is_empty() {
            url.push_str(&format!("?path={}", encode_path(p)));
        }
    }
    let data: String = client.json_envelope(client.put(Origin::Pan, &url)).await?;
    if data.is_empty() {
        return Err(Error::Decode("empty download url".into()));
    }
    Ok(data)
}

/// Recursively expands a share into downloadable file entries.
///
/// Folder shares list their children one level at a time; the pan has no
/// archive endpoint on this deployment (`/share/archive` answers 40007), so the
/// client must enumerate files and download them individually.
pub async fn list_files_recursive(
    client: &TouchgalClient,
    key: &str,
    max_depth: usize,
) -> crate::Result<Vec<PanObject>> {
    let mut out = Vec::new();
    let mut queue = vec![(String::from("/"), 0usize)];
    while let Some((path, depth)) = queue.pop() {
        let listing = list(client, key, Some(&path)).await?;
        for object in listing.objects {
            if object.is_dir() {
                if depth < max_depth {
                    queue.push((object.full_path(), depth + 1));
                }
            } else {
                out.push(object);
            }
        }
    }
    Ok(out)
}

fn encode_key(key: &str) -> String {
    percent_encode(key, false)
}

fn encode_path(path: &str) -> String {
    percent_encode(path, true)
}

/// Minimal percent-encoder matching `encodeURIComponent`.
fn percent_encode(input: &str, keep_slash: bool) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.as_bytes() {
        let c = *byte as char;
        let unreserved = c.is_ascii_alphanumeric()
            || matches!(c, '-' | '_' | '.' | '!' | '~' | '*' | '\'' | '(' | ')');
        if unreserved || (keep_slash && c == '/') {
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

    #[test]
    fn encodes_paths_like_encode_uri_component() {
        assert_eq!(
            encode_path("/千恋＊万花.zip"),
            "/%E5%8D%83%E6%81%8B%EF%BC%8A%E4%B8%87%E8%8A%B1.zip"
        );
        assert_eq!(encode_key("5yqBTE"), "5yqBTE");
    }

    #[test]
    fn full_path_joins_root_and_nested_entries() {
        let root = PanObject {
            id: "a".into(),
            name: "x.rar".into(),
            path: "/".into(),
            size: 1,
            r#type: "file".into(),
            date: String::new(),
            create_date: String::new(),
            key: "k".into(),
        };
        assert_eq!(root.full_path(), "/x.rar");
        let nested = PanObject {
            path: "/sub".into(),
            ..root
        };
        assert_eq!(nested.full_path(), "/sub/x.rar");
    }

    #[test]
    fn parses_real_share_info_payload() {
        // Verbatim response captured from pan.touchgal.net for share 5yqBTE.
        let raw = r#"{"key":"5yqBTE","locked":false,"is_dir":true,"create_date":"2025-01-24T10:21:56+08:00","downloads":75662,"views":171995,"expire":0,"preview":false,"creator":{"key":"7NuO","nick":"Palentum","group_name":"Admin"},"source":{"name":"Senren  Banka","size":0}}"#;
        let info: ShareInfo = serde_json::from_str(raw).unwrap();
        assert!(info.is_dir);
        assert!(!info.locked);
        assert_eq!(info.creator.unwrap().nick, "Palentum");
    }
}
