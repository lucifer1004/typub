use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct GhostPostResponse {
    pub posts: Vec<GhostPost>,
}

#[derive(Debug, Deserialize)]
pub struct GhostPost {
    pub id: String,
    pub uuid: String,
    pub url: String,
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub tags: Vec<GhostTag>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GhostTag {
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub slug: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GhostPostRequest {
    pub posts: Vec<GhostPostData>,
}

#[derive(Debug, Serialize)]
pub struct GhostPostData {
    pub title: String,
    /// Lexical content (JSON string)
    pub lexical: String,
    pub status: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<GhostTagInput>,
    /// Required for updates to prevent version conflicts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GhostTagInput {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct GhostPostsListResponse {
    pub posts: Vec<GhostPost>,
    #[serde(default)]
    pub meta: Option<GhostMeta>,
}

#[derive(Debug, Deserialize)]
pub struct GhostMeta {
    pub pagination: GhostPagination,
}

#[derive(Debug, Deserialize)]
pub struct GhostPagination {
    #[allow(dead_code)] // API response: current page index; we only use .pages for iteration
    pub page: u32,
    pub pages: u32,
}

#[derive(Debug, Deserialize)]
pub struct GhostImageUploadResponse {
    pub images: Vec<GhostUploadedImage>,
}

#[derive(Debug, Deserialize)]
pub struct GhostUploadedImage {
    pub url: String,
}

impl Clone for GhostPost {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            uuid: self.uuid.clone(),
            url: self.url.clone(),
            slug: self.slug.clone(),
            title: self.title.clone(),
            tags: self.tags.clone(),
            updated_at: self.updated_at.clone(),
        }
    }
}
