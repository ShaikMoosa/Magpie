use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityStatus {
    Dangerous,
    Suspicious,
    NoThreat,
    NotChecked,
}

impl SecurityStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            SecurityStatus::Dangerous => "dangerous",
            SecurityStatus::Suspicious => "suspicious",
            SecurityStatus::NoThreat => "no_threat",
            SecurityStatus::NotChecked => "not_checked",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "dangerous" => SecurityStatus::Dangerous,
            "suspicious" => SecurityStatus::Suspicious,
            "no_threat" => SecurityStatus::NoThreat,
            _ => SecurityStatus::NotChecked,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bookmark {
    pub id: String,
    pub url: String,
    pub title: String,
    pub description: Option<String>,
    pub domain: String,
    pub favicon_path: Option<String>,
    pub thumbnail_path: Option<String>,
    pub category_id: Option<String>,
    pub is_favourite: bool,
    pub source_browser: Option<String>,
    pub source_folder: Option<String>,
    pub security_status: SecurityStatus,
    pub security_reasons: Vec<String>,
    pub security_source: Option<String>,
    pub security_checked_at: Option<String>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_opened_at: Option<String>,
    pub open_count: i64,
    pub deleted_at: Option<String>,
    /// `None` = preview not fetched yet, `ok`, or `failed`.
    pub metadata_status: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NewBookmark {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub category_id: Option<String>,
    pub is_favourite: bool,
    pub tags: Vec<String>,
    /// Save even if the normalised URL already exists.
    pub allow_duplicate: bool,
    pub source_browser: Option<String>,
    pub source_folder: Option<String>,
    /// Cached preview image / favicon from `fetch_metadata` (cache file names).
    pub thumbnail: Option<String>,
    pub favicon: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateBookmark {
    pub id: String,
    pub url: String,
    pub title: String,
    pub description: Option<String>,
    pub category_id: Option<String>,
    pub is_favourite: bool,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum View {
    #[default]
    All,
    Favourites,
    Recent,
    Uncategorized,
    Security,
    Trash,
    Category,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sort {
    #[default]
    RecentlyAdded,
    RecentlyModified,
    TitleAsc,
    TitleDesc,
    Domain,
    RecentlyOpened,
    MostOpened,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BookmarkQuery {
    pub view: View,
    pub category_id: Option<String>,
    pub search: Option<String>,
    pub sort: Sort,
    pub favourites_only: bool,
    pub security: Option<SecurityStatus>,
    pub domain: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub color: String,
    pub icon: Option<String>,
    pub parent_id: Option<String>,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
    /// Non-deleted bookmarks directly in this category.
    pub bookmark_count: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewCategory {
    pub name: String,
    pub description: Option<String>,
    pub color: String,
    pub icon: Option<String>,
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCategory {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub color: String,
    pub icon: Option<String>,
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteCategory {
    pub id: String,
    /// Where the bookmarks go. `None` = Uncategorized.
    pub move_to: Option<String>,
    /// Keep subcategories (as top-level categories) instead of deleting them too.
    pub keep_subcategories: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub all: i64,
    pub favourites: i64,
    pub recent: i64,
    pub uncategorized: i64,
    pub security_alerts: i64,
    pub trash: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UrlCheck {
    pub url: String,
    pub normalized: String,
    pub domain: String,
    pub duplicate_of: Option<String>,
    pub security_status: SecurityStatus,
    pub security_reasons: Vec<String>,
}
