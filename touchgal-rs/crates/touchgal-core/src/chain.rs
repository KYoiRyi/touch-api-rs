//! End-to-end download chain reconstruction.
//!
//! The site renders this path as: search / browse -> patch page -> resource tab
//! -> expand a resource card -> click a link -> `/redirect?url=…` (5 s
//! interstitial) -> `pan.touchgal.net/s/<key>` -> pick a file -> presigned S3 URL.
//!
//! `resolve_download` walks the same path programmatically and reports every hop
//! so a caller can display (or automate) it. Resource rows already carry
//! `uniqueId`, which is what `GET /api/patch` needs, so the chain never has to
//! guess an id-to-uniqueId mapping.

use crate::client::TouchgalClient;
use crate::models::{Patch, PatchResource};
use crate::pan::{self, PanObject, ShareInfo};
use crate::resource::{self, LinkTarget};
use crate::search::{self, SearchQuery};
use crate::{patch, Error};

/// One resolved link inside a resource.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ResolvedLink {
    pub link_id: i64,
    pub storage: String,
    pub size: String,
    pub code: String,
    pub password: String,
    pub hash: String,
    pub content: String,
    /// Where the browser would send the user.
    pub target: LinkTarget,
    /// Files behind an official pan share (empty for community mirrors).
    pub files: Vec<PanFile>,
    pub share: Option<ShareInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PanFile {
    pub id: String,
    pub name: String,
    pub path: String,
    pub size: i64,
    pub modified: String,
    /// Presigned URL, filled when [`ResolveOptions::fetch_urls`] is set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
}

impl From<PanObject> for PanFile {
    fn from(object: PanObject) -> Self {
        let path = object.full_path();
        Self {
            id: object.id,
            name: object.name,
            path,
            size: object.size,
            modified: object.date,
            download_url: None,
        }
    }
}

/// One resource with its resolved links.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ResolvedResource {
    pub id: i64,
    pub name: String,
    pub section: String,
    pub r#type: Vec<String>,
    pub language: Vec<String>,
    pub platform: Vec<String>,
    pub note: String,
    pub download: i64,
    pub uploader: Option<String>,
    pub created: String,
    pub links: Vec<ResolvedLink>,
}

/// The full chain for one patch.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DownloadChain {
    pub patch_id: i64,
    pub unique_id: String,
    pub name: String,
    pub content_limit: String,
    pub banner: String,
    pub resources: Vec<ResolvedResource>,
    pub official_resource_count: usize,
}

#[derive(Debug, Clone)]
pub struct ResolveOptions {
    /// Populate presigned URLs for every file (one extra request per file).
    pub fetch_urls: bool,
    /// Only keep resources whose links include the official pan.
    pub official_only: bool,
    /// Fire the download counter PUT for every link (deduplicated server-side).
    pub count_downloads: bool,
    pub enable_redirect: bool,
    pub delay_seconds: u64,
    pub excluded_domains: Vec<String>,
    /// Depth limit when walking folder shares.
    pub max_depth: usize,
}

impl Default for ResolveOptions {
    fn default() -> Self {
        Self {
            fetch_urls: false,
            official_only: false,
            count_downloads: false,
            enable_redirect: true,
            delay_seconds: 5,
            excluded_domains: vec!["touchgal.ink".into(), "kungal.com".into()],
            max_depth: 4,
        }
    }
}

impl ResolveOptions {
    pub fn official() -> Self {
        Self {
            official_only: true,
            ..Default::default()
        }
    }

    pub fn with_urls(mut self) -> Self {
        self.fetch_urls = true;
        self
    }

    pub fn counting(mut self) -> Self {
        self.count_downloads = true;
        self
    }
}

/// Resolves the download chain for a patch id.
pub async fn resolve_download(
    client: &TouchgalClient,
    patch_id: i64,
    options: ResolveOptions,
) -> crate::Result<DownloadChain> {
    let resources = resource::list(client, patch_id).await?;
    build_chain(client, patch_id, resources, options).await
}

