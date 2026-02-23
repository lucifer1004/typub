use serde::Deserialize;

#[derive(Debug)]
pub struct DevtoPayload {
    pub title: String,
    pub body_markdown: String,
    pub tags: Vec<String>,
    pub existing_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DevtoArticleResponse {
    pub id: u64,
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct DevtoArticleListItem {
    pub id: u64,
    pub title: String,
    pub url: String,
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_devto_payload_defaults() {
        let payload = DevtoPayload {
            title: "Test".into(),
            body_markdown: String::new(),
            tags: vec!["rust".into()],
            existing_id: None,
        };
        assert_eq!(payload.title, "Test");
        assert!(payload.body_markdown.is_empty());
        assert_eq!(payload.tags.len(), 1);
        assert!(payload.existing_id.is_none());
    }

    #[test]
    fn test_devto_article_response_deserialize() {
        let json = r#"{"id": 123, "url": "https://dev.to/test/article"}"#;
        let article: DevtoArticleResponse = serde_json::from_str(json).expect("parse");
        assert_eq!(article.id, 123);
        assert_eq!(article.url, "https://dev.to/test/article");
    }

    #[test]
    fn test_devto_article_list_item_deserialize() {
        let json = r#"{"id": 456, "title": "My Article", "url": "https://dev.to/me/my-article"}"#;
        let item: DevtoArticleListItem = serde_json::from_str(json).expect("parse");
        assert_eq!(item.id, 456);
        assert_eq!(item.title, "My Article");
    }
}
