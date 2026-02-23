use serde::Deserialize;

#[derive(Debug)]
pub(super) struct WordPressPayload {
    pub title: String,
    pub slug: String,
    pub tags: Vec<String>,
    pub categories: Vec<String>,
    pub tag_ids: Vec<u64>,
    pub category_ids: Vec<u64>,
    pub final_body: Option<String>,
    pub existing_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct WpMediaResponse {
    pub source_url: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct WpTermResponse {
    pub id: u64,
    pub name: String,
    pub slug: String,
}
