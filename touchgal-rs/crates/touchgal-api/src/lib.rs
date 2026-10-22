//! HTTP surface over [`touchgal_core`], so other applications can call the
//! reconstructed TouchGal download chain without embedding the crawler.
//!
//! All routes are `GET` and JSON-in/JSON-out; every response is either
//! `{"ok":true,"data":…}` or `{"ok":false,"error":…,"kind":…}`.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;
use touchgal_core::{
    auth, chain,
    client::{ClientConfig, TouchgalClient},
    pan, patch, resource, search, Error,
};

#[derive(Clone)]
pub struct AppState {
    pub client: Arc<TouchgalClient>,
}

pub fn router(client: TouchgalClient) -> Router {
    let state = AppState {
        client: Arc::new(client),
    };
    Router::new()
        .route("/healthz", get(health))
        .route("/v1/search", get(api_search))
        .route("/v1/suggest", get(api_suggest))
        .route("/v1/browse", get(api_browse))
        .route("/v1/patch", get(api_patch))
        .route("/v1/resources", get(api_resources))
        .route("/v1/pan/info", get(api_pan_info))
        .route("/v1/pan/list", get(api_pan_list))
        .route("/v1/pan/download", get(api_pan_download))
        .route("/v1/chain", get(api_chain))
        .route("/v1/login", get(api_login))
        .with_state(state)
}

#[derive(Debug, serde::Serialize)]
struct ApiError {
    ok: bool,
    error: String,
    kind: &'static str,
}

/// Local wrapper so the core error can implement [`IntoResponse`]
/// (the orphan rule forbids implementing a foreign trait for a foreign type).
#[derive(Debug)]
pub struct ApiFailure(pub Error);

impl From<Error> for ApiFailure {
    fn from(error: Error) -> Self {
        Self(error)
    }
}

impl IntoResponse for ApiFailure {
    fn into_response(self) -> Response {
        let self_ = self.0;
        let (status, kind) = match &self_ {
            Error::Business(_) => (StatusCode::OK, "business"),
            Error::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            Error::InvalidArgument(_) => (StatusCode::BAD_REQUEST, "invalid_argument"),
            Error::CloudflareBlocked { .. } => (StatusCode::BAD_GATEWAY, "cloudflare"),
            Error::Transport(_) => (StatusCode::BAD_GATEWAY, "transport"),
            Error::Decode(_) => (StatusCode::BAD_GATEWAY, "decode"),
            Error::Status { .. } => (StatusCode::BAD_GATEWAY, "upstream_status"),
            Error::Captcha(_) => (StatusCode::BAD_GATEWAY, "captcha"),
        };
        (
            status,
            Json(ApiError {
                ok: false,
                error: self_.to_string(),
                kind,
            }),
        )
            .into_response()
    }
}

type ApiResult = Result<Json<serde_json::Value>, ApiFailure>;

