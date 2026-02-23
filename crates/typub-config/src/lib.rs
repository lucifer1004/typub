//! Configuration types for typub.
//!
//! This crate provides the core configuration structures used by typub:
//! - `Config` — main configuration from `typub.toml`
//! - `PlatformConfig` — per-platform configuration
//! - `StorageConfig` — S3-compatible storage configuration per [[RFC-0004]]
//!
//! Extracted per [[RFC-0007:C-SHARED-TYPES]] to enable adapter subcrates
//! to depend on configuration without circular dependencies.

pub mod project;

use anyhow::{Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use url::Url;

// Re-export ThemeId from typub-core
pub use typub_core::ThemeId;

/// Expand environment variables in a string using shell-like syntax.
///
/// Supports:
/// - `$VAR` or `${VAR}` — substitute from environment
/// - `${VAR:default}` — use default if VAR is unset
///
/// If expansion fails, returns the original string unchanged.
fn expand_env_vars(s: &str) -> String {
    subst::substitute(s, &subst::Env).unwrap_or_else(|_| s.to_string())
}

/// Main configuration structure
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default = "default_content_dir")]
    pub content_dir: PathBuf,
    #[serde(default = "default_output_dir")]
    pub output_dir: PathBuf,
    /// Global storage configuration for External asset strategy
    /// Per [[RFC-0004:C-STORAGE-CONFIG]]
    #[serde(default)]
    pub storage: Option<StorageConfig>,
    /// Global default for published state (layer 4 per [[RFC-0005:C-RESOLUTION-ORDER]])
    #[serde(default)]
    pub published: Option<bool>,
    /// Global default theme (layer 4 in theme resolution chain)
    #[serde(default)]
    pub theme: Option<ThemeId>,
    /// Global default platform for internal link resolution in copypaste adapters.
    /// Per-post `internal_link_target` in meta.toml overrides this.
    /// If neither is set, auto-selects first published platform alphabetically.
    #[serde(default)]
    pub internal_link_target: Option<String>,
    /// Global Typst render preamble override (layer 4 per [[RFC-0005:C-RESOLUTION-ORDER]]).
    #[serde(default)]
    pub preamble: Option<String>,
    #[serde(default)]
    pub platforms: HashMap<String, PlatformConfig>,
}

fn default_content_dir() -> PathBuf {
    PathBuf::from("posts")
}

fn default_output_dir() -> PathBuf {
    PathBuf::from("output")
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlatformConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub asset_strategy: Option<String>,
    /// Platform-specific published setting (layer 3 per [[RFC-0005:C-RESOLUTION-ORDER]])
    #[serde(default)]
    pub published: Option<bool>,
    /// Platform-specific theme (layer 3 in theme resolution chain)
    #[serde(default)]
    pub theme: Option<ThemeId>,
    /// Platform-specific internal link target (layer 3 in resolution chain)
    #[serde(default)]
    pub internal_link_target: Option<String>,
    /// Math rendering strategy override (layer 3 per [[RFC-0005:C-RESOLUTION-ORDER]])
    /// Per [[WI-2026-02-17-002]].
    #[serde(default)]
    pub math_rendering: Option<String>,
    /// Math delimiter syntax override (layer 3 per [[RFC-0005:C-RESOLUTION-ORDER]])
    /// Per [[WI-2026-02-17-002]].
    #[serde(default)]
    pub math_delimiters: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, toml::Value>,
}

fn default_true() -> bool {
    true
}

/// Result of loading configuration.
pub enum ConfigLoadResult {
    /// Configuration loaded successfully.
    Loaded(Config),
    /// Configuration file not found, using defaults.
    /// The String contains the path that was not found.
    DefaultsUsed(Config, String),
}

