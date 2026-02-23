//! Shared HTTP helpers for API-style adapters.

use anyhow::{Context, Result};

/// Ensure an HTTP response indicates success, otherwise return an error with body details.
pub async fn ensure_success(
    response: reqwest::Response,
    operation: &str,
) -> Result<reqwest::Response> {
    if response.status().is_success() {
        return Ok(response);
    }

    let status = response.status();
    let body = response
        .text()
        .await
        .with_context(|| format!("Failed to read error body for {operation}"))?;
    anyhow::bail!("{operation} failed with {status}: {body}");
}

/// Like [`ensure_success`], but escalates 401/403 responses with a
/// platform-specific authentication hint (e.g. "Check wordpress.jwt_token.").
pub async fn ensure_success_with_auth_hint(
    response: reqwest::Response,
    operation: &str,
    auth_hint: &str,
) -> Result<reqwest::Response> {
    if response.status().is_success() {
        return Ok(response);
    }

    let status = response.status();
    let body = response
        .text()
        .await
        .with_context(|| format!("Failed to read error body for {operation}"))?;
    if status.as_u16() == 401 || status.as_u16() == 403 {
        anyhow::bail!(
            "{} auth failed ({}): {}. {}",
            operation,
            status,
            body,
            auth_hint
        );
    }
    anyhow::bail!("{} error ({}): {}", operation, status, body);
}
