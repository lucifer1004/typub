//! Notion API client - encapsulates all REST API interactions

use anyhow::{Context, Result};
use reqwest::header::{self, HeaderMap, HeaderValue};
use reqwest::multipart::{Form, Part};
use reqwest::{Client, RequestBuilder, StatusCode};
use serde_json::{Value, json};

pub const NOTION_API_BASE: &str = "https://api.notion.com/v1";
const NOTION_VERSION: &str = "2025-09-03";
const MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF_MS: u64 = 1000;
const BLOCK_CHUNK_SIZE: usize = 100;

/// Low-level Notion API client.
/// Borrows an external reqwest Client and adds auth headers per-request.
/// All requests are retried on 429 (rate limit) and 5xx (server error).
pub(super) struct NotionClient<'a> {
    client: &'a Client,
    base_url: &'a str,
    api_token: &'a str,
}

impl<'a> NotionClient<'a> {
    pub fn new(client: &'a Client, base_url: &'a str, api_token: &'a str) -> Self {
        Self {
            client,
            base_url,
            api_token,
        }
    }

    fn auth_headers(&self) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert("Notion-Version", HeaderValue::from_static(NOTION_VERSION));
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.api_token))
                .context("Invalid Notion API token")?,
        );
        Ok(headers)
    }

    /// Query a data source with a filter (API version 2025-09-03).
    pub async fn query_data_source(&self, data_source_id: &str, filter: Value) -> Result<Value> {
        let base = self.base_url;
        self.json_request(
            self.client
                .post(format!("{base}/data_sources/{data_source_id}/query"))
                .headers(self.auth_headers()?)
                .json(&filter),
            "query data source",
        )
        .await
    }

    /// Get data source schema/details (API version 2025-09-03).
    pub async fn get_data_source(&self, data_source_id: &str) -> Result<Value> {
        let base = self.base_url;
        self.json_request(
            self.client
                .get(format!("{base}/data_sources/{data_source_id}"))
                .headers(self.auth_headers()?),
            "get data source",
        )
        .await
    }

    /// Update data source schema/properties (API version 2025-09-03).
    pub async fn update_data_source(&self, data_source_id: &str, payload: Value) -> Result<Value> {
        let base = self.base_url;
        self.json_request(
            self.client
                .patch(format!("{base}/data_sources/{data_source_id}"))
                .headers(self.auth_headers()?)
                .json(&payload),
            "update data source",
        )
        .await
    }

    /// Get child blocks of a block (or page).
    #[allow(dead_code)] // used in tests; kept for future block-level operations
    pub async fn get_block_children(&self, block_id: &str) -> Result<Value> {
        let base = self.base_url;
        self.json_request(
            self.client
                .get(format!("{base}/blocks/{block_id}/children"))
                .headers(self.auth_headers()?),
            "get block children",
        )
        .await
    }

    /// Delete a single block.
    #[allow(dead_code)] // used in tests; kept for future block-level operations
    pub async fn delete_block(&self, block_id: &str) -> Result<()> {
        let base = self.base_url;
        self.void_request(
            self.client
                .delete(format!("{base}/blocks/{block_id}"))
                .headers(self.auth_headers()?),
            "delete block",
        )
        .await
    }

    /// Erase all block content from a page in one API call.
    /// Uses PATCH /pages/{page_id} with erase_content: true.
    pub async fn erase_page_content(&self, page_id: &str) -> Result<()> {
        let base = self.base_url;
        self.void_request(
            self.client
                .patch(format!("{base}/pages/{page_id}"))
                .headers(self.auth_headers()?)
                .json(&json!({ "erase_content": true })),
            "erase page content",
        )
        .await
    }

    /// Append child blocks to a block (or page).
    /// Automatically chunks into batches of 100 (Notion API limit).
    pub async fn append_block_children(&self, block_id: &str, children: &[Value]) -> Result<()> {
        if children.is_empty() {
            return Ok(());
        }
        let base = self.base_url;
        for chunk in children.chunks(BLOCK_CHUNK_SIZE) {
            self.void_request(
                self.client
                    .patch(format!("{base}/blocks/{block_id}/children"))
                    .headers(self.auth_headers()?)
                    .json(&json!({ "children": chunk })),
                "append blocks",
            )
            .await?;
        }
        Ok(())
    }

    /// Create a page in a data source (API version 2025-09-03).
    pub async fn create_page(
        &self,
        data_source_id: &str,
        properties: Value,
        children: &[Value],
    ) -> Result<Value> {
        let base = self.base_url;
        let mut body = json!({
            "parent": { "type": "data_source_id", "data_source_id": data_source_id },
            "properties": properties,
        });
        if !children.is_empty() {
            body["children"] = json!(children);
        }
        self.json_request(
            self.client
                .post(format!("{base}/pages"))
                .headers(self.auth_headers()?)
                .json(&body),
            "create page",
        )
        .await
    }

    /// Update page properties.
    pub async fn update_page_properties(&self, page_id: &str, properties: Value) -> Result<Value> {
        let base = self.base_url;
        self.json_request(
            self.client
                .patch(format!("{base}/pages/{page_id}"))
                .headers(self.auth_headers()?)
                .json(&json!({ "properties": properties })),
            "update page properties",
        )
        .await
    }

    /// Create a file upload slot. Returns (upload_id, upload_url).
    pub async fn create_file_upload(
        &self,
        filename: &str,
        content_type: &str,
    ) -> Result<(String, String)> {
        let base = self.base_url;
        let result = self
            .json_request(
                self.client
                    .post(format!("{base}/file_uploads"))
                    .headers(self.auth_headers()?)
                    .json(&json!({
                        "filename": filename,
                        "content_type": content_type,
                    })),
                "create file upload",
            )
            .await?;

        let upload_id = result["id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No upload ID in response"))?
            .to_string();
        let upload_url = result["upload_url"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No upload URL in response"))?
            .to_string();

        Ok((upload_id, upload_url))
    }

    /// Upload file content to a previously created upload slot.
    pub async fn send_file_upload(
        &self,
        upload_url: &str,
        data: Vec<u8>,
        filename: &str,
        content_type: &str,
    ) -> Result<()> {
        let part = Part::bytes(data)
            .file_name(filename.to_string())
            .mime_str(content_type)?;

        self.void_request(
            self.client
                .post(upload_url)
                .headers(self.auth_headers()?)
                .multipart(Form::new().part("file", part)),
            "upload file",
        )
        .await
    }

    // ---- internal helpers with retry ----

    /// Send a request with automatic retry on 429 and 5xx.
    async fn request_with_retry(
        &self,
        request: RequestBuilder,
        action: &str,
    ) -> Result<reqwest::Response> {
        let mut current = request;

        for attempt in 0..=MAX_RETRIES {
            let cloned = current.try_clone();

            let response = current
                .send()
                .await
                .with_context(|| format!("Failed to send {action} request"))?;

            let status = response.status();

            if status.is_success() {
                return Ok(response);
            }

            let retryable = status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error();

            if !retryable || attempt == MAX_RETRIES {
                let body = response.text().await.unwrap_or_default();
                anyhow::bail!("Notion {action} error ({status}): {body}");
            }

            let Some(next) = cloned else {
                let body = response.text().await.unwrap_or_default();
                anyhow::bail!("Notion {action} error ({status}), cannot retry: {body}");
            };

            if status == StatusCode::TOO_MANY_REQUESTS {
                let secs = response
                    .headers()
                    .get("Retry-After")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(1.0);
                tokio::time::sleep(std::time::Duration::from_secs_f64(secs)).await;
            } else {
                let ms = INITIAL_BACKOFF_MS * 2u64.pow(attempt);
                tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
            }

            current = next;
        }

        unreachable!()
    }

    /// Send request with retry, expect JSON response body.
    async fn json_request(&self, request: RequestBuilder, action: &str) -> Result<Value> {
        let response = self.request_with_retry(request, action).await?;
        response
            .json()
            .await
            .with_context(|| format!("Failed to parse {action} response"))
    }

    /// Send request with retry, expect no meaningful response body.
    async fn void_request(&self, request: RequestBuilder, action: &str) -> Result<()> {
        self.request_with_retry(request, action).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use reqwest::Client;
    use wiremock::matchers::{method, path, path_regex};

    fn test_http_client() -> Client {
        Client::builder().build().expect("build test http client")
    }
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Test fixture that owns the reqwest::Client, base_url, and MockServer.
    struct TestFixture {
        http_client: Client,
        server: MockServer,
        base_url: String,
    }

    impl TestFixture {
        async fn new() -> Self {
            let server = MockServer::start().await;
            let base_url = server.uri();
            Self {
                http_client: test_http_client(),
                server,
                base_url,
            }
        }

        fn client(&self) -> NotionClient<'_> {
            NotionClient::new(&self.http_client, &self.base_url, "test-token")
        }

        fn server(&self) -> &MockServer {
            &self.server
        }
    }

    // ---- existing API tests (updated for 2025-09-03 data sources) ----

    #[tokio::test]
    async fn test_query_data_source() {
        let fixture = TestFixture::new().await;

        Mock::given(method("POST"))
            .and(path("/data_sources/ds-123/query"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({ "results": [{ "id": "page-1" }] })),
            )
            .expect(1)
            .mount(fixture.server())
            .await;

        let filter = json!({ "filter": { "property": "title", "title": { "equals": "Test" } } });
        let result = fixture
            .client()
            .query_data_source("ds-123", filter)
            .await
            .expect("query data source");
        assert_eq!(result["results"][0]["id"], "page-1");
    }

    #[tokio::test]
    async fn test_get_data_source() {
        let fixture = TestFixture::new().await;

        Mock::given(method("GET"))
            .and(path("/data_sources/ds-123"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({ "id": "ds-123", "properties": {} })),
            )
            .expect(1)
            .mount(fixture.server())
            .await;

        let result = fixture
            .client()
            .get_data_source("ds-123")
            .await
            .expect("get data source");
        assert_eq!(result["id"], "ds-123");
    }

    #[tokio::test]
    async fn test_update_data_source() {
        let fixture = TestFixture::new().await;

        Mock::given(method("PATCH"))
            .and(path("/data_sources/ds-123"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({ "id": "ds-123", "properties": {} })),
            )
            .expect(1)
            .mount(fixture.server())
            .await;

        let result = fixture
            .client()
            .update_data_source("ds-123", json!({ "properties": {} }))
            .await
            .expect("update data source");
        assert_eq!(result["id"], "ds-123");
    }

    #[tokio::test]
    async fn test_query_data_source_error() {
        let fixture = TestFixture::new().await;

        Mock::given(method("POST"))
            .and(path("/data_sources/ds-123/query"))
            .respond_with(ResponseTemplate::new(400).set_body_string("bad request"))
            .expect(1)
            .mount(fixture.server())
            .await;

        let result = fixture
            .client()
            .query_data_source("ds-123", json!({}))
            .await;
        assert!(result.is_err());
        let err = result.expect_err("should fail").to_string();
        assert!(err.contains("400"), "error should contain status: {err}");
    }

    #[tokio::test]
    async fn test_get_block_children() {
        let fixture = TestFixture::new().await;

        Mock::given(method("GET"))
            .and(path("/blocks/block-1/children"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    json!({ "results": [{ "id": "child-1", "type": "paragraph" }] }),
                ),
            )
            .expect(1)
            .mount(fixture.server())
            .await;

        let result = fixture
            .client()
            .get_block_children("block-1")
            .await
            .expect("get block children");
        assert_eq!(result["results"][0]["id"], "child-1");
    }

    #[tokio::test]
    async fn test_get_block_children_error() {
        let fixture = TestFixture::new().await;

        Mock::given(method("GET"))
            .and(path("/blocks/block-1/children"))
            .respond_with(ResponseTemplate::new(500).set_body_string("internal error"))
            .expect(1..=4) // may be retried
            .mount(fixture.server())
            .await;

        let result = fixture.client().get_block_children("block-1").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_block() {
        let fixture = TestFixture::new().await;

        Mock::given(method("DELETE"))
            .and(path("/blocks/block-99"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(fixture.server())
            .await;

        fixture
            .client()
            .delete_block("block-99")
            .await
            .expect("delete block");
    }

    #[tokio::test]
    async fn test_delete_block_error() {
        let fixture = TestFixture::new().await;

        Mock::given(method("DELETE"))
            .and(path("/blocks/block-99"))
            .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
            .expect(1)
            .mount(fixture.server())
            .await;

        let result = fixture.client().delete_block("block-99").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_append_block_children() {
        let fixture = TestFixture::new().await;

        Mock::given(method("PATCH"))
            .and(path("/blocks/page-1/children"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(fixture.server())
            .await;

        let children = vec![json!({ "type": "paragraph", "paragraph": { "rich_text": [] } })];
        fixture
            .client()
            .append_block_children("page-1", &children)
            .await
            .expect("append block children");
    }

    #[tokio::test]
    async fn test_append_block_children_empty() {
        let fixture = TestFixture::new().await;

        // Empty children should not make any request
        fixture
            .client()
            .append_block_children("page-1", &[])
            .await
            .expect("append block children empty");
    }

    #[tokio::test]
    async fn test_append_block_children_error() {
        let fixture = TestFixture::new().await;

        Mock::given(method("PATCH"))
            .and(path("/blocks/page-1/children"))
            .respond_with(ResponseTemplate::new(400).set_body_string("fail"))
            .expect(1)
            .mount(fixture.server())
            .await;

        let children = vec![json!({ "type": "paragraph" })];
        let result = fixture
            .client()
            .append_block_children("page-1", &children)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_page() {
        let fixture = TestFixture::new().await;

        Mock::given(method("POST"))
            .and(path("/pages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "new-page-id",
                "url": "https://www.notion.so/new-page-id"
            })))
            .expect(1)
            .mount(fixture.server())
            .await;

        let props = json!({ "title": { "title": [{ "text": { "content": "Test" } }] } });
        let children = vec![json!({ "type": "paragraph" })];
        let result = fixture
            .client()
            .create_page("db-123", props, &children)
            .await
            .expect("create page");
        assert_eq!(result["id"], "new-page-id");
    }

    #[tokio::test]
    async fn test_update_page_properties() {
        let fixture = TestFixture::new().await;

        Mock::given(method("PATCH"))
            .and(path("/pages/page-1"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    json!({ "id": "page-1", "url": "https://www.notion.so/page-1" }),
                ),
            )
            .expect(1)
            .mount(fixture.server())
            .await;

        let result = fixture
            .client()
            .update_page_properties("page-1", json!({ "Tags": { "multi_select": [] } }))
            .await
            .expect("update page properties");
        assert_eq!(result["id"], "page-1");
    }

    #[tokio::test]
    async fn test_create_page_error() {
        let fixture = TestFixture::new().await;

        Mock::given(method("POST"))
            .and(path("/pages"))
            .respond_with(ResponseTemplate::new(400).set_body_string("bad"))
            .expect(1)
            .mount(fixture.server())
            .await;

        let result = fixture.client().create_page("db-123", json!({}), &[]).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_file_upload() {
        let fixture = TestFixture::new().await;

        Mock::given(method("POST"))
            .and(path("/file_uploads"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "upload-id-42",
                "upload_url": "https://upload.example.com/42"
            })))
            .expect(1)
            .mount(fixture.server())
            .await;

        let (upload_id, upload_url) = fixture
            .client()
            .create_file_upload("photo.png", "image/png")
            .await
            .expect("create file upload");
        assert_eq!(upload_id, "upload-id-42");
        assert_eq!(upload_url, "https://upload.example.com/42");
    }

    #[tokio::test]
    async fn test_create_file_upload_error() {
        let fixture = TestFixture::new().await;

        Mock::given(method("POST"))
            .and(path("/file_uploads"))
            .respond_with(ResponseTemplate::new(500).set_body_string("fail"))
            .expect(1..=4) // may be retried
            .mount(fixture.server())
            .await;

        let result = fixture
            .client()
            .create_file_upload("photo.png", "image/png")
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_send_file_upload() {
        let fixture = TestFixture::new().await;

        // send_file_upload posts to an arbitrary URL, so use a wildcard path
        Mock::given(method("POST"))
            .and(path_regex("/upload/.*"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(fixture.server())
            .await;

        let upload_url = format!("{}/upload/42", fixture.server().uri());
        fixture
            .client()
            .send_file_upload(&upload_url, vec![0u8; 10], "test.png", "image/png")
            .await
            .expect("send file upload");
    }

    #[tokio::test]
    async fn test_send_file_upload_error() {
        let fixture = TestFixture::new().await;

        Mock::given(method("POST"))
            .and(path_regex("/upload/.*"))
            .respond_with(ResponseTemplate::new(500).set_body_string("fail"))
            .expect(1..=4) // may be retried
            .mount(fixture.server())
            .await;

        let upload_url = format!("{}/upload/42", fixture.server().uri());
        let result = fixture
            .client()
            .send_file_upload(&upload_url, vec![0u8; 10], "test.png", "image/png")
            .await;
        assert!(result.is_err());
    }

    // ---- erase_page_content tests ----

    #[tokio::test]
    async fn test_erase_page_content() {
        let fixture = TestFixture::new().await;

        Mock::given(method("PATCH"))
            .and(path("/pages/page-42"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "page-42" })))
            .expect(1)
            .mount(fixture.server())
            .await;

        fixture
            .client()
            .erase_page_content("page-42")
            .await
            .expect("erase page content");
    }

    #[tokio::test]
    async fn test_erase_page_content_error() {
        let fixture = TestFixture::new().await;

        Mock::given(method("PATCH"))
            .and(path("/pages/page-42"))
            .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
            .expect(1)
            .mount(fixture.server())
            .await;

        let result = fixture.client().erase_page_content("page-42").await;
        assert!(result.is_err());
    }

    // ---- chunked append tests ----

    #[tokio::test]
    async fn test_append_block_children_chunked() {
        let fixture = TestFixture::new().await;

        // 150 blocks → should result in 2 PATCH requests (100 + 50)
        Mock::given(method("PATCH"))
            .and(path("/blocks/page-1/children"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(2)
            .mount(fixture.server())
            .await;

        let children: Vec<Value> = (0..150)
            .map(|i| json!({ "type": "paragraph", "paragraph": { "rich_text": [{ "type": "text", "text": { "content": format!("block {i}") } }] } }))
            .collect();

        fixture
            .client()
            .append_block_children("page-1", &children)
            .await
            .expect("append block children");
    }

    #[tokio::test]
    async fn test_append_exactly_100_blocks_single_request() {
        let fixture = TestFixture::new().await;

        Mock::given(method("PATCH"))
            .and(path("/blocks/page-1/children"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(fixture.server())
            .await;

        let children: Vec<Value> = (0..100)
            .map(|i| json!({ "type": "paragraph", "paragraph": { "rich_text": [{ "type": "text", "text": { "content": format!("block {i}") } }] } }))
            .collect();

        fixture
            .client()
            .append_block_children("page-1", &children)
            .await
            .expect("append block children");
    }

    // ---- retry tests ----

    #[tokio::test(start_paused = true)]
    async fn test_retry_on_429() {
        let fixture = TestFixture::new().await;

        // Mount success first (lower priority), then 429 (higher priority, exhausted after 1)
        Mock::given(method("GET"))
            .and(path("/blocks/b1/children"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "results": [] })))
            .mount(fixture.server())
            .await;

        Mock::given(method("GET"))
            .and(path("/blocks/b1/children"))
            .respond_with(
                ResponseTemplate::new(429)
                    .set_body_string("rate limited")
                    .insert_header("Retry-After", "1"),
            )
            .up_to_n_times(1)
            .mount(fixture.server())
            .await;

        // Should succeed after one retry
        let result = fixture
            .client()
            .get_block_children("b1")
            .await
            .expect("get block children");
        assert_eq!(
            result["results"].as_array().expect("should be array").len(),
            0
        );
    }

    #[tokio::test(start_paused = true)]
    async fn test_retry_on_500() {
        let fixture = TestFixture::new().await;

        Mock::given(method("GET"))
            .and(path("/blocks/b2/children"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "results": [] })))
            .mount(fixture.server())
            .await;

        Mock::given(method("GET"))
            .and(path("/blocks/b2/children"))
            .respond_with(ResponseTemplate::new(500).set_body_string("fail"))
            .up_to_n_times(2)
            .mount(fixture.server())
            .await;

        // Should succeed after two 500 retries
        let result = fixture
            .client()
            .get_block_children("b2")
            .await
            .expect("get block children");
        assert_eq!(
            result["results"].as_array().expect("should be array").len(),
            0
        );
    }

    #[tokio::test(start_paused = true)]
    async fn test_exhausted_retries_on_500() {
        let fixture = TestFixture::new().await;

        // Always return 500 → should fail after MAX_RETRIES + 1 attempts
        Mock::given(method("GET"))
            .and(path("/blocks/b3/children"))
            .respond_with(ResponseTemplate::new(500).set_body_string("persistent failure"))
            .expect(4) // 1 initial + 3 retries
            .mount(fixture.server())
            .await;

        let result = fixture.client().get_block_children("b3").await;
        assert!(result.is_err());
        let err = result.expect_err("should fail").to_string();
        assert!(err.contains("500"), "should contain status: {err}");
    }

    #[tokio::test]
    async fn test_no_retry_on_4xx() {
        let fixture = TestFixture::new().await;

        // 400 should NOT be retried
        Mock::given(method("GET"))
            .and(path("/blocks/b4/children"))
            .respond_with(ResponseTemplate::new(400).set_body_string("bad request"))
            .expect(1) // exactly one attempt, no retries
            .mount(fixture.server())
            .await;

        let result = fixture.client().get_block_children("b4").await;
        assert!(result.is_err());
    }
}
