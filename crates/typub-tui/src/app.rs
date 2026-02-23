use anyhow::Result;
use std::path::Path;
use typub_config::Config;
use typub_core::PostInfo;
use typub_engine::{Renderer, adapters, content, sorting};
use typub_storage::StatusTracker;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    PostList,
    PostDetail,
    Preview,
}

#[derive(Debug, Clone)]
pub enum PublishState {
    Idle,
    InProgress {
        platform: String,
        progress: f64,
    },
    Completed {
        platform: String,
        success: bool,
        message: String,
    },
}

pub struct App<'a> {
    pub config: &'a Config,
    project_root: std::path::PathBuf,
    pub tracker: StatusTracker,
    pub view: View,
    pub posts: Vec<PostInfo>,
    pub selected_index: usize,
    pub preview_scroll: u16,
    pub preview_cache: Option<String>,
    pub publish_state: PublishState,
    pub selected_platform_index: usize,
    pub should_quit: bool,
    pub status_message: Option<String>,
    pub pending_action: Option<PendingAction>,
    pub sort_field: sorting::SortField,
    pub sort_order: sorting::SortOrder,
}

#[derive(Debug, Clone)]
pub enum PendingAction {
    RenderPreview,
    OpenBrowser,
    PublishPlatform { platform: String },
    PublishAll,
}

impl<'a> App<'a> {
    pub fn new(config: &'a Config, project_root: &Path) -> Result<Self> {
        let tracker = StatusTracker::load(project_root)?;
        let posts = Self::load_posts(config, &tracker)?;

        Ok(Self {
            config,
            project_root: project_root.to_path_buf(),
            tracker,
            view: View::PostList,
            posts,
            selected_index: 0,
            preview_scroll: 0,
            preview_cache: None,
            publish_state: PublishState::Idle,
            selected_platform_index: 0,
            should_quit: false,
            status_message: None,
            pending_action: None,
            sort_field: sorting::SortField::default(),
            sort_order: sorting::SortOrder::default(),
        })
    }

    fn load_posts(config: &Config, tracker: &StatusTracker) -> Result<Vec<PostInfo>> {
        let contents = typub_engine::content::discover_all_with_logging(&config.content_dir)?;
        let mut posts: Vec<PostInfo> = contents
            .iter()
            .map(|content| {
                let status = tracker.get_status(content);
                PostInfo::from_content(content, status)
            })
            .collect();

        sorting::sort_posts(
            &mut posts,
            sorting::SortField::Created,
            sorting::SortOrder::Desc,
        );

        Ok(posts)
    }

    pub fn resort_posts(&mut self) {
        sorting::sort_posts(&mut self.posts, self.sort_field, self.sort_order);
        self.selected_index = 0;
    }

    pub fn cycle_sort_field(&mut self) {
        self.sort_field = self.sort_field.next();
        self.resort_posts();
        self.status_message = Some(format!(
            "Sort: {} {}",
            self.sort_field.as_str(),
            self.sort_order.arrow()
        ));
    }

    pub fn toggle_sort_order(&mut self) {
        self.sort_order = self.sort_order.toggle();
        self.resort_posts();
        self.status_message = Some(format!(
            "Sort: {} {}",
            self.sort_field.as_str(),
            self.sort_order.arrow()
        ));
    }

    pub fn selected_post(&self) -> Option<&PostInfo> {
        self.posts.get(self.selected_index)
    }

    pub fn selected_platforms(&self) -> Vec<String> {
        self.selected_post()
            .map(|p| {
                let mut platforms: Vec<_> = p.status.keys().cloned().collect();
                platforms.sort();
                platforms
            })
            .unwrap_or_default()
    }

    pub fn select_up(&mut self) {
        match self.view {
            View::PostList => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            View::PostDetail => {
                if self.selected_platform_index > 0 {
                    self.selected_platform_index -= 1;
                }
            }
            View::Preview => {
                self.preview_scroll = self.preview_scroll.saturating_sub(1);
            }
        }
    }

