//! Hashnode GraphQL API client.
use typub_adapters_core::debug;

use crate::model::{
    CreateDraftData, DraftResponse, FindPostData, GraphQLResponse, HashnodePostResponse,
    PublishPostData, UpdateDraftData, UpdatePostData,
};
use anyhow::{Context, Result};
use reqwest::Client;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::Deserialize;
use serde_json::json;
use typub_adapters_core::http_utils;

/// GraphQL mutation for publishing a new post.
const PUBLISH_POST_MUTATION: &str = r#"
mutation PublishPost($input: PublishPostInput!) {
    publishPost(input: $input) {
        post {
            id
            url
            slug
        }
    }
}
"#;

/// GraphQL mutation for updating an existing post.
const UPDATE_POST_MUTATION: &str = r#"
mutation UpdatePost($input: UpdatePostInput!) {
    updatePost(input: $input) {
        post {
            id
            url
            slug
        }
    }
}
"#;

/// GraphQL query to find a post by slug within a publication.
const FIND_POST_BY_SLUG_QUERY: &str = r#"
query FindPost($host: String!, $slug: String!) {
    publication(host: $host) {
        post(slug: $slug) {
            id
            url
        }
    }
}
"#;

/// GraphQL mutation for creating a draft (unpublished post).
const CREATE_DRAFT_MUTATION: &str = r#"
mutation CreateDraft($input: CreateDraftInput!) {
    createDraft(input: $input) {
        draft {
            id
        }
    }
}
"#;

/// GraphQL mutation for updating an existing draft.
const UPDATE_DRAFT_MUTATION: &str = r#"
mutation UpdateDraft($input: UpdateDraftInput!) {
    updateDraft(input: $input) {
        draft {
            id
        }
    }
}
"#;

/// Hashnode GraphQL API client.
pub(super) struct HashnodeClient<'a> {
    pub client: &'a Client,
    pub base_url: &'a str,
    pub api_key: Option<&'a str>,
    pub publication_id: Option<&'a str>,
    pub publication_host: Option<&'a str>,
    /// If false, create draft instead of publishing.
    pub published: bool,
}

impl<'a> HashnodeClient<'a> {
    pub fn new(
        client: &'a Client,
        base_url: &'a str,
        api_key: Option<&'a str>,
        publication_id: Option<&'a str>,
        publication_host: Option<&'a str>,
        published: bool,
    ) -> Self {
        Self {
            client,
            base_url,
            api_key,
            publication_id,
            publication_host,
            published,
        }
    }

    pub fn auth_key(&self) -> Result<&str> {
        self.api_key
            .ok_or_else(|| anyhow::anyhow!("hashnode.api_key or HASHNODE_API_KEY is required"))
    }

    pub fn publication_id(&self) -> Result<&str> {
        self.publication_id.ok_or_else(|| {
            anyhow::anyhow!("hashnode.publication_id or HASHNODE_PUBLICATION_ID is required")
        })
    }

