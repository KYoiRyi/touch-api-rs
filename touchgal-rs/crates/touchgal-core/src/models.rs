//! Response models mirrored from `types/api/*.ts` and the live responses.

use serde::{Deserialize, Serialize};

/// `GalgameCard` — the list/card projection returned by `/api/galgame`,
/// `/api/search` and `/api/home`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GalgameCard {
    pub id: i64,
    #[serde(rename = "uniqueId")]
    pub unique_id: String,
    pub name: String,
    #[serde(default)]
    pub banner: String,
    #[serde(default)]
    pub view: i64,
    #[serde(default)]
    pub download: i64,
    #[serde(default)]
    pub r#type: Vec<String>,
    #[serde(default)]
    pub language: Vec<String>,
    #[serde(default)]
    pub platform: Vec<String>,
    #[serde(default)]
    pub created: String,
    #[serde(rename = "_count", default)]
    pub count: GalgameCardCount,
    #[serde(rename = "averageRating", default)]
    pub average_rating: Option<f64>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct GalgameCardCount {
    #[serde(default)]
    pub favorite_folder: i64,
    #[serde(default)]
    pub resource: i64,
    #[serde(default)]
    pub comment: i64,
}

/// `{ galgames, total }` — both `/api/galgame` and `/api/search`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GalgameList {
    pub galgames: Vec<GalgameCard>,
    #[serde(default)]
    pub total: i64,
}

/// Full patch payload from `GET /api/patch?uniqueId=…`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Patch {
    pub id: i64,
    #[serde(rename = "uniqueId")]
    pub unique_id: String,
    #[serde(rename = "vndbId", default)]
    pub vndb_id: Option<String>,
    #[serde(rename = "vndbRelationId", default)]
    pub vndb_relation_id: Option<String>,
    #[serde(rename = "bangumiId", default)]
    pub bangumi_id: Option<i64>,
    #[serde(rename = "steamId", default)]
    pub steam_id: Option<i64>,
    #[serde(rename = "dlsiteCode", default)]
    pub dlsite_code: Option<String>,
    pub name: String,
    #[serde(default)]
    pub introduction: String,
    #[serde(default)]
    pub banner: String,
    #[serde(default)]
    pub status: i64,
    #[serde(default)]
    pub view: i64,
    #[serde(default)]
    pub download: i64,
    #[serde(default)]
    pub r#type: Vec<String>,
    #[serde(default)]
    pub language: Vec<String>,
    #[serde(default)]
    pub platform: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub alias: Vec<String>,
    #[serde(rename = "contentLimit", default)]
    pub content_limit: String,
    #[serde(rename = "ratingSummary", default)]
    pub rating_summary: Option<RatingSummary>,
    #[serde(default)]
    pub user: Option<PatchAuthor>,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub updated: String,
    #[serde(rename = "_count", default)]
    pub count: GalgameCardCount,
    #[serde(rename = "isFavorite", default)]
    pub is_favorite: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct RatingSummary {
    #[serde(default)]
    pub average: f64,
    #[serde(default)]
    pub count: i64,
    #[serde(default)]
    pub histogram: Vec<RatingBucket>,
    #[serde(default)]
    pub recommend: Option<RatingRecommend>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RatingBucket {
    pub score: i64,
    pub count: i64,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct RatingRecommend {
    #[serde(default)]
    pub strong_no: i64,
    #[serde(default)]
    pub no: i64,
    #[serde(default)]
    pub neutral: i64,
    #[serde(default)]
    pub yes: i64,
    #[serde(default)]
    pub strong_yes: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PatchAuthor {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub avatar: String,
}

/// `PatchResourceLink` from `types/api/patch.ts`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResourceLink {
    pub id: i64,
    pub storage: String,
    #[serde(default)]
    pub size: String,
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub hash: String,
    #[serde(default)]
    pub content: String,
    #[serde(rename = "sortOrder", default)]
    pub sort_order: i64,
    #[serde(default)]
    pub download: i64,
}

/// `PatchResource` from `types/api/patch.ts`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PatchResource {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub section: String,
    #[serde(rename = "uniqueId", default)]
    pub unique_id: String,
    #[serde(default)]
    pub r#type: Vec<String>,
    #[serde(default)]
    pub language: Vec<String>,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub platform: Vec<String>,
    #[serde(rename = "emulatorType", default)]
    pub emulator_type: Vec<String>,
    #[serde(rename = "modelName", default)]
    pub model_name: String,
    #[serde(default)]
    pub download: i64,
    #[serde(default)]
    pub links: Vec<ResourceLink>,
    #[serde(rename = "likeCount", default)]
    pub like_count: i64,
    #[serde(rename = "isLike", default)]
    pub is_like: bool,
    #[serde(default)]
    pub status: i64,
    #[serde(rename = "userId", default)]
    pub user_id: i64,
    #[serde(rename = "patchId", default)]
    pub patch_id: i64,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub user: Option<ResourceAuthor>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResourceAuthor {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub avatar: String,
    #[serde(default)]
    pub role: i64,
    #[serde(rename = "patchCount", default)]
    pub patch_count: i64,
}

/// Suggestion returned by `POST /api/search/tag`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearchSuggestion {
    #[serde(default)]
    pub id: Option<i64>,
    pub r#type: String,
    #[serde(default = "default_mode")]
    pub mode: String,
    pub name: String,
}

fn default_mode() -> String {
    "include".to_string()
}

/// Login response (`UserState` subset the CLI cares about).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UserSession {
    pub uid: i64,
    pub name: String,
    #[serde(default)]
    pub role: i64,
    #[serde(default)]
    pub avatar: String,
    #[serde(default)]
    pub moemoepoint: i64,
    #[serde(rename = "enableRedirect", default)]
    pub enable_redirect: bool,
    #[serde(rename = "excludedDomains", default)]
    pub excluded_domains: Vec<String>,
    #[serde(rename = "delaySeconds", default)]
    pub delay_seconds: u64,
    #[serde(rename = "require2FA", default)]
    pub require_2fa: bool,
}
