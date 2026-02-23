use crate::adapters;
use crate::content::PostInfo;
use regex::Regex;
use std::collections::HashMap;

/// Field to sort posts by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortField {
    /// Sort by created date
    #[default]
    Created,
    /// Sort by updated date (falls back to created if no updated date)
    Updated,
    /// Sort by title (alphabetical)
    Title,
    /// Sort by publish status (unpublished first)
    Status,
}

impl SortField {
    /// Cycle to the next sort field.
    pub fn next(self) -> Self {
        match self {
            Self::Created => Self::Updated,
            Self::Updated => Self::Title,
            Self::Title => Self::Status,
            Self::Status => Self::Created,
        }
    }

    /// Display name for the sort field.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Updated => "updated",
            Self::Title => "title",
            Self::Status => "status",
        }
    }
}

impl std::str::FromStr for SortField {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "created" => Ok(Self::Created),
            "updated" => Ok(Self::Updated),
            "title" => Ok(Self::Title),
            "status" => Ok(Self::Status),
            _ => Err(format!("unknown sort field: {}", s)),
        }
    }
}

/// Sort order (ascending or descending).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortOrder {
    /// Ascending order (A-Z, oldest first)
    Asc,
    /// Descending order (Z-A, newest first)
    #[default]
    Desc,
}

impl SortOrder {
    /// Toggle between ascending and descending.
    pub fn toggle(self) -> Self {
        match self {
            Self::Asc => Self::Desc,
            Self::Desc => Self::Asc,
        }
    }

    /// Display name for the sort order.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }

    /// Arrow indicator for UI.
    pub fn arrow(&self) -> &'static str {
        match self {
            Self::Asc => "↑",
            Self::Desc => "↓",
        }
    }
}

/// Sort TUI PostInfo items.
pub fn sort_posts(posts: &mut [PostInfo], field: SortField, order: SortOrder) {
    posts.sort_by(|a, b| {
        let cmp = match field {
            SortField::Created => a.created.cmp(&b.created),
            SortField::Updated => a.updated.cmp(&b.updated),
            SortField::Title => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
            SortField::Status => {
                // Sort by unpublished count (more unpublished = earlier)
                let a_unpub = count_unpublished(&a.status);
                let b_unpub = count_unpublished(&b.status);
                a_unpub.cmp(&b_unpub)
            }
        };

        match order {
            SortOrder::Asc => cmp,
            SortOrder::Desc => cmp.reverse(),
        }
    });
}

/// Count unpublished API platforms for a post.
fn count_unpublished(status: &HashMap<String, (bool, Option<String>)>) -> usize {
    status
        .iter()
        .filter(|(platform, (published, _))| {
            !adapters::is_local_output_platform(platform) && !published
        })
        .count()
}

/// Filter predicates for posts.
#[derive(Debug, Default)]
pub struct PostFilter {
    /// Filter to posts configured for this platform.
    pub platform: Option<String>,
    /// Filter by publish status: true = fully published, false = has pending.
    pub published: Option<bool>,
    /// Filter by tag (case-insensitive).
    pub tag: Option<String>,
    /// Filter by title (regex match).
    pub title_regex: Option<Regex>,
}

impl PostFilter {
    /// Check if a post matches all active filters (using PostInfo).
    pub fn matches(&self, info: &PostInfo) -> bool {
        // Platform filter
        if let Some(ref platform) = self.platform
            && !info.status.contains_key(platform)
        {
            return false;
        }

        // Published/pending filter
        if let Some(want_published) = self.published {
            let is_fully_published = info
                .status
                .iter()
                .filter(|(p, _)| !adapters::is_local_output_platform(p))
                .all(|(_, (pub_status, _))| *pub_status);

            if want_published != is_fully_published {
                return false;
            }
        }

        // Tag filter (case-insensitive)
        if let Some(ref tag) = self.tag {
            let tag_lower = tag.to_lowercase();
            let has_tag = info.tags.iter().any(|t| t.to_lowercase() == tag_lower);
            if !has_tag {
                return false;
            }
        }

        // Title regex filter
        if let Some(ref regex) = self.title_regex
            && !regex.is_match(&info.title)
        {
            return false;
        }

        true
    }

    /// Check if any filter is active.
    pub fn is_active(&self) -> bool {
        self.platform.is_some()
            || self.published.is_some()
            || self.tag.is_some()
            || self.title_regex.is_some()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use chrono::NaiveDate;
    use std::path::PathBuf;

    fn make_post_info(
        title: &str,
        created: NaiveDate,
        updated: NaiveDate,
        published_count: usize,
        total_count: usize,
    ) -> PostInfo {
        let mut status = HashMap::new();
        for i in 0..total_count {
            let name = format!("platform{}", i);
            let is_published = i < published_count;
            status.insert(name, (is_published, None));
        }
        PostInfo {
            path: PathBuf::from("/tmp/test"),
            title: title.to_string(),
            slug: title.to_lowercase().replace(' ', "-"),
            created,
            updated,
            tags: vec![],
            status,
        }
    }

    #[test]
    fn test_sort_by_created_desc() {
        let old_date = NaiveDate::from_ymd_opt(2024, 1, 1).expect("valid");
        let new_date = NaiveDate::from_ymd_opt(2024, 6, 1).expect("valid");
        let mut posts = vec![
            make_post_info("Old", old_date, old_date, 0, 1),
            make_post_info("New", new_date, new_date, 0, 1),
        ];

        sort_posts(&mut posts, SortField::Created, SortOrder::Desc);
        assert_eq!(posts[0].title, "New");
        assert_eq!(posts[1].title, "Old");
    }

    #[test]
    fn test_sort_by_title_asc() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 1).expect("valid");
        let mut posts = vec![
            make_post_info("Zebra", date, date, 0, 1),
            make_post_info("Apple", date, date, 0, 1),
        ];

        sort_posts(&mut posts, SortField::Title, SortOrder::Asc);
        assert_eq!(posts[0].title, "Apple");
        assert_eq!(posts[1].title, "Zebra");
    }

    #[test]
    fn test_sort_by_status_unpublished_first() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 1).expect("valid");
        let mut posts = vec![
            make_post_info("Published", date, date, 2, 2),
            make_post_info("Pending", date, date, 0, 2),
        ];

        // Desc order = more unpublished first
        sort_posts(&mut posts, SortField::Status, SortOrder::Desc);
        assert_eq!(posts[0].title, "Pending");
        assert_eq!(posts[1].title, "Published");
    }

    #[test]
    fn test_sort_field_cycle() {
        assert_eq!(SortField::Created.next(), SortField::Updated);
        assert_eq!(SortField::Updated.next(), SortField::Title);
        assert_eq!(SortField::Title.next(), SortField::Status);
        assert_eq!(SortField::Status.next(), SortField::Created);
    }
}