impl Config {
    /// Load configuration from a file.
    ///
    /// Returns `ConfigLoadResult::DefaultsUsed` if the file does not exist,
    /// allowing the caller to decide how to handle the warning.
    pub fn load(path: &Path) -> Result<ConfigLoadResult> {
        if !path.exists() {
            return Ok(ConfigLoadResult::DefaultsUsed(
                Self::default(),
                path.display().to_string(),
            ));
        }

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

        Ok(ConfigLoadResult::Loaded(config))
    }

    /// Load configuration, returning defaults if file not found.
    ///
    /// This is a convenience method that discards the "defaults used" information.
    /// Use `load()` if you need to know whether defaults were used.
    pub fn load_or_default(path: &Path) -> Result<Config> {
        match Self::load(path)? {
            ConfigLoadResult::Loaded(config) => Ok(config),
            ConfigLoadResult::DefaultsUsed(config, _) => Ok(config),
        }
    }

    /// Get platform configuration by ID
    pub fn get_platform(&self, id: &str) -> Option<&PlatformConfig> {
        self.platforms.get(id)
    }

    /// Get platforms that should be targeted by default (present and not disabled).
    pub fn default_platforms(&self) -> Vec<(&str, &PlatformConfig)> {
        self.platforms
            .iter()
            .filter(|(_, c)| c.enabled)
            .map(|(k, v)| (k.as_str(), v))
            .collect()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            content_dir: default_content_dir(),
            output_dir: default_output_dir(),
            storage: None,
            published: None,
            theme: None,
            internal_link_target: None,
            preamble: None,
            platforms: HashMap::new(),
        }
    }
}

impl PlatformConfig {
    /// Get a string value from extra config, expanding environment variables.
    ///
    /// Supports shell-like variable substitution via the `subst` crate:
    /// - `$VAR` or `${VAR}` — substitute from environment
    /// - `${VAR:default}` — use default if VAR is unset
    ///
    /// If expansion fails (e.g., undefined variable without default),
    /// returns the original unexpanded string.
    pub fn get_str(&self, key: &str) -> Option<String> {
        self.extra
            .get(key)
            .and_then(|v| v.as_str())
            .map(expand_env_vars)
    }

    /// Get a raw string value without environment variable expansion.
    pub fn get_str_raw(&self, key: &str) -> Option<&str> {
        self.extra.get(key).and_then(|v| v.as_str())
    }

    /// Get an integer value from extra config
    pub fn get_int(&self, key: &str) -> Option<i64> {
        self.extra.get(key).and_then(|v| v.as_integer())
    }

    /// Get a boolean value from extra config
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.extra.get(key).and_then(|v| v.as_bool())
    }

    /// Get platform-specific storage config override
    pub fn get_storage(&self) -> Option<StorageConfig> {
        self.extra.get("storage").and_then(|v| {
            let table = v.as_table()?;
            let toml_str = toml::to_string(table).ok()?;
            toml::from_str(&toml_str).ok()
        })
    }
}

/// Storage configuration for external asset storage
/// Per [[RFC-0004:C-STORAGE-CONFIG]]
#[derive(Debug, Clone, Deserialize, Default)]
pub struct StorageConfig {
    /// Storage type (e.g., "s3")
    #[serde(rename = "type", default)]
    pub storage_type: Option<String>,
    /// S3-compatible endpoint URL (optional)
    #[serde(default)]
    pub endpoint: Option<String>,
    /// Bucket name
    #[serde(default)]
    pub bucket: Option<String>,
    /// Region (e.g., "us-east-1", "auto" for R2)
    #[serde(default)]
    pub region: Option<String>,
    /// URL prefix for constructing public asset URLs
    #[serde(default)]
    pub url_prefix: Option<String>,
    /// Access key ID (or use S3_ACCESS_KEY_ID env var)
    #[serde(default)]
    pub access_key_id: Option<String>,
    /// Secret access key (or use S3_SECRET_ACCESS_KEY env var)
    #[serde(default)]
    pub secret_access_key: Option<String>,
}

