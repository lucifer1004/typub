use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct CreatePageRequest {
    #[serde(rename = "type")]
    pub page_type: String,
    pub title: String,
    pub space: SpaceKey,
    pub body: PageBody,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ancestors: Option<Vec<Ancestor>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[derive(Serialize)]
pub struct UpdatePageRequest {
    #[serde(rename = "type")]
    pub page_type: String,
    pub title: String,
    pub body: PageBody,
    pub version: PageVersion,
    pub metadata: PageMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ancestors: Option<Vec<Ancestor>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[derive(Serialize)]
pub struct PageMetadata {
    pub labels: Vec<PageLabel>,
}

#[derive(Serialize)]
pub struct PageLabel {
    pub prefix: String,
    pub name: String,
}

#[derive(Serialize)]
pub struct PageVersion {
    pub number: u32,
}

#[derive(Serialize)]
pub struct SpaceKey {
    pub key: String,
}

#[derive(Serialize)]
pub struct PageBody {
    pub storage: StorageContent,
}

#[derive(Serialize)]
pub struct StorageContent {
    pub value: String,
    pub representation: String,
}

#[derive(Serialize)]
pub struct Ancestor {
    pub id: String,
}

#[derive(Deserialize)]
pub struct PageResponse {
    pub id: String,
    pub version: PageVersionInfo,
    #[serde(rename = "_links")]
    pub links: PageLinks,
}

#[derive(Deserialize)]
pub struct PageVersionInfo {
    pub number: u32,
}

#[derive(Deserialize)]
pub struct PageLinks {
    pub webui: String,
}

#[derive(Deserialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
}

#[derive(Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    pub version: PageVersionInfo,
    #[serde(rename = "_links")]
    pub links: PageLinks,
}

#[derive(Deserialize)]
pub struct AttachmentResponse {
    pub results: Vec<AttachmentInfo>,
}

#[derive(Deserialize)]
pub struct AttachmentInfo {
    pub id: String,
    pub title: String,
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_page_response_deserialize() {
        let json = r#"{"id": "123", "version": {"number": 1}, "_links": {"webui": "/pages/123"}}"#;
        let page: PageResponse = serde_json::from_str(json).expect("parse");
        assert_eq!(page.id, "123");
        assert_eq!(page.version.number, 1);
    }

    #[test]
    fn test_search_response_deserialize() {
        let json = r#"{"results": [{"id": "1", "title": "Test", "version": {"number": 1}, "_links": {"webui": "/1"}}]}"#;
        let search: SearchResponse = serde_json::from_str(json).expect("parse");
        assert_eq!(search.results.len(), 1);
        assert_eq!(search.results[0].title, "Test");
    }
}