    pub fn auth_headers(&self) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            HeaderValue::from_str(self.auth_key()?)
                .context("Invalid HashNode API key header value")?,
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        Ok(headers)
    }

    pub async fn execute_graphql<T: for<'de> Deserialize<'de>>(
        &self,
        query: &str,
        variables: serde_json::Value,
    ) -> Result<T> {
        let payload = json!({
            "query": query,
            "variables": variables
        });

        let response = self
            .client
            .post(self.base_url)
            .headers(self.auth_headers()?)
            .json(&payload)
            .send()
            .await
            .context("Failed to send HashNode GraphQL request")?;

        let response = http_utils::ensure_success(response, "HashNode GraphQL").await?;
        let gql_response: GraphQLResponse<T> = response
            .json()
            .await
            .context("Failed to parse HashNode GraphQL response")?;

        if let Some(errors) = gql_response.errors {
            let messages: Vec<_> = errors.iter().map(|e| e.message.as_str()).collect();
            anyhow::bail!("HashNode GraphQL errors: {}", messages.join("; "));
        }

        gql_response
            .data
            .ok_or_else(|| anyhow::anyhow!("HashNode GraphQL response missing data"))
    }

    /// Create a new post or draft based on `self.published`.
    ///
    /// - `published = true` → `publishPost` mutation (returns post with URL)
    /// - `published = false` → `createDraft` mutation (returns draft id only)
    pub async fn publish_post(
        &self,
        title: &str,
        content_markdown: &str,
        tags: &[String],
    ) -> Result<HashnodePostResponse> {
        let tag_objects: Vec<_> = tags
            .iter()
            .take(5)
            .map(|t| {
                json!({
                    "slug": t.to_lowercase().replace(' ', "-"),
                    "name": t
                })
            })
            .collect();

        if self.published {
            // Create published post
            let variables = json!({
                "input": {
                    "publicationId": self.publication_id()?,
                    "title": title,
                    "contentMarkdown": content_markdown,
                    "tags": tag_objects
                }
            });

            let data: PublishPostData = self
                .execute_graphql(PUBLISH_POST_MUTATION, variables)
                .await?;
            Ok(data.publish_post.post)
        } else {
            // Create draft
            let draft = self
                .create_draft(title, content_markdown, &tag_objects)
                .await?;
            // Convert draft response to post response (no URL for drafts)
            Ok(HashnodePostResponse {
                id: draft.id.clone(),
                url: format!("https://hashnode.com/draft/{}", draft.id),
                slug: String::new(),
            })
        }
    }

    /// Create a draft (unpublished post).
    async fn create_draft(
        &self,
        title: &str,
        content_markdown: &str,
        tag_objects: &[serde_json::Value],
    ) -> Result<DraftResponse> {
        let variables = json!({
            "input": {
                "publicationId": self.publication_id()?,
                "title": title,
                "contentMarkdown": content_markdown,
                "tags": tag_objects
            }
        });

        let data: CreateDraftData = self
            .execute_graphql(CREATE_DRAFT_MUTATION, variables)
            .await?;
        Ok(data.create_draft.draft)
    }

    /// Update an existing post or draft based on `self.published`.
    ///
    /// Note: This only updates content. It does NOT change publish status.
    /// - If the existing item is a published post, it stays published.
    /// - If the existing item is a draft, it stays as draft.
    pub async fn update_post(
        &self,
        post_id: &str,
        title: &str,
        content_markdown: &str,
        tags: &[String],
    ) -> Result<Option<HashnodePostResponse>> {
        let tag_objects: Vec<_> = tags
            .iter()
            .take(5)
            .map(|t| {
                json!({
                    "slug": t.to_lowercase().replace(' ', "-"),
                    "name": t
                })
            })
            .collect();

        if self.published {
            // Update published post
            let variables = json!({
                "input": {
                    "id": post_id,
                    "title": title,
                    "contentMarkdown": content_markdown,
                    "tags": tag_objects
                }
            });

            match self
                .execute_graphql::<UpdatePostData>(UPDATE_POST_MUTATION, variables)
                .await
            {
                Ok(data) => Ok(Some(data.update_post.post)),
                Err(e) => {
                    let msg = e.to_string();
                    // GraphQL returns error if post not found
                    if msg.contains("NOT_FOUND") || msg.contains("not found") {
                        Ok(None)
                    } else {
                        Err(e)
                    }
                }
            }
        } else {
            // Update draft - note: UpdateDraftInput uses "draftId" not "id"
            let variables = json!({
                "input": {
                    "draftId": post_id,
                    "title": title,
                    "contentMarkdown": content_markdown,
                    "tags": tag_objects
                }
            });

            match self
                .execute_graphql::<UpdateDraftData>(UPDATE_DRAFT_MUTATION, variables)
                .await
            {
                Ok(data) => {
                    let draft = data.update_draft.draft;
                    Ok(Some(HashnodePostResponse {
                        id: draft.id.clone(),
                        url: format!("https://hashnode.com/draft/{}", draft.id),
                        slug: String::new(),
                    }))
                }
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("NOT_FOUND") || msg.contains("not found") {
                        Ok(None)
                    } else {
                        Err(e)
                    }
                }
            }
        }
    }

    /// Slug-based lookup per [[RFC-0003:C-DECISION-KEY]] step 2.
    pub async fn find_post_by_slug(&self, slug: &str) -> Result<Option<HashnodePostResponse>> {
        let host = match self.publication_host {
            Some(h) => h,
            None => {
                // Cannot do slug lookup without publication host
                return Ok(None);
            }
        };

        let variables = json!({
            "host": host,
            "slug": slug
        });

        match self
            .execute_graphql::<FindPostData>(FIND_POST_BY_SLUG_QUERY, variables)
            .await
        {
            Ok(data) => Ok(data.publication.and_then(|p| p.post)),
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("NOT_FOUND") || msg.contains("not found") {
                    Ok(None)
                } else {
                    Err(e)
                }
            }
        }
    }

    /// Slug lookup then update, or create as last resort.
    ///
    /// Shared logic for both the "stale cached ID" and "no cached ID" branches
    /// of [[RFC-0003:C-DECISION-KEY]] steps 2–3.
    pub async fn update_or_create_by_slug(
        &self,
        slug: &str,
        title: &str,
        content_markdown: &str,
        tags: &[String],
    ) -> Result<HashnodePostResponse> {
        if let Some(found) = self.find_post_by_slug(slug).await? {
            debug!("Found existing HashNode post by slug: id={}", found.id);
            self.update_post(&found.id, title, content_markdown, tags)
                .await?
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Slug-matched HashNode post {} also returned not found",
                        found.id
                    )
                })
        } else {
            self.publish_post(title, content_markdown, tags).await
        }
    }
}