impl StorageConfig {
    /// Resolve a field value using RFC-0004 precedence ladder:
    /// 1. Platform-specific env var (e.g., HASHNODE_S3_BUCKET)
    /// 2. Platform-specific config value
    /// 3. Global env var (e.g., S3_BUCKET)
    /// 4. Global config value
    fn resolve_field(
        platform_id: Option<&str>,
        platform_value: Option<&str>,
        global_value: Option<&str>,
        env_suffix: &str,
    ) -> Option<String> {
        // 1. Platform-specific env var
        if let Some(pid) = platform_id {
            let env_key = format!("{}_{}", pid.to_uppercase(), env_suffix);
            if let Ok(val) = std::env::var(&env_key)
                && !val.is_empty()
            {
                return Some(val);
            }
        }

        // 2. Platform-specific config value
        if let Some(val) = platform_value
            && !val.is_empty()
        {
            return Some(val.to_string());
        }

        // 3. Global env var
        if let Ok(val) = std::env::var(env_suffix)
            && !val.is_empty()
        {
            return Some(val);
        }

        // 4. Global config value
        global_value
            .filter(|v| !v.is_empty())
            .map(|v| v.to_string())
    }

    /// Merge global and platform-specific config with env var precedence
    /// Per [[RFC-0004:C-STORAGE-CONFIG]] precedence ladder
    pub fn resolve(
        global: Option<&StorageConfig>,
        platform: Option<&StorageConfig>,
        platform_id: &str,
    ) -> StorageConfig {
        let g = global.cloned().unwrap_or_default();
        let p = platform.cloned().unwrap_or_default();

        StorageConfig {
            storage_type: Self::resolve_field(
                Some(platform_id),
                p.storage_type.as_deref(),
                g.storage_type.as_deref(),
                "S3_TYPE",
            ),
            endpoint: Self::resolve_field(
                Some(platform_id),
                p.endpoint.as_deref(),
                g.endpoint.as_deref(),
                "S3_ENDPOINT",
            ),
            bucket: Self::resolve_field(
                Some(platform_id),
                p.bucket.as_deref(),
                g.bucket.as_deref(),
                "S3_BUCKET",
            ),
            region: Self::resolve_field(
                Some(platform_id),
                p.region.as_deref(),
                g.region.as_deref(),
                "S3_REGION",
            ),
            url_prefix: Self::resolve_field(
                Some(platform_id),
                p.url_prefix.as_deref(),
                g.url_prefix.as_deref(),
                "S3_URL_PREFIX",
            ),
            access_key_id: Self::resolve_field(
                Some(platform_id),
                p.access_key_id.as_deref(),
                g.access_key_id.as_deref(),
                "S3_ACCESS_KEY_ID",
            ),
            secret_access_key: Self::resolve_field(
                Some(platform_id),
                p.secret_access_key.as_deref(),
                g.secret_access_key.as_deref(),
                "S3_SECRET_ACCESS_KEY",
            ),
        }
    }

    /// Validate required fields are present
    /// Per [[RFC-0004:C-STORAGE-CONFIG]]
    pub fn validate(&self) -> Result<()> {
        if self.storage_type.is_none() {
            anyhow::bail!(
                "Storage configuration missing 'type' field. \
                Set S3_TYPE env var or add type = \"s3\" to [storage] config."
            );
        }
        if self.bucket.is_none() {
            anyhow::bail!(
                "Storage configuration missing 'bucket' field. \
                Set S3_BUCKET env var or add bucket = \"your-bucket\" to [storage] config."
            );
        }
        if self.url_prefix.is_none() {
            anyhow::bail!(
                "Storage configuration missing 'url_prefix' field. \
                Set S3_URL_PREFIX env var or add url_prefix to [storage] config."
            );
        }
        Ok(())
    }