    pub fn select_down(&mut self) {
        match self.view {
            View::PostList => {
                if self.selected_index < self.posts.len().saturating_sub(1) {
                    self.selected_index += 1;
                }
            }
            View::PostDetail => {
                let platforms = self.selected_platforms();
                if self.selected_platform_index < platforms.len().saturating_sub(1) {
                    self.selected_platform_index += 1;
                }
            }
            View::Preview => {
                self.preview_scroll = self.preview_scroll.saturating_add(1);
            }
        }
    }

    pub fn page_up(&mut self) {
        if self.view == View::Preview {
            self.preview_scroll = self.preview_scroll.saturating_sub(20);
        }
    }

    pub fn page_down(&mut self) {
        if self.view == View::Preview {
            self.preview_scroll = self.preview_scroll.saturating_add(20);
        }
    }

    pub fn enter(&mut self) {
        match self.view {
            View::PostList => {
                if !self.posts.is_empty() {
                    self.view = View::PostDetail;
                    self.selected_platform_index = 0;
                }
            }
            View::PostDetail | View::Preview => {}
        }
    }

    pub fn back(&mut self) {
        match self.view {
            View::PostList => {
                self.should_quit = true;
            }
            View::PostDetail => {
                self.view = View::PostList;
            }
            View::Preview => {
                self.view = View::PostDetail;
            }
        }
    }

    pub fn clear_status(&mut self) {
        self.status_message = None;
    }

    pub fn request_preview(&mut self) {
        if self.view == View::PostDetail {
            self.pending_action = Some(PendingAction::RenderPreview);
        }
    }

    pub fn request_browser(&mut self) {
        if self.view == View::Preview {
            self.pending_action = Some(PendingAction::OpenBrowser);
        }
    }

    pub fn request_publish_platform(&mut self) {
        if self.view == View::PostDetail
            && let Some(_post) = self.selected_post()
        {
            let platforms = self.selected_platforms();
            if let Some(platform) = platforms.get(self.selected_platform_index) {
                self.pending_action = Some(PendingAction::PublishPlatform {
                    platform: platform.clone(),
                });
                self.publish_state = PublishState::InProgress {
                    platform: platform.clone(),
                    progress: 0.0,
                };
            }
        }
    }

    pub fn request_publish_all(&mut self) {
        if self.view == View::PostDetail {
            self.pending_action = Some(PendingAction::PublishAll);
            self.publish_state = PublishState::InProgress {
                platform: "all".to_string(),
                progress: 0.0,
            };
        }
    }

    pub async fn execute_pending(&mut self) -> Result<()> {
        let action = match self.pending_action.take() {
            Some(action) => action,
            None => return Ok(()),
        };

        match action {
            PendingAction::RenderPreview => self.render_preview().await,
            PendingAction::OpenBrowser => self.open_preview_in_browser().await,
            PendingAction::PublishPlatform { platform } => self.publish_platform(&platform).await,
            PendingAction::PublishAll => self.publish_all().await,
        }
    }

    async fn render_preview(&mut self) -> Result<()> {
        let Some(post) = self.selected_post() else {
            return Ok(());
        };

        let content = content::Content::load(&post.path)?;
        let renderer = Renderer::new_with_root(self.config, self.project_root.clone());
        let registry = adapters::AdapterRegistry::new(self.config)?;
        let mut ctx = adapters::PublishContext::new_with_root(self.config, &self.project_root)?;

        let platforms = self.selected_platforms();
        let Some(platform_id) = platforms.first() else {
            return Ok(());
        };
        let adapter = registry.get(platform_id)?;
        let preview_path = typub_engine::pipeline::preview_single_platform(
            adapter,
            platform_id,
            &content,
            &renderer,
            &mut ctx,
            self.config,
            None,
        )
        .await?;

        let html = std::fs::read_to_string(preview_path)?;
        self.preview_cache = Some(html2text::from_read(html.as_bytes(), 80)?);
        self.view = View::Preview;
        Ok(())
    }

