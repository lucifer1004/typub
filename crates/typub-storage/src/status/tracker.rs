//! Status tracker for managing publish status.
//!
//! SQLite-backed status tracking for content publishing.

use super::types::{AssetUploadRecord, PlatformStatus, PublishResult};
use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use typub_core::Content;

/// Status tracker for managing publish status
pub struct StatusTracker {
    conn: Connection,
    path: PathBuf,
    /// Project root for path normalization per [[RFC-0005:C-PROJECT-ROOT]]
    project_root: PathBuf,
}

impl StatusTracker {
    /// Load status with default configuration.
    /// Uses current directory as project root.
    pub fn load_default() -> Result<Self> {
        Self::load(Path::new("."))
    }

    /// Load status from disk
    ///
    /// # Arguments
    /// * `project_root` - The project root directory for path normalization
    pub fn load(project_root: &Path) -> Result<Self> {
        let status_dir = PathBuf::from(".typub");
        std::fs::create_dir_all(&status_dir)?;
        Self::open_at(status_dir.join("status.db"), project_root)
    }

    fn open_at(path: PathBuf, project_root: &Path) -> Result<Self> {
        let conn = Connection::open(&path)
            .with_context(|| format!("Failed to open status DB: {}", path.display()))?;
        conn.execute_batch(
            r#"
CREATE TABLE IF NOT EXISTS posts (
    slug TEXT PRIMARY KEY
);

CREATE TABLE IF NOT EXISTS platform_status (
    slug TEXT NOT NULL,
    platform TEXT NOT NULL,
    published INTEGER NOT NULL,
    url TEXT,
    platform_id TEXT,
    published_at TEXT,
    content_hash TEXT,
    PRIMARY KEY (slug, platform)
);

CREATE INDEX IF NOT EXISTS idx_platform_status_slug ON platform_status(slug);
CREATE INDEX IF NOT EXISTS idx_platform_status_platform ON platform_status(platform);

CREATE TABLE IF NOT EXISTS publish_reconcile (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    slug TEXT NOT NULL,
    platform TEXT NOT NULL,
    remote_url TEXT,
    remote_id TEXT,
    error TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_publish_reconcile_slug_platform ON publish_reconcile(slug, platform);

-- Asset upload tracking per [[RFC-0004:C-UPLOAD-TRACKING]]
-- Two-index model: content_index and path_index

CREATE TABLE IF NOT EXISTS asset_uploads (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    local_path TEXT NOT NULL,
    storage_config_id TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    extension TEXT NOT NULL,
    remote_key TEXT NOT NULL,
    remote_url TEXT NOT NULL,
    uploaded_at TEXT NOT NULL,
    UNIQUE(storage_config_id, content_hash, extension)
);

CREATE INDEX IF NOT EXISTS idx_asset_uploads_content
    ON asset_uploads(storage_config_id, content_hash, extension);
CREATE INDEX IF NOT EXISTS idx_asset_uploads_path
    ON asset_uploads(local_path, storage_config_id);
"#,
        )
        .context("Failed to initialize status DB schema")?;

        // Migration: add remote_status column per [[RFC-0005:C-STATUS-TRACKING]]
        // This is safe to run multiple times (SQLite ignores if column exists)
        let _ = conn.execute(
            "ALTER TABLE platform_status ADD COLUMN remote_status TEXT",
            [],
        );

        let project_root = project_root
            .canonicalize()
            .unwrap_or_else(|_| project_root.to_path_buf());
        Ok(Self {
            conn,
            path,
            project_root,
        })
    }

    /// Open a status tracker for testing purposes.
    #[cfg(test)]
    pub fn open_for_test(path: PathBuf, project_root: &Path) -> Result<Self> {
        Self::open_at(path, project_root)
    }

    /// Get the project root for path normalization.
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    /// Normalize a local path to relative format for storage.
    ///
    /// Per [[RFC-0005:C-PROJECT-ROOT]], paths stored in the database MUST be:
    /// - Relative to project root
    /// - Using forward slashes (OS-agnostic)
    /// - Without `./` prefix or `..` components
    pub fn normalize_path(&self, local_path: &Path) -> Result<String> {
        normalize_to_relative(local_path, &self.project_root)
    }

    /// Save status to disk
    pub fn save(&self) -> Result<()> {
        // SQLite writes happen transactionally in mutating methods.
        // Keep this method for API compatibility.
        let _ = &self.path;
        Ok(())
    }

    // =========================================================================
    // Platform Status Methods
    // =========================================================================

    fn load_platform_status(&self, slug: &str, platform: &str) -> Result<Option<PlatformStatus>> {
        self.conn
            .query_row(
                r#"
SELECT published, url, platform_id, published_at, content_hash, remote_status
FROM platform_status
WHERE slug = ?1 AND platform = ?2
"#,
                params![slug, platform],
                |row| {
                    let published: i64 = row.get(0)?;
                    let url: Option<String> = row.get(1)?;
                    let platform_id: Option<String> = row.get(2)?;
                    let published_at_str: Option<String> = row.get(3)?;
                    let content_hash: Option<String> = row.get(4)?;
                    let remote_status: Option<String> = row.get(5)?;

                    let last_publish = match published_at_str {
                        Some(ts) => {
                            let parsed = chrono::DateTime::parse_from_rfc3339(&ts)
                                .map_err(|e| {
                                    rusqlite::Error::FromSqlConversionFailure(
                                        3,
                                        rusqlite::types::Type::Text,
                                        Box::new(e),
                                    )
                                })?
                                .with_timezone(&Utc);
                            Some(PublishResult {
                                url,
                                platform_id,
                                published_at: parsed,
                            })
                        }
                        None => None,
                    };

                    Ok(PlatformStatus {
                        published: published != 0,
                        last_publish,
                        content_hash,
                        remote_status,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    /// Get previously published URL for a post+platform.
    pub fn get_published_url(&self, slug: &str, platform: &str) -> Result<Option<String>> {
        self.load_platform_status(slug, platform).map(|o| {
            o.and_then(|s| {
                if s.published {
                    s.last_publish.and_then(|p| p.url)
                } else {
                    None
                }
            })
        })
    }

    /// Get the first published URL for a post across all platforms.
    ///
    /// Returns the platform with the lowest alphabetical ID that has a published URL.
    /// Used by copypaste adapters for auto-selecting internal link targets.
    pub fn get_first_published_url(&self, slug: &str) -> Result<Option<(String, String)>> {
        self.conn
            .query_row(
                r#"
SELECT platform, url
FROM platform_status
WHERE slug = ?1 AND published = 1 AND url IS NOT NULL
ORDER BY platform ASC
LIMIT 1
"#,
                params![slug],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(Into::into)
    }

    /// Get previously stored platform-specific ID for a post+platform.
    pub fn get_platform_id(&self, slug: &str, platform: &str) -> Result<Option<String>> {
        self.conn
            .query_row(
                r#"
SELECT platform_id
FROM platform_status
WHERE slug = ?1 AND platform = ?2 AND published = 1
"#,
                params![slug, platform],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map(|o| o.flatten())
            .map_err(Into::into)
    }

    /// Get platform status for lifecycle determination.
    /// Per [[RFC-0005:C-LIFECYCLE-TRANSITIONS]], used by pipeline to determine remote_status.
    pub fn load_platform_status_internal(
        &self,
        slug: &str,
        platform: &str,
    ) -> Result<Option<PlatformStatus>> {
        self.load_platform_status(slug, platform)
    }

    /// Get status for a post across all platforms.
    /// Returns (published, last_url) for each platform.
    ///
    /// This queries the database directly for all platforms that have status
    /// records for this slug, rather than being limited to platforms defined
    /// in the content's meta.toml.
    pub fn get_status(&self, content: &Content) -> HashMap<String, (bool, Option<String>)> {
        let slug = content.slug();
        let mut result = HashMap::new();

        // Query all platforms from the database for this slug
        let db_platforms = self.load_all_platform_statuses(slug).unwrap_or_default();
        for (platform, status) in db_platforms {
            let published = status.published;
            let url = status.last_publish.and_then(|p| p.url);
            result.insert(platform, (published, url));
        }

        // Also include platforms from meta.toml that may not have status yet
        for platform in content.meta.platforms.keys() {
            result.entry(platform.clone()).or_insert((false, None));
        }

        result
    }

    /// Load all platform statuses for a slug from the database.
    fn load_all_platform_statuses(&self, slug: &str) -> Result<Vec<(String, PlatformStatus)>> {
        let mut stmt = self.conn.prepare(
            r#"
SELECT platform, published, url, platform_id, published_at, content_hash, remote_status
FROM platform_status
WHERE slug = ?1
"#,
        )?;

        let rows = stmt.query_map(params![slug], |row| {
            let platform: String = row.get(0)?;
            let published: i64 = row.get(1)?;
            let url: Option<String> = row.get(2)?;
            let platform_id: Option<String> = row.get(3)?;
            let published_at_str: Option<String> = row.get(4)?;
            let content_hash: Option<String> = row.get(5)?;
            let remote_status: Option<String> = row.get(6)?;

            let last_publish = match published_at_str {
                Some(ts) => {
                    let parsed = chrono::DateTime::parse_from_rfc3339(&ts).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            3,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?;
                    Some(PublishResult {
                        url,
                        platform_id,
                        published_at: parsed.with_timezone(&chrono::Utc),
                    })
                }
                None => None,
            };

            Ok((
                platform,
                PlatformStatus {
                    published: published != 0,
                    last_publish,
                    content_hash,
                    remote_status,
                },
            ))
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to load platform statuses")
    }

    /// Mark a post as published to a platform.
    ///
    /// Per [[RFC-0005:C-STATUS-TRACKING]], stores the remote lifecycle state.
    ///
    /// # Arguments
    /// * `remote_status` - "draft" or "published" for API-based platforms,
    ///   or `None` for local output platforms.
    pub fn mark_published(
        &mut self,
        content: &Content,
        platform: &str,
        result: &PublishResult,
        remote_status: Option<&str>,
    ) -> Result<()> {
        let slug = content.slug().to_string();
        let content_hash = self.compute_hash(content)?;
        let tx = self
            .conn
            .transaction()
            .context("Failed to start status transaction")?;
        tx.execute(
            "INSERT OR IGNORE INTO posts (slug) VALUES (?1)",
            params![slug.as_str()],
        )?;
        tx.execute(
            r#"
INSERT INTO platform_status (slug, platform, published, url, platform_id, published_at, content_hash, remote_status)
VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6, ?7)
ON CONFLICT(slug, platform) DO UPDATE SET
    published = 1,
    url = excluded.url,
    platform_id = excluded.platform_id,
    published_at = excluded.published_at,
    content_hash = excluded.content_hash,
    remote_status = excluded.remote_status
"#,
            params![
                slug.as_str(),
                platform,
                result.url.as_deref(),
                result.platform_id.as_deref(),
                result.published_at.to_rfc3339(),
                content_hash,
                remote_status
            ],
        )?;
        tx.commit().context("Failed to commit status transaction")?;

        Ok(())
    }

    /// Record a reconciliation signal when remote publish succeeded
    /// but local status persistence failed.
    pub fn record_reconcile(
        &self,
        slug: &str,
        platform: &str,
        remote_id: Option<&str>,
        remote_url: Option<&str>,
        error: &str,
    ) -> Result<()> {
        self.conn.execute(
            r#"
INSERT INTO publish_reconcile (slug, platform, remote_url, remote_id, error, created_at)
VALUES (?1, ?2, ?3, ?4, ?5, ?6)
"#,
            params![
                slug,
                platform,
                remote_url,
                remote_id,
                error,
                Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    }

    /// Check if content has changed since last publish
    pub fn has_changed(&self, content: &Content, platform: &str) -> Result<bool> {
        let slug = content.slug();

        let current_hash = self.compute_hash(content)?;
        let last_hash: Option<String> = self
            .conn
            .query_row(
                "SELECT content_hash FROM platform_status WHERE slug = ?1 AND platform = ?2",
                params![slug, platform],
                |row| row.get(0),
            )
            .optional()?;

        Ok(last_hash.as_deref() != Some(current_hash.as_str()))
    }

    /// Compute a simple hash of content for change detection
    fn compute_hash(&self, content: &Content) -> Result<String> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let content_str = std::fs::read_to_string(&content.content_file)?;
        let mut tags = content.meta.tags.clone();
        tags.sort();
        tags.dedup();

        let mut hasher = DefaultHasher::new();
        content_str.hash(&mut hasher);
        content.meta.title.hash(&mut hasher);
        tags.hash(&mut hasher);

        Ok(format!("{:x}", hasher.finish()))
    }

    // =========================================================================
    // Asset Upload Tracking per [[RFC-0004:C-UPLOAD-TRACKING]]
    // =========================================================================

    /// Look up cached asset URL by content index key.
    /// Per [[RFC-0004:C-UPLOAD-TRACKING]] - content index lookup.
    pub fn get_asset_by_content(
        &self,
        storage_config_id: &str,
        content_hash: &str,
        extension: &str,
    ) -> Result<Option<AssetUploadRecord>> {
        self.conn
            .query_row(
                r#"
SELECT local_path, storage_config_id, content_hash, extension, remote_key, remote_url, uploaded_at
FROM asset_uploads
WHERE storage_config_id = ?1 AND content_hash = ?2 AND extension = ?3
"#,
                params![storage_config_id, content_hash, extension],
                |row| {
                    Ok(AssetUploadRecord {
                        local_path: row.get(0)?,
                        storage_config_id: row.get(1)?,
                        content_hash: row.get(2)?,
                        extension: row.get(3)?,
                        remote_key: row.get(4)?,
                        remote_url: row.get(5)?,
                        uploaded_at: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    /// Look up cached asset by local path.
    /// Per [[RFC-0004:C-UPLOAD-TRACKING]] - path index lookup.
    pub fn get_asset_by_path(
        &self,
        local_path: &str,
        storage_config_id: &str,
    ) -> Result<Option<AssetUploadRecord>> {
        self.conn
            .query_row(
                r#"
SELECT local_path, storage_config_id, content_hash, extension, remote_key, remote_url, uploaded_at
FROM asset_uploads
WHERE local_path = ?1 AND storage_config_id = ?2
ORDER BY uploaded_at DESC
LIMIT 1
"#,
                params![local_path, storage_config_id],
                |row| {
                    Ok(AssetUploadRecord {
                        local_path: row.get(0)?,
                        storage_config_id: row.get(1)?,
                        content_hash: row.get(2)?,
                        extension: row.get(3)?,
                        remote_key: row.get(4)?,
                        remote_url: row.get(5)?,
                        uploaded_at: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    /// Record a successful asset upload.
    /// Per [[RFC-0004:C-UPLOAD-TRACKING]] - per-asset atomic persistence.
    pub fn record_asset_upload(&self, record: &AssetUploadRecord) -> Result<()> {
        self.conn.execute(
            r#"
INSERT INTO asset_uploads (local_path, storage_config_id, content_hash, extension, remote_key, remote_url, uploaded_at)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
ON CONFLICT(storage_config_id, content_hash, extension) DO UPDATE SET
    local_path = excluded.local_path,
    remote_key = excluded.remote_key,
    remote_url = excluded.remote_url,
    uploaded_at = excluded.uploaded_at
"#,
            params![
                record.local_path,
                record.storage_config_id,
                record.content_hash,
                record.extension,
                record.remote_key,
                record.remote_url,
                record.uploaded_at
            ],
        )?;
        Ok(())
    }

    /// List all asset uploads for assets under a given path prefix.
    /// Per [[RFC-0004:C-UPLOAD-TRACKING]].
    pub fn list_assets_by_prefix(&self, path_prefix: &str) -> Result<Vec<AssetUploadRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
SELECT local_path, storage_config_id, content_hash, extension, remote_key, remote_url, uploaded_at
FROM asset_uploads
WHERE local_path LIKE ?1 || '%'
ORDER BY uploaded_at DESC
"#,
        )?;

        let records = stmt
            .query_map(params![path_prefix], |row| {
                Ok(AssetUploadRecord {
                    local_path: row.get(0)?,
                    storage_config_id: row.get(1)?,
                    content_hash: row.get(2)?,
                    extension: row.get(3)?,
                    remote_key: row.get(4)?,
                    remote_url: row.get(5)?,
                    uploaded_at: row.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(records)
    }
}

fn normalize_to_relative(path: &Path, project_root: &Path) -> Result<String> {
    let normalized = typub_project::normalize_to_relative(path, project_root)?;
    Ok(normalized
        .strip_prefix("./")
        .unwrap_or(&normalized)
        .to_string())
}