    /// Compute storage configuration identifier (64-char SHA-256 hex)
    /// Per [[RFC-0004:C-STORAGE-CONFIG]]
    /// Excludes credentials, includes all other fields with normalization.
    pub fn config_id(&self) -> String {
        let storage_type = self
            .storage_type
            .as_deref()
            .unwrap_or("")
            .to_lowercase()
            .trim()
            .to_string();

        let endpoint = self
            .endpoint
            .as_deref()
            .map(Self::normalize_url)
            .unwrap_or_default();

        let bucket = self.bucket.as_deref().unwrap_or("").to_string();

        let region = self
            .region
            .as_deref()
            .unwrap_or("")
            .to_lowercase()
            .trim()
            .to_string();

        let url_prefix = self
            .url_prefix
            .as_deref()
            .map(Self::normalize_url)
            .unwrap_or_default();

        let concatenated = format!(
            "{}|{}|{}|{}|{}",
            storage_type, endpoint, bucket, region, url_prefix
        );

        let mut hasher = Sha256::new();
        hasher.update(concatenated.as_bytes());
        let result = hasher.finalize();
        hex::encode(result)
    }

    /// Normalize URL: lowercase scheme+host, preserve path case, remove trailing slash and default ports
    fn normalize_url(url_str: &str) -> String {
        let trimmed = url_str.trim();
        if trimmed.is_empty() {
            return String::new();
        }

        // Try to parse as URL
        if let Ok(mut url) = Url::parse(trimmed) {
            // Lowercase scheme (already done by Url)
            // Lowercase host
            if let Some(host) = url.host_str() {
                let lower_host = host.to_lowercase();
                // Reconstruct URL with lowercase host
                let _ = url.set_host(Some(&lower_host));
            }

            // Remove default ports
            if let Some(port) = url.port() {
                let scheme = url.scheme();
                if (scheme == "https" && port == 443) || (scheme == "http" && port == 80) {
                    let _ = url.set_port(None);
                }
            }

            // Get the string and remove trailing slash from path
            let mut result = url.to_string();
            while result.ends_with('/') {
                result.pop();
            }
            result
        } else {
            // Not a valid URL, just trim and remove trailing slashes
            let mut result = trimmed.to_string();
            while result.ends_with('/') {
                result.pop();
            }
            result
        }
    }

