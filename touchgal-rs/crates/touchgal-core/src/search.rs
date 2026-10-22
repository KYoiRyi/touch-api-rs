//! Search: `POST /api/search`, `POST /api/search/tag` and `GET /api/galgame`.
//!
//! The browser sends a JSON-encoded suggestion array inside `queryString`, not a
//! plain keyword, so the wrapper keeps that shape and offers a friendlier
//! `SearchQuery` for callers.
//!
//! Validated against `validations/search.ts`:
//! * `queryString` <= 1007 chars, `limit` 1..=24, `page` >= 1,
//! * each suggestion is `{ type: keyword|tag|company, mode: include|exclude, id?, name }`,
//! * `searchOption` toggles introduction/alias/tag fields for the keyword match,
//! * `minRatingCount` only takes effect for `sortField = rating` on this endpoint.

use crate::client::{Origin, TouchgalClient};
use crate::models::{GalgameList, SearchSuggestion};
use crate::Error;

/// Sort fields accepted by `/api/search`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortField {
    ResourceUpdateTime,
    Created,
    Rating,
    View,
    Download,
    Favorite,
}

impl SortField {
    pub fn as_str(&self) -> &'static str {
        match self {
            SortField::ResourceUpdateTime => "resource_update_time",
            SortField::Created => "created",
            SortField::Rating => "rating",
            SortField::View => "view",
            SortField::Download => "download",
            SortField::Favorite => "favorite",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    Asc,
    Desc,
}

impl SortOrder {
    pub fn as_str(&self) -> &'static str {
        match self {
            SortOrder::Asc => "asc",
            SortOrder::Desc => "desc",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SuggestionKind {
    Keyword,
    Tag,
    Company,
}

impl SuggestionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SuggestionKind::Keyword => "keyword",
            SuggestionKind::Tag => "tag",
            SuggestionKind::Company => "company",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SuggestionMode {
    Include,
    Exclude,
}

impl SuggestionMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            SuggestionMode::Include => "include",
            SuggestionMode::Exclude => "exclude",
        }
    }
}

/// One chip in the search bar.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Suggestion {
    pub r#type: SuggestionKind,
    #[serde(default = "default_mode")]
    pub mode: SuggestionMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub name: String,
}

fn default_mode() -> SuggestionMode {
    SuggestionMode::Include
}

impl Suggestion {
    pub fn keyword(name: impl Into<String>) -> Self {
        Self {
            r#type: SuggestionKind::Keyword,
            mode: SuggestionMode::Include,
            id: None,
            name: name.into(),
        }
    }

    pub fn tag(name: impl Into<String>, id: Option<i64>) -> Self {
        Self {
            r#type: SuggestionKind::Tag,
            mode: SuggestionMode::Include,
            id,
            name: name.into(),
        }
    }

    pub fn company(name: impl Into<String>, id: Option<i64>) -> Self {
        Self {
            r#type: SuggestionKind::Company,
            mode: SuggestionMode::Include,
            id,
            name: name.into(),
        }
    }

    pub fn exclude(mut self) -> Self {
        self.mode = SuggestionMode::Exclude;
        self
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SearchOptions {
    #[serde(rename = "searchInIntroduction")]
    pub search_in_introduction: bool,
    #[serde(rename = "searchInAlias")]
    pub search_in_alias: bool,
    #[serde(rename = "searchInTag")]
    pub search_in_tag: bool,
}

impl SearchOptions {
    /// The default the search page ships with: alias matching on.
    pub fn site_default() -> Self {
        Self {
            search_in_introduction: false,
            search_in_alias: true,
            search_in_tag: false,
        }
    }
}

/// High-level search request.
#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub suggestions: Vec<Suggestion>,
    pub page: u32,
    pub limit: u32,
    pub options: SearchOptions,
    pub selected_type: String,
    pub selected_language: String,
    pub selected_platform: String,
    pub sort_field: SortField,
    pub sort_order: SortOrder,
    pub selected_years: Vec<String>,
    pub selected_months: Vec<String>,
    pub min_rating_count: u32,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            suggestions: Vec::new(),
            page: 1,
            limit: 12,
            options: SearchOptions::site_default(),
            selected_type: "all".into(),
            selected_language: "all".into(),
            selected_platform: "all".into(),
            sort_field: SortField::ResourceUpdateTime,
            sort_order: SortOrder::Desc,
            selected_years: vec!["all".into()],
            selected_months: vec!["all".into()],
            min_rating_count: 0,
        }
    }
}

impl SearchQuery {
    /// Convenience constructor for the common "type a keyword" case.
    pub fn keyword(text: impl Into<String>) -> Self {
        Self {
            suggestions: vec![Suggestion::keyword(text)],
            ..Default::default()
        }
    }

    pub fn with_page(mut self, page: u32) -> Self {
        self.page = page.max(1);
        self
    }

    pub fn with_limit(mut self, limit: u32) -> Self {
        self.limit = limit.clamp(1, 24);
        self
    }

    pub fn with_suggestions(mut self, suggestions: Vec<Suggestion>) -> Self {
        self.suggestions = suggestions;
        self
    }

    pub fn with_sort(mut self, field: SortField, order: SortOrder) -> Self {
        self.sort_field = field;
        self.sort_order = order;
        self
    }

