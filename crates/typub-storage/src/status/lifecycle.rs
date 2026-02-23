//! Lifecycle management for publish status.
//!
//! Per [[RFC-0005:C-LIFECYCLE-TRANSITIONS]].

use anyhow::Result;
use typub_core::DraftSupport;

/// Validate remote_status for API-based platforms.
/// Per [[RFC-0005:C-LIFECYCLE-TRANSITIONS]] data integrity guard.
///
/// If `platform_id` is present (remote object exists), `remote_status` MUST be
/// one of "draft" or "published". This ensures corrupted or incomplete status
/// data causes immediate, diagnosable failures rather than undefined behavior.
pub fn validate_remote_status(
    slug: &str,
    platform: &str,
    platform_id: Option<&str>,
    remote_status: Option<&str>,
) -> Result<()> {
    if platform_id.is_some() {
        match remote_status {
            Some("draft") | Some("published") => Ok(()),
            Some(invalid) => anyhow::bail!(
                "Invalid remote_status '{}' for {}/{}: expected 'draft' or 'published'",
                invalid,
                slug,
                platform
            ),
            None => anyhow::bail!(
                "Missing remote_status for {}/{} with existing platform_id",
                slug,
                platform
            ),
        }
    } else {
        Ok(())
    }
}

/// Lifecycle action to take per [[RFC-0005:C-LIFECYCLE-TRANSITIONS]].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleAction {
    /// Create new published content
    CreatePublished,
    /// Create new draft content
    CreateDraft,
    /// Update existing published content
    UpdatePublished,
    /// Update existing draft content
    UpdateDraft,
    /// Transition from draft to published (draft-to-publish)
    TransitionDraftToPublished,
    /// Transition from published to draft (publish-to-draft, if reversible)
    TransitionPublishedToDraft,
    /// Cannot unpublish - warn and update content only
    WarnCannotUnpublish,
}