    async fn open_preview_in_browser(&mut self) -> Result<()> {
        if let Some(post) = self.selected_post() {
            let content = content::Content::load(&post.path)?;
            let renderer = Renderer::new_with_root(self.config, self.project_root.clone());
            let registry = adapters::AdapterRegistry::new(self.config)?;
            let mut ctx = adapters::PublishContext::new_with_root(self.config, &self.project_root)?;
            let platforms = self.selected_platforms();
            let Some(platform_id) = platforms.first() else {
                return Ok(());
            };
            let adapter = registry.get(platform_id)?;
            let preview_path = typub_engine::pipeline::preview_single_platform(
                adapter,
                platform_id,
                &content,
                &renderer,
                &mut ctx,
                self.config,
                None,
            )
            .await?;
            typub_ui::info(&format!("Preview ready: {}", preview_path.display()));
            open::that(preview_path)?;
        }
        Ok(())
    }

    async fn publish_platform(&mut self, platform_id: &str) -> Result<()> {
        let Some(post) = self.selected_post() else {
            return Ok(());
        };

        let content = content::Content::load(&post.path)?;
        let renderer = Renderer::new_with_root(self.config, self.project_root.clone());
        let registry = adapters::AdapterRegistry::new(self.config)?;
        let mut publish_ctx =
            adapters::PublishContext::new_with_root(self.config, &self.project_root)?;
        let adapter = registry.get(platform_id)?;

        let result = typub_engine::pipeline::publish_single_platform(
            adapter,
            platform_id,
            &content,
            &renderer,
            &mut publish_ctx,
            self.config,
            None,
        )
        .await;

        match result {
            Ok(url) => {
                self.publish_state = PublishState::Completed {
                    platform: platform_id.to_string(),
                    success: true,
                    message: format!("Published: {}", url.url.as_deref().unwrap_or("(no URL)")),
                };
                let _ = self.reload();
            }
            Err(e) => {
                self.publish_state = PublishState::Completed {
                    platform: platform_id.to_string(),
                    success: false,
                    message: format!("{}", e),
                };
            }
        }

        Ok(())
    }

    async fn publish_all(&mut self) -> Result<()> {
        let Some(post) = self.selected_post() else {
            return Ok(());
        };

        let content = content::Content::load(&post.path)?;
        let renderer = Renderer::new_with_root(self.config, self.project_root.clone());
        let registry = adapters::AdapterRegistry::new(self.config)?;
        let mut publish_ctx =
            adapters::PublishContext::new_with_root(self.config, &self.project_root)?;

        let mut success_count = 0;
        let mut fail_count = 0;

        let platforms = self.selected_platforms();
        for (i, platform_id) in platforms.iter().enumerate() {
            self.publish_state = PublishState::InProgress {
                platform: platform_id.clone(),
                progress: i as f64 / platforms.len() as f64,
            };

            let adapter = match registry.get(platform_id) {
                Ok(a) => a,
                Err(_) => {
                    fail_count += 1;
                    continue;
                }
            };

            let result = typub_engine::pipeline::publish_single_platform(
                adapter,
                platform_id,
                &content,
                &renderer,
                &mut publish_ctx,
                self.config,
                None,
            )
            .await;

            if result.is_ok() {
                success_count += 1;
            } else {
                fail_count += 1;
            }
        }

        self.publish_state = PublishState::Completed {
            platform: "all".to_string(),
            success: fail_count == 0,
            message: format!("{} succeeded, {} failed", success_count, fail_count),
        };

        let _ = self.reload();
        Ok(())
    }

    pub fn reload(&mut self) -> Result<()> {
        self.tracker = StatusTracker::load(&self.project_root)?;
        self.posts = Self::load_posts(self.config, &self.tracker)?;

        if self.selected_index >= self.posts.len() {
            self.selected_index = self.posts.len().saturating_sub(1);
        }
        Ok(())
    }
}
