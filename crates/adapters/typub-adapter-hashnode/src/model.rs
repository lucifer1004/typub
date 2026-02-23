use serde::Deserialize;

#[derive(Debug)]
pub struct HashnodePayload {
    pub title: String,
    pub content_markdown: String,
    pub tags: Vec<String>,
    pub existing_id: Option<String>,
    pub slug: String,
}

#[derive(Debug, Deserialize)]
pub struct HashnodePostResponse {
    pub id: String,
    pub url: String,
    #[allow(dead_code)]
    pub slug: String,
}

#[derive(Debug, Deserialize)]
pub struct PublishPostData {
    #[serde(rename = "publishPost")]
    pub publish_post: PostWrapper,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePostData {
    #[serde(rename = "updatePost")]
    pub update_post: PostWrapper,
}

#[derive(Debug, Deserialize)]
pub struct PostWrapper {
    pub post: HashnodePostResponse,
}

#[derive(Debug, Deserialize)]
pub struct FindPostData {
    pub publication: Option<PublicationWrapper>,
}

#[derive(Debug, Deserialize)]
pub struct PublicationWrapper {
    pub post: Option<HashnodePostResponse>,
}

#[derive(Debug, Deserialize)]
pub struct GraphQLResponse<T> {
    pub data: Option<T>,
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize)]
pub struct GraphQLError {
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct DraftResponse {
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateDraftData {
    #[serde(rename = "createDraft")]
    pub create_draft: DraftWrapper,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDraftData {
    #[serde(rename = "updateDraft")]
    pub update_draft: DraftWrapper,
}

#[derive(Debug, Deserialize)]
pub struct DraftWrapper {
    pub draft: DraftResponse,
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_hashnode_payload_defaults() {
        let payload = HashnodePayload {
            title: "Test".into(),
            content_markdown: String::new(),
            tags: vec!["rust".into()],
            existing_id: None,
            slug: "test-slug".into(),
        };
        assert_eq!(payload.title, "Test");
        assert!(payload.content_markdown.is_empty());
        assert_eq!(payload.tags.len(), 1);
        assert!(payload.existing_id.is_none());
    }

    #[test]
    fn test_graphql_response_deserialize() {
        let json = r#"{"data": {"publishPost": {"post": {"id": "123", "url": "https://test.com", "slug": "test"}}}, "errors": null}"#;
        let response: GraphQLResponse<PublishPostData> = serde_json::from_str(json).expect("parse");
        assert!(response.data.is_some());
        assert!(response.errors.is_none());
    }
}