/// Determine lifecycle action per [[RFC-0005:C-LIFECYCLE-TRANSITIONS]].
///
/// This implements the decision table for API-based platforms.
/// Local output platforms should not use this function.
///
/// # Arguments
/// * `has_remote_object` - Whether `platform_id` is present in the status row
/// * `remote_status` - The stored lifecycle state ("draft" or "published")
/// * `desired_published` - The resolved `published` configuration value
/// * `draft_support` - The platform's declared `DraftSupport` capability
pub fn determine_lifecycle_action(
    has_remote_object: bool,
    remote_status: Option<&str>,
    desired_published: bool,
    draft_support: DraftSupport,
) -> LifecycleAction {
    match (
        has_remote_object,
        remote_status,
        desired_published,
        draft_support,
    ) {
        // No remote object exists - create new
        (false, _, true, _) => LifecycleAction::CreatePublished,
        (false, _, false, DraftSupport::StatusField { .. } | DraftSupport::SeparateObjects) => {
            LifecycleAction::CreateDraft
        }
        (false, _, false, DraftSupport::None) => LifecycleAction::CreatePublished, // Ignore config

        // Remote object exists as published
        (true, Some("published"), true, _) => LifecycleAction::UpdatePublished,
        (true, Some("published"), false, DraftSupport::StatusField { reversible: true }) => {
            LifecycleAction::TransitionPublishedToDraft
        }
        (true, Some("published"), false, DraftSupport::StatusField { reversible: false }) => {
            LifecycleAction::WarnCannotUnpublish
        }
        (true, Some("published"), false, DraftSupport::SeparateObjects) => {
            LifecycleAction::WarnCannotUnpublish
        }
        (true, Some("published"), false, DraftSupport::None) => LifecycleAction::UpdatePublished, // Ignore config

        // Remote object exists as draft
        (true, Some("draft"), true, DraftSupport::StatusField { .. }) => {
            LifecycleAction::TransitionDraftToPublished
        }
        (true, Some("draft"), true, DraftSupport::SeparateObjects) => {
            LifecycleAction::TransitionDraftToPublished
        }
        (true, Some("draft"), true, DraftSupport::None) => {
            // N/A - DraftSupport::None never creates draft
            // Treat as CreatePublished since we somehow have a draft
            LifecycleAction::CreatePublished
        }
        (
            true,
            Some("draft"),
            false,
            DraftSupport::StatusField { .. } | DraftSupport::SeparateObjects,
        ) => LifecycleAction::UpdateDraft,
        (true, Some("draft"), false, DraftSupport::None) => {
            // N/A - DraftSupport::None never creates draft
            LifecycleAction::CreatePublished
        }

        // Invalid/unknown remote_status - should have been caught by validate_remote_status
        // Default to CreatePublished as a safe fallback
        _ => LifecycleAction::CreatePublished,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn test_validate_remote_status_ok() {
        assert!(validate_remote_status("slug", "platform", Some("id"), Some("draft")).is_ok());
        assert!(validate_remote_status("slug", "platform", Some("id"), Some("published")).is_ok());
        assert!(validate_remote_status("slug", "platform", None, None).is_ok());
        assert!(validate_remote_status("slug", "platform", None, Some("garbage")).is_ok());
    }

    #[test]
    fn test_validate_remote_status_invalid() {
        let result = validate_remote_status("slug", "platform", Some("id"), Some("garbage"));
        assert!(result.is_err());
        assert!(
            result
                .expect_err("already verified is_err()")
                .to_string()
                .contains("Invalid remote_status")
        );
    }

    #[test]
    fn test_validate_remote_status_missing() {
        let result = validate_remote_status("slug", "platform", Some("id"), None);
        assert!(result.is_err());
        assert!(
            result
                .expect_err("already verified is_err()")
                .to_string()
                .contains("Missing remote_status")
        );
    }

    #[test]
    fn test_lifecycle_create_published() {
        assert_eq!(
            determine_lifecycle_action(false, None, true, DraftSupport::None),
            LifecycleAction::CreatePublished
        );
        assert_eq!(
            determine_lifecycle_action(
                false,
                None,
                true,
                DraftSupport::StatusField { reversible: true }
            ),
            LifecycleAction::CreatePublished
        );
    }

    #[test]
    fn test_lifecycle_create_draft() {
        assert_eq!(
            determine_lifecycle_action(
                false,
                None,
                false,
                DraftSupport::StatusField { reversible: true }
            ),
            LifecycleAction::CreateDraft
        );
        assert_eq!(
            determine_lifecycle_action(false, None, false, DraftSupport::SeparateObjects),
            LifecycleAction::CreateDraft
        );
    }

    #[test]
    fn test_lifecycle_create_published_when_no_draft_support() {
        // Even if desired_published=false, platforms with DraftSupport::None
        // should create published content
        assert_eq!(
            determine_lifecycle_action(false, None, false, DraftSupport::None),
            LifecycleAction::CreatePublished
        );
    }

    #[test]
    fn test_lifecycle_update_published() {
        assert_eq!(
            determine_lifecycle_action(true, Some("published"), true, DraftSupport::None),
            LifecycleAction::UpdatePublished
        );
        assert_eq!(
            determine_lifecycle_action(
                true,
                Some("published"),
                true,
                DraftSupport::StatusField { reversible: true }
            ),
            LifecycleAction::UpdatePublished
        );
    }

    #[test]
    fn test_lifecycle_transition_draft_to_published() {
        assert_eq!(
            determine_lifecycle_action(
                true,
                Some("draft"),
                true,
                DraftSupport::StatusField { reversible: true }
            ),
            LifecycleAction::TransitionDraftToPublished
        );
        assert_eq!(
            determine_lifecycle_action(true, Some("draft"), true, DraftSupport::SeparateObjects),
            LifecycleAction::TransitionDraftToPublished
        );
    }

    #[test]
    fn test_lifecycle_transition_published_to_draft() {
        assert_eq!(
            determine_lifecycle_action(
                true,
                Some("published"),
                false,
                DraftSupport::StatusField { reversible: true }
            ),
            LifecycleAction::TransitionPublishedToDraft
        );
    }

    #[test]
    fn test_lifecycle_warn_cannot_unpublish() {
        assert_eq!(
            determine_lifecycle_action(
                true,
                Some("published"),
                false,
                DraftSupport::StatusField { reversible: false }
            ),
            LifecycleAction::WarnCannotUnpublish
        );
        assert_eq!(
            determine_lifecycle_action(
                true,
                Some("published"),
                false,
                DraftSupport::SeparateObjects
            ),
            LifecycleAction::WarnCannotUnpublish
        );
    }

    #[test]
    fn test_lifecycle_update_draft() {
        assert_eq!(
            determine_lifecycle_action(
                true,
                Some("draft"),
                false,
                DraftSupport::StatusField { reversible: true }
            ),
            LifecycleAction::UpdateDraft
        );
        assert_eq!(
            determine_lifecycle_action(true, Some("draft"), false, DraftSupport::SeparateObjects),
            LifecycleAction::UpdateDraft
        );
    }
}
