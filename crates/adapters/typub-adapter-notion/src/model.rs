pub const ID: &str = "notion";

pub const DESIRED_TITLE_PROPERTY: &str = "Title";

#[derive(Debug, Clone)]
pub struct NotionSchema {
    pub title_property: String,
    pub tags_property: String,
}

#[derive(Debug)]
pub struct NotionPayload {
    pub data_source_id: String,
    pub title: String,
    pub existing_page_id: Option<String>,
    pub schema: Option<NotionSchema>,
    pub blocks: Vec<serde_json::Value>,
}