    /// Get normalized URL prefix (trailing slashes removed)
    pub fn normalized_url_prefix(&self) -> Option<String> {
        self.url_prefix.as_deref().map(Self::normalize_url)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use std::io::Write;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.content_dir, PathBuf::from("posts"));
        assert_eq!(config.output_dir, PathBuf::from("output"));
        assert!(config.platforms.is_empty());
    }

    #[test]
    fn test_load_nonexistent_returns_default() {
        let result =
            Config::load(Path::new("/tmp/does-not-exist-config.toml")).expect("load config");
        match result {
            ConfigLoadResult::DefaultsUsed(config, path) => {
                assert_eq!(config.content_dir, PathBuf::from("posts"));
                assert!(path.contains("does-not-exist"));
            }
            ConfigLoadResult::Loaded(_) => panic!("expected DefaultsUsed"),
        }
    }

    #[test]
    fn test_load_full_config() {
        let toml = r#"
content_dir = "articles"
output_dir = "build"

[platforms.astro]
enabled = true
output_dir = "/var/www"

[platforms.notion]
enabled = false
data_source_id = "ds-123"

[platforms.confluence]
enabled = true
space_key = "TEAM"
"#;
        let mut tmp = tempfile::NamedTempFile::new().expect("create temp file");
        tmp.write_all(toml.as_bytes()).expect("write config");

        let result = Config::load(tmp.path()).expect("load config");
        let config = match result {
            ConfigLoadResult::Loaded(c) => c,
            ConfigLoadResult::DefaultsUsed(_, _) => panic!("expected Loaded"),
        };

        assert_eq!(config.content_dir, PathBuf::from("articles"));
        assert_eq!(config.output_dir, PathBuf::from("build"));
        assert_eq!(config.platforms.len(), 3);
    }

    #[test]
    fn test_load_global_preamble() {
        let toml = r##"
preamble = "#set text(fill: red)"
"##;
        let mut tmp = tempfile::NamedTempFile::new().expect("create temp file");
        tmp.write_all(toml.as_bytes()).expect("write config");

        let result = Config::load(tmp.path()).expect("load config");
        let config = match result {
            ConfigLoadResult::Loaded(c) => c,
            ConfigLoadResult::DefaultsUsed(_, _) => panic!("expected Loaded"),
        };

        assert_eq!(config.preamble.as_deref(), Some("#set text(fill: red)"));
    }

    #[test]
    fn test_load_invalid_toml() {
        let mut tmp = tempfile::NamedTempFile::new().expect("create temp file");
        tmp.write_all(b"this is not { valid toml")
            .expect("write config");
        assert!(Config::load(tmp.path()).is_err());
    }

    #[test]
    fn test_get_platform() {
        let config: Config = toml::from_str(
            r#"
[platforms.astro]
output_dir = "/var/www"
"#,
        )
        .expect("parse TOML");

        assert!(config.get_platform("astro").is_some());
        assert!(config.get_platform("notion").is_none());
    }

    #[test]
    fn test_default_platforms() {
        let config: Config = toml::from_str(
            r#"
[platforms.astro]
enabled = true
[platforms.notion]
enabled = false
[platforms.wechat]
"#,
        )
        .expect("parse TOML");

        let defaults = config.default_platforms();
        let names: Vec<&str> = defaults.iter().map(|(k, _)| *k).collect();
        assert!(names.contains(&"astro"));
        assert!(names.contains(&"wechat")); // default is true
        assert!(!names.contains(&"notion"));
    }

    #[test]
    fn test_platform_config_accessors() {
        let config: Config = toml::from_str(
            r#"
[platforms.test]
output_dir = "/tmp/out"
max_retries = 3
dry_run = true
"#,
        )
        .expect("parse TOML");

        let pc = config.get_platform("test").expect("platform should exist");
        assert_eq!(pc.get_str("output_dir"), Some("/tmp/out".to_string()));
        assert_eq!(pc.get_int("max_retries"), Some(3));
        assert_eq!(pc.get_bool("dry_run"), Some(true));

        // Missing keys
        assert_eq!(pc.get_str("nonexistent"), None);
        assert_eq!(pc.get_int("output_dir"), None); // wrong type
        assert_eq!(pc.get_bool("output_dir"), None); // wrong type
    }

    #[test]
    fn test_minimal_config_uses_defaults() {
        let config: Config = toml::from_str("").expect("parse TOML");
        assert_eq!(config.content_dir, PathBuf::from("posts"));
        assert_eq!(config.output_dir, PathBuf::from("output"));
        assert!(config.platforms.is_empty());
    }

    // --- Environment variable substitution tests ---

    #[test]
    fn test_get_str_expands_env_var() {
        // SAFETY: Test runs in single thread; no concurrent access to this env var
        unsafe {
            std::env::set_var("TYPUB_TEST_API_KEY", "secret123");
        }
        let config: Config = toml::from_str(
            r#"
[platforms.test]
api_key = "$TYPUB_TEST_API_KEY"
"#,
        )
        .expect("parse TOML");

        let pc = config.get_platform("test").expect("platform should exist");
        assert_eq!(pc.get_str("api_key"), Some("secret123".to_string()));
        // SAFETY: Test cleanup
        unsafe {
            std::env::remove_var("TYPUB_TEST_API_KEY");
        }
    }

    #[test]
    fn test_get_str_expands_env_var_long_format() {
        // SAFETY: Test runs in single thread; no concurrent access to this env var
        unsafe {
            std::env::set_var("TYPUB_TEST_TOKEN", "token456");
        }
        let config: Config = toml::from_str(
            r#"
[platforms.test]
token = "${TYPUB_TEST_TOKEN}"
"#,
        )
        .expect("parse TOML");

        let pc = config.get_platform("test").expect("platform should exist");
        assert_eq!(pc.get_str("token"), Some("token456".to_string()));
        // SAFETY: Test cleanup
        unsafe {
            std::env::remove_var("TYPUB_TEST_TOKEN");
        }
    }

    #[test]
    fn test_get_str_expands_env_var_with_default() {
        // Ensure the var is NOT set
        // SAFETY: Test runs in single thread; no concurrent access to this env var
        unsafe {
            std::env::remove_var("TYPUB_UNDEFINED_VAR");
        }
        let config: Config = toml::from_str(
            r#"
[platforms.test]
value = "${TYPUB_UNDEFINED_VAR:fallback_value}"
"#,
        )
        .expect("parse TOML");

        let pc = config.get_platform("test").expect("platform should exist");
        assert_eq!(pc.get_str("value"), Some("fallback_value".to_string()));
    }

    #[test]
    fn test_get_str_returns_original_on_undefined_var() {
        // When a var is undefined and no default is provided, subst returns an error
        // and we fall back to the original string
        // SAFETY: Test runs in single thread; no concurrent access to this env var
        unsafe {
            std::env::remove_var("TYPUB_NONEXISTENT_VAR");
        }
        let config: Config = toml::from_str(
            r#"
[platforms.test]
value = "${TYPUB_NONEXISTENT_VAR}"
"#,
        )
        .expect("parse TOML");

        let pc = config.get_platform("test").expect("platform should exist");
        // Should return original unexpanded string on error
        assert_eq!(
            pc.get_str("value"),
            Some("${TYPUB_NONEXISTENT_VAR}".to_string())
        );
    }

    #[test]
    fn test_get_str_no_expansion_for_plain_string() {
        let config: Config = toml::from_str(
            r#"
[platforms.test]
plain = "no variables here"
"#,
        )
        .expect("parse TOML");

        let pc = config.get_platform("test").expect("platform should exist");
        assert_eq!(pc.get_str("plain"), Some("no variables here".to_string()));
    }

    #[test]
    fn test_get_str_raw_does_not_expand() {
        // SAFETY: Test runs in single thread; no concurrent access to this env var
        unsafe {
            std::env::set_var("TYPUB_TEST_RAW", "expanded");
        }
        let config: Config = toml::from_str(
            r#"
[platforms.test]
raw_value = "$TYPUB_TEST_RAW"
"#,
        )
        .expect("parse TOML");

        let pc = config.get_platform("test").expect("platform should exist");
        // get_str_raw should NOT expand
        assert_eq!(pc.get_str_raw("raw_value"), Some("$TYPUB_TEST_RAW"));
        // get_str SHOULD expand
        assert_eq!(pc.get_str("raw_value"), Some("expanded".to_string()));
        // SAFETY: Test cleanup
        unsafe {
            std::env::remove_var("TYPUB_TEST_RAW");
        }
    }

    // --- StorageConfig tests per [[RFC-0004]] ---

    #[test]
    fn test_storage_config_parse() {
        let config: Config = toml::from_str(
            r#"
[storage]
type = "s3"
endpoint = "https://xxx.r2.cloudflarestorage.com"
bucket = "my-assets"
region = "auto"
url_prefix = "https://cdn.example.com/assets"
"#,
        )
        .expect("parse TOML");

        let storage = config.storage.expect("storage should be present");
        assert_eq!(storage.storage_type, Some("s3".to_string()));
        assert_eq!(
            storage.endpoint,
            Some("https://xxx.r2.cloudflarestorage.com".to_string())
        );
        assert_eq!(storage.bucket, Some("my-assets".to_string()));
        assert_eq!(storage.region, Some("auto".to_string()));
        assert_eq!(
            storage.url_prefix,
            Some("https://cdn.example.com/assets".to_string())
        );
    }

    #[test]
    fn test_storage_config_id_deterministic() {
        let config1 = StorageConfig {
            storage_type: Some("s3".to_string()),
            endpoint: Some("https://xxx.r2.cloudflarestorage.com".to_string()),
            bucket: Some("my-assets".to_string()),
            region: Some("auto".to_string()),
            url_prefix: Some("https://cdn.example.com/assets".to_string()),
            access_key_id: Some("key1".to_string()),
            secret_access_key: Some("secret1".to_string()),
        };
        let config2 = StorageConfig {
            storage_type: Some("s3".to_string()),
            endpoint: Some("https://xxx.r2.cloudflarestorage.com".to_string()),
            bucket: Some("my-assets".to_string()),
            region: Some("auto".to_string()),
            url_prefix: Some("https://cdn.example.com/assets".to_string()),
            access_key_id: Some("different_key".to_string()),
            secret_access_key: Some("different_secret".to_string()),
        };

        // Config ID should be identical since credentials are excluded
        assert_eq!(config1.config_id(), config2.config_id());
        assert_eq!(config1.config_id().len(), 64); // SHA-256 = 64 hex chars
    }

    #[test]
    fn test_storage_config_id_differs_by_bucket() {
        let config1 = StorageConfig {
            storage_type: Some("s3".to_string()),
            bucket: Some("bucket-a".to_string()),
            ..Default::default()
        };
        let config2 = StorageConfig {
            storage_type: Some("s3".to_string()),
            bucket: Some("bucket-b".to_string()),
            ..Default::default()
        };

        assert_ne!(config1.config_id(), config2.config_id());
    }

    #[test]
    fn test_storage_config_normalize_url_trailing_slash() {
        let config = StorageConfig {
            url_prefix: Some("https://cdn.example.com/assets/".to_string()),
            ..Default::default()
        };
        let normalized = config.normalized_url_prefix().expect("prefix");
        assert!(!normalized.ends_with('/'));
        assert_eq!(normalized, "https://cdn.example.com/assets");
    }

    #[test]
    fn test_storage_config_normalize_url_lowercase_host() {
        let config = StorageConfig {
            endpoint: Some("https://S3.US-EAST-1.AMAZONAWS.COM".to_string()),
            ..Default::default()
        };
        let _id = config.config_id();
        // The ID should include the normalized URL, different case should produce same ID
        let config2 = StorageConfig {
            endpoint: Some("https://s3.us-east-1.amazonaws.com".to_string()),
            ..Default::default()
        };
        assert_eq!(config.config_id(), config2.config_id());
    }

    #[test]
    fn test_storage_config_validate_missing_type() {
        let config = StorageConfig {
            bucket: Some("my-bucket".to_string()),
            url_prefix: Some("https://cdn.example.com".to_string()),
            ..Default::default()
        };
        let err = config.validate().expect_err("should fail");
        assert!(err.to_string().contains("type"));
    }

    #[test]
    fn test_storage_config_validate_missing_bucket() {
        let config = StorageConfig {
            storage_type: Some("s3".to_string()),
            url_prefix: Some("https://cdn.example.com".to_string()),
            ..Default::default()
        };
        let err = config.validate().expect_err("should fail");
        assert!(err.to_string().contains("bucket"));
    }

    #[test]
    fn test_storage_config_validate_missing_url_prefix() {
        let config = StorageConfig {
            storage_type: Some("s3".to_string()),
            bucket: Some("my-bucket".to_string()),
            ..Default::default()
        };
        let err = config.validate().expect_err("should fail");
        assert!(err.to_string().contains("url_prefix"));
    }

    #[test]
    fn test_storage_config_validate_ok() {
        let config = StorageConfig {
            storage_type: Some("s3".to_string()),
            bucket: Some("my-bucket".to_string()),
            url_prefix: Some("https://cdn.example.com".to_string()),
            ..Default::default()
        };
        config.validate().expect("should pass");
    }
}