    pub fn with_platform(mut self, platform: impl Into<String>) -> Self {
        self.selected_platform = platform.into();
        self
    }

    pub fn with_type(mut self, kind: impl Into<String>) -> Self {
        self.selected_type = kind.into();
        self
    }

    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.selected_language = language.into();
        self
    }

    pub fn with_years(mut self, years: Vec<String>) -> Self {
        self.selected_years = years;
        self
    }

    pub fn with_min_rating_count(mut self, count: u32) -> Self {
        self.min_rating_count = count;
        self
    }

    /// Renders the wire payload, including the JSON-in-a-string `queryString`.
    pub fn to_payload(&self) -> Result<serde_json::Value, Error> {
        let query_string = serde_json::to_string(&self.suggestions)
            .map_err(|e| Error::InvalidArgument(format!("suggestions: {e}")))?;
        if query_string.chars().count() > 1007 {
            return Err(Error::InvalidArgument(
                "搜索字符串最大为 1007 个字符".into(),
            ));
        }
        if self.selected_years.len() > 64 || self.selected_months.len() > 64 {
            return Err(Error::InvalidArgument("最多选择 64 组年份/月份".into()));
        }
        Ok(serde_json::json!({
            "queryString": query_string,
            "limit": self.limit.clamp(1, 24),
            "searchOption": self.options,
            "page": self.page.max(1),
            "selectedType": self.selected_type,
            "selectedLanguage": self.selected_language,
            "selectedPlatform": self.selected_platform,
            "sortField": self.sort_field.as_str(),
            "sortOrder": self.sort_order.as_str(),
            "selectedYears": self.selected_years,
            "selectedMonths": self.selected_months,
            "minRatingCount": self.min_rating_count,
        }))
    }
}

/// `POST /api/search`.
pub async fn search(client: &TouchgalClient, query: &SearchQuery) -> crate::Result<GalgameList> {
    let payload = query.to_payload()?;
    let builder = client.post(Origin::Site, "/api/search").json(&payload);
    client.json(builder).await
}

/// `POST /api/search/tag` — the typeahead used by the search bar.
pub async fn suggest(
    client: &TouchgalClient,
    keywords: &[String],
) -> crate::Result<Vec<SearchSuggestion>> {
    if keywords.is_empty() || keywords.len() > 10 {
        return Err(Error::InvalidArgument("最多使用 10 组关键词".into()));
    }
    let builder = client
        .post(Origin::Site, "/api/search/tag")
        .json(&serde_json::json!({ "query": keywords }));
    client.json(builder).await
}

/// `GET /api/galgame` — the browse list (no keyword, only filters).
pub async fn browse(client: &TouchgalClient, query: &BrowseQuery) -> crate::Result<GalgameList> {
    let path = format!(
        "/api/galgame?page={}&limit={}&sortField={}&sortOrder={}&selectedType={}&selectedLanguage={}&selectedPlatform={}&yearString=%5B%22all%22%5D&monthString=%5B%22all%22%5D&minRatingCount=0",
        query.page.max(1),
        query.limit.clamp(1, 24),
        query.sort_field.as_str(),
        query.sort_order.as_str(),
        query.selected_type,
        query.selected_language,
        query.selected_platform,
    );
    client.json(client.get(Origin::Site, &path)).await
}

/// Filters for [browse]. Built with Default::default() and struct update syntax.
#[derive(Debug, Clone)]
pub struct BrowseQuery {
    pub page: u32,
    pub limit: u32,
    pub sort_field: SortField,
    pub sort_order: SortOrder,
    pub selected_type: String,
    pub selected_language: String,
    pub selected_platform: String,
}

impl Default for BrowseQuery {
    fn default() -> Self {
        Self {
            page: 1,
            limit: 12,
            sort_field: SortField::ResourceUpdateTime,
            sort_order: SortOrder::Desc,
            selected_type: "all".into(),
            selected_language: "all".into(),
            selected_platform: "all".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_matches_observed_browser_body() {
        // Body captured from the real search page for 千恋万花.
        let query = SearchQuery {
            suggestions: vec![Suggestion::keyword("千恋万花")],
            page: 1,
            limit: 12,
            options: SearchOptions::site_default(),
            selected_type: "all".into(),
            selected_language: "all".into(),
            selected_platform: "all".into(),
            sort_field: SortField::ResourceUpdateTime,
            sort_order: SortOrder::Desc,
            selected_years: vec!["all".into()],
            selected_months: vec!["all".into()],
            min_rating_count: 0,
        };
        let payload = query.to_payload().unwrap();
        assert_eq!(
            payload["queryString"],
            "[{\"type\":\"keyword\",\"mode\":\"include\",\"name\":\"千恋万花\"}]"
        );
        assert_eq!(payload["searchOption"]["searchInAlias"], true);
        assert_eq!(payload["sortField"], "resource_update_time");
        assert_eq!(payload["selectedYears"][0], "all");
    }

    #[test]
    fn limits_are_clamped_like_the_server_schema() {
        let query = SearchQuery::keyword("a").with_limit(999).with_page(0);
        let payload = query.to_payload().unwrap();
        assert_eq!(payload["limit"], 24);
        assert_eq!(payload["page"], 1);
    }

    #[test]
    fn oversized_query_string_is_rejected() {
        let query = SearchQuery::keyword("あ".repeat(4000));
        assert!(query.to_payload().is_err());
    }
}