fn ok<T: serde::Serialize>(value: T) -> ApiResult {
    Ok(Json(
        serde_json::to_value(value).unwrap_or(serde_json::Value::Null),
    ))
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({
        "ok": true,
        "service": "touchgal-api",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

#[derive(Debug, Deserialize)]
pub struct SearchParams {
    /// Free-text keyword. Repeatable through comma separation.
    pub q: Option<String>,
    #[serde(default)]
    pub page: Option<u32>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub sort_field: Option<String>,
    #[serde(default)]
    pub sort_order: Option<String>,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    /// JSON array of suggestion objects, taking precedence over `q`.
    #[serde(default)]
    pub suggestions: Option<String>,
    #[serde(default)]
    pub years: Option<String>,
    #[serde(default)]
    pub months: Option<String>,
    #[serde(default)]
    pub min_rating_count: Option<u32>,
    #[serde(default)]
    pub search_in_alias: Option<bool>,
    #[serde(default)]
    pub search_in_introduction: Option<bool>,
    #[serde(default)]
    pub search_in_tag: Option<bool>,
}

fn parse_sort(
    field: Option<String>,
    order: Option<String>,
) -> (search::SortField, search::SortOrder) {
    let field = match field.as_deref() {
        Some("created") => search::SortField::Created,
        Some("rating") => search::SortField::Rating,
        Some("view") => search::SortField::View,
        Some("download") => search::SortField::Download,
        Some("favorite") => search::SortField::Favorite,
        _ => search::SortField::ResourceUpdateTime,
    };
    let order = match order.as_deref() {
        Some("asc") => search::SortOrder::Asc,
        _ => search::SortOrder::Desc,
    };
    (field, order)
}

async fn api_search(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> ApiResult {
    let mut query = search::SearchQuery::default()
        .with_page(params.page.unwrap_or(1))
        .with_limit(params.limit.unwrap_or(12));
    let (field, order) = parse_sort(params.sort_field, params.sort_order);
    query = query.with_sort(field, order);
    if let Some(platform) = params.platform {
        query = query.with_platform(platform);
    }
    if let Some(kind) = params.r#type {
        query = query.with_type(kind);
    }
    if let Some(language) = params.language {
        query = query.with_language(language);
    }
    if let Some(count) = params.min_rating_count {
        query = query.with_min_rating_count(count);
    }
    if let Some(years) = params.years {
        query = query.with_years(split_list(&years));
    }
    if let Some(months) = params.months {
        query.selected_months = split_list(&months);
    }
    if let Some(flag) = params.search_in_alias {
        query.options.search_in_alias = flag;
    }
    if let Some(flag) = params.search_in_introduction {
        query.options.search_in_introduction = flag;
    }
    if let Some(flag) = params.search_in_tag {
        query.options.search_in_tag = flag;
    }
    if let Some(raw) = params.suggestions {
        let parsed: Vec<search::Suggestion> = serde_json::from_str(&raw)
            .map_err(|e| Error::InvalidArgument(format!("suggestions: {e}")))?;
        query = query.with_suggestions(parsed);
    } else if let Some(q) = params.q {
        query = query.with_suggestions(
            q.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(search::Suggestion::keyword)
                .collect(),
        );
    }
    ok(search::search(&state.client, &query).await?)
}

fn split_list(raw: &str) -> Vec<String> {
    let values: Vec<String> = raw
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    if values.is_empty() {
        vec!["all".into()]
    } else {
        values
    }
}

#[derive(Debug, Deserialize)]
pub struct SuggestParams {
    pub q: String,
}

async fn api_suggest(
    State(state): State<AppState>,
    Query(params): Query<SuggestParams>,
) -> ApiResult {
    let keywords: Vec<String> = params
        .q
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    ok(search::suggest(&state.client, &keywords).await?)
}

#[derive(Debug, Deserialize)]
pub struct BrowseParams {
    #[serde(default)]
    pub page: Option<u32>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub sort_field: Option<String>,
    #[serde(default)]
    pub sort_order: Option<String>,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
}

async fn api_browse(
    State(state): State<AppState>,
    Query(params): Query<BrowseParams>,
) -> ApiResult {
    let (field, order) = parse_sort(params.sort_field, params.sort_order);
    let query = search::BrowseQuery {
        page: params.page.unwrap_or(1),
        limit: params.limit.unwrap_or(12),
        sort_field: field,
        sort_order: order,
        selected_type: params.r#type.unwrap_or_else(|| "all".into()),
        selected_language: params.language.unwrap_or_else(|| "all".into()),
        selected_platform: params.platform.unwrap_or_else(|| "all".into()),
    };
    ok(search::browse(&state.client, &query).await?)
}

#[derive(Debug, Deserialize)]
pub struct PatchParams {
    /// 8-character uniqueId, or any site URL containing it.
    pub id: String,
}

async fn api_patch(State(state): State<AppState>, Query(params): Query<PatchParams>) -> ApiResult {
    let unique_id = patch::unique_id_from_url(&params.id)
        .ok_or_else(|| Error::InvalidArgument("id must contain an 8-character uniqueId".into()))?;
    ok(patch::get(&state.client, &unique_id).await?)
}

#[derive(Debug, Deserialize)]
pub struct ResourceParams {
    pub patch_id: i64,
    #[serde(default)]
    pub official_only: Option<bool>,
}

async fn api_resources(
    State(state): State<AppState>,
    Query(params): Query<ResourceParams>,
) -> ApiResult {
    let resources = resource::list(&state.client, params.patch_id).await?;
    if params.official_only.unwrap_or(false) {
        return ok(resource::official_resources(&resources));
    }
    ok(resources)
}

#[derive(Debug, Deserialize)]
pub struct PanInfoParams {
    pub key: String,
}

async fn api_pan_info(
    State(state): State<AppState>,
    Query(params): Query<PanInfoParams>,
) -> ApiResult {
    ok(pan::share_info(&state.client, &params.key).await?)
}

#[derive(Debug, Deserialize)]
pub struct PanListParams {
    pub key: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub recursive: Option<bool>,
}

async fn api_pan_list(
    State(state): State<AppState>,
    Query(params): Query<PanListParams>,
) -> ApiResult {
    if params.recursive.unwrap_or(false) {
        return ok(pan::list_files_recursive(&state.client, &params.key, 6).await?);
    }
    ok(pan::list(&state.client, &params.key, params.path.as_deref()).await?)
}

#[derive(Debug, Deserialize)]
pub struct PanDownloadParams {
    pub key: String,
    #[serde(default)]
    pub path: Option<String>,
}

async fn api_pan_download(
    State(state): State<AppState>,
    Query(params): Query<PanDownloadParams>,
) -> ApiResult {
    let url = pan::download_url(&state.client, &params.key, params.path.as_deref()).await?;
    ok(serde_json::json!({ "url": url }))
}

#[derive(Debug, Deserialize)]
pub struct ChainParams {
    #[serde(default)]
    pub patch_id: Option<i64>,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub official_only: Option<bool>,
    #[serde(default)]
    pub urls: Option<bool>,
}

async fn api_chain(State(state): State<AppState>, Query(params): Query<ChainParams>) -> ApiResult {
    let mut options = if params.official_only.unwrap_or(false) {
        chain::ResolveOptions::official()
    } else {
        chain::ResolveOptions::default()
    };
    if params.urls.unwrap_or(false) {
        options.fetch_urls = true;
    }
    let result = if let Some(patch_id) = params.patch_id {
        chain::resolve_download(&state.client, patch_id, options).await?
    } else if let Some(id) = params.id {
        let unique_id = patch::unique_id_from_url(&id).ok_or_else(|| {
            Error::InvalidArgument("id must contain an 8-character uniqueId".into())
        })?;
        chain::resolve_by_unique_id(&state.client, &unique_id, options).await?
    } else if let Some(q) = params.q {
        chain::resolve_by_keyword(&state.client, &q, options).await?
    } else {
        return Err(ApiFailure::from(Error::InvalidArgument(
            "one of patch_id, id or q is required".into(),
        )));
    };
    ok(result)
}

#[derive(Debug, Deserialize)]
pub struct LoginParams {
    pub name: String,
    pub password: String,
}

async fn api_login(State(state): State<AppState>, Query(params): Query<LoginParams>) -> ApiResult {
    let mut client = (*state.client).clone();
    let session = auth::login(&mut client, &params.name, &params.password).await?;
    ok(session)
}

/// Builds a client from environment variables so the server can be configured
/// without a config file.
pub fn client_from_env() -> Result<TouchgalClient, Error> {
    let mut config = ClientConfig::default();
    if let Ok(base) = std::env::var("TOUCHGAL_SITE_BASE") {
        if !base.is_empty() {
            config.site_base = base;
        }
    }
    if let Ok(base) = std::env::var("TOUCHGAL_PAN_BASE") {
        if !base.is_empty() {
            config.pan_base = base;
        }
    }
    if let Ok(token) = std::env::var("TOUCHGAL_SESSION_TOKEN") {
        if !token.is_empty() {
            config.session_token = Some(token);
        }
    }
    TouchgalClient::new(config)
}