/// Resolves the download chain starting from a search keyword.
pub async fn resolve_by_keyword(
    client: &TouchgalClient,
    keyword: &str,
    options: ResolveOptions,
) -> crate::Result<DownloadChain> {
    let results = search::search(client, &SearchQuery::keyword(keyword)).await?;
    let first = results
        .galgames
        .first()
        .ok_or_else(|| Error::Business(format!("未找到与 {keyword} 匹配的 Galgame")))?;
    resolve_download(client, first.id, options).await
}

/// Resolves the download chain for a patch unique id.
pub async fn resolve_by_unique_id(
    client: &TouchgalClient,
    unique_id: &str,
    options: ResolveOptions,
) -> crate::Result<DownloadChain> {
    let detail = patch::get(client, unique_id).await?;
    resolve_download(client, detail.id, options).await
}

async fn build_chain(
    client: &TouchgalClient,
    patch_id: i64,
    resources: Vec<PatchResource>,
    options: ResolveOptions,
) -> crate::Result<DownloadChain> {
    let unique_id = resources
        .first()
        .map(|resource| resource.unique_id.clone())
        .filter(|id| id.chars().count() == 8)
        .ok_or_else(|| Error::Business("该 Galgame 暂无公开资源".into()))?;
    let detail: Patch = patch::get(client, &unique_id).await?;

    let mut resolved = Vec::new();
    for resource in resources {
        if options.official_only
            && !resource
                .links
                .iter()
                .any(|link| link.storage == resource::STORAGE_TOUCHGAL)
        {
            continue;
        }
        let mut links = Vec::new();
        for link in &resource.links {
            let target = resource::resolve_link(
                &link.content,
                options.enable_redirect,
                &options.excluded_domains,
                options.delay_seconds,
            );
            let mut files = Vec::new();
            let mut share = None;
            let mut error = None;
            if let LinkTarget::PanShare { key, .. } = &target {
                match pan::share_info(client, key).await {
                    Ok(info) => {
                        let is_dir = info.is_dir;
                        let fallback_name = info
                            .source
                            .as_ref()
                            .map(|source| source.name.clone())
                            .unwrap_or_default();
                        share = Some(info);
                        let listing = if is_dir {
                            pan::list_files_recursive(client, key, options.max_depth).await
                        } else {
                            Ok(vec![PanObject {
                                id: String::new(),
                                name: fallback_name,
                                path: "/".into(),
                                size: 0,
                                r#type: "file".into(),
                                date: String::new(),
                                create_date: String::new(),
                                key: key.clone(),
                            }])
                        };
                        match listing {
                            Ok(objects) => {
                                files = objects.into_iter().map(PanFile::from).collect();
                            }
                            Err(e) => error = Some(e.to_string()),
                        }
                    }
                    Err(e) => error = Some(e.to_string()),
                }
                if options.fetch_urls && error.is_none() {
                    for file in files.iter_mut() {
                        let path = if file.path.is_empty() || file.path == "/" {
                            None
                        } else {
                            Some(file.path.as_str())
                        };
                        match pan::download_url(client, key, path).await {
                            Ok(url) => file.download_url = Some(url),
                            Err(e) => error = Some(e.to_string()),
                        }
                    }
                }
            }
            if options.count_downloads {
                let _ = resource::count_download(client, patch_id, resource.id, link.id).await;
            }
            links.push(ResolvedLink {
                link_id: link.id,
                storage: link.storage.clone(),
                size: link.size.clone(),
                code: link.code.clone(),
                password: link.password.clone(),
                hash: link.hash.clone(),
                content: link.content.clone(),
                target,
                files,
                share,
                error,
            });
        }
        resolved.push(ResolvedResource {
            id: resource.id,
            name: resource.name,
            section: resource.section,
            r#type: resource.r#type,
            language: resource.language,
            platform: resource.platform,
            note: resource.note,
            download: resource.download,
            uploader: resource.user.map(|user| user.name),
            created: resource.created,
            links,
        });
    }
    let official_resource_count = resolved
        .iter()
        .filter(|resource| {
            resource
                .links
                .iter()
                .any(|link| link.storage == resource::STORAGE_TOUCHGAL)
        })
        .count();
    Ok(DownloadChain {
        patch_id: detail.id,
        unique_id: detail.unique_id,
        name: detail.name,
        content_limit: detail.content_limit,
        banner: detail.banner,
        resources: resolved,
        official_resource_count,
    })
}