// ============================================================================
// Config Resolution Functions
// ============================================================================

/// Resolve `published` using RFC-0005:C-RESOLUTION-ORDER 5-level chain:
/// 1. meta.toml[platforms.X].published — per-content platform-specific
/// 2. meta.toml.published — per-content default
/// 3. typub.toml[platforms.X].published — global platform-specific
/// 4. typub.toml.published — global default
/// 5. Adapter default (true)
///
/// Implements [[RFC-0005:C-RESOLUTION-ORDER]].
pub fn resolve_published(
    content_meta: &typub_core::ContentMeta,
    platform_id: &str,
    global_config: &Config,
) -> bool {
    // Layer 1: per-content platform-specific
    content_meta
        .platforms
        .get(platform_id)
        .and_then(|p| p.published)
        // Layer 2: per-content default
        .or(content_meta.published)
        // Layer 3: global platform-specific
        .or(global_config
            .platforms
            .get(platform_id)
            .and_then(|p| p.published))
        // Layer 4: global default
        .or(global_config.published)
        // Layer 5: adapter default
        .unwrap_or(true)
}

#[cfg(test)]
mod resolution_tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use typub_core::{ContentMeta, PostPlatformConfig};

    fn make_content_meta(published: Option<bool>, platform_published: Option<bool>) -> ContentMeta {
        let mut platforms = std::collections::HashMap::new();
        if platform_published.is_some() {
            platforms.insert(
                "hashnode".to_string(),
                PostPlatformConfig {
                    published: platform_published,
                    internal_link_target: None,
                    extra: std::collections::HashMap::new(),
                },
            );
        }
        ContentMeta {
            title: "Test".to_string(),
            created: chrono::NaiveDate::from_ymd_opt(2026, 1, 1).expect("valid date"),
            updated: None,
            tags: vec![],
            categories: vec![],
            published,
            theme: None,
            internal_link_target: None,
            preamble: None,
            platforms,
        }
    }

    fn make_global_config(published: Option<bool>, platform_published: Option<bool>) -> Config {
        let mut platforms = std::collections::HashMap::new();
        if platform_published.is_some() {
            platforms.insert(
                "hashnode".to_string(),
                PlatformConfig {
                    enabled: true,
                    asset_strategy: None,
                    published: platform_published,
                    theme: None,
                    internal_link_target: None,
                    math_rendering: None,
                    math_delimiters: None,
                    extra: std::collections::HashMap::new(),
                },
            );
        }
        Config {
            content_dir: std::path::PathBuf::from("posts"),
            output_dir: std::path::PathBuf::from("output"),
            storage: None,
            published,
            theme: None,
            internal_link_target: None,
            preamble: None,
            platforms,
        }
    }

    #[test]
    fn test_resolve_published_layer_1_per_content_platform_specific() {
        let meta = make_content_meta(Some(true), Some(false));
        let config = make_global_config(Some(true), Some(true));
        assert!(!resolve_published(&meta, "hashnode", &config));
    }

    #[test]
    fn test_resolve_published_layer_2_per_content_default() {
        let meta = make_content_meta(Some(false), None);
        let config = make_global_config(Some(true), Some(true));
        assert!(!resolve_published(&meta, "hashnode", &config));
    }

    #[test]
    fn test_resolve_published_layer_3_global_platform_specific() {
        let meta = make_content_meta(None, None);
        let config = make_global_config(Some(true), Some(false));
        assert!(!resolve_published(&meta, "hashnode", &config));
    }

    #[test]
    fn test_resolve_published_layer_4_global_default() {
        let meta = make_content_meta(None, None);
        let config = make_global_config(Some(false), None);
        assert!(!resolve_published(&meta, "hashnode", &config));
    }

    #[test]
    fn test_resolve_published_layer_5_adapter_default() {
        let meta = make_content_meta(None, None);
        let config = make_global_config(None, None);
        assert!(resolve_published(&meta, "hashnode", &config));
    }

    #[test]
    fn test_resolve_published_different_platform_uses_correct_layer() {
        let mut meta = make_content_meta(None, None);
        meta.published = Some(false);
        let config = make_global_config(Some(true), Some(true));
        assert!(!resolve_published(&meta, "hashnode", &config));
        assert!(!resolve_published(&meta, "devto", &config));
    }
}
