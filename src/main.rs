use anyhow::Result;
use clap::{Parser, Subcommand};
use notify::Watcher;
use std::path::{Path, PathBuf};

use anyhow::Context;
use typub::dev_server;
use typub_config::{Config, ConfigLoadResult};
use typub_core::CapabilityGapBehavior;
use typub_engine::{Renderer, adapters, content, pipeline, project, sorting};
use typub_storage::StatusTracker;
use typub_ui as ui;

#[derive(Parser)]
#[command(name = "typub")]
#[command(about = "Multi-platform content publishing from Typst sources")]
#[command(version)]
#[command(after_help = "\
COMMON WORKFLOWS:
  typub new \"My Post\"           Create a new post
  typub watch posts/my-post     Watch for changes and auto-rebuild
  typub preview posts/my-post   Open preview in browser
  typub publish posts/my-post   Publish to all configured platforms
  typub ls -u                   Show posts with pending platforms
  typub tui                     Interactive dashboard

CONFIGURATION:
  typub.toml                    Project configuration (platforms, themes, etc.)
  posts/*/meta.toml             Per-post metadata and platform overrides

For more information, see: https://github.com/lucifer1004/typub")]
struct Cli {
    /// Path to config file
    #[arg(short, long, value_name = "PATH")]
    config: Option<PathBuf>,

    /// Verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new Contents project
    Init {
        /// Project directory (defaults to current directory)
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// List all posts in a compact table view
    #[command(visible_alias = "ls")]
    #[command(long_about = "\
List all posts in a compact table view.

Posts are sorted by created date (newest first) by default.
Use -s/--sort to change the sort field, --asc to reverse order.
Filter options can be combined to narrow results.")]
    #[command(after_help = "\
EXAMPLES:
  typub list                    All posts, newest first
  typub ls -s title --asc       Sort by title A-Z
  typub ls -p ghost -u          Pending posts for Ghost
  typub ls -t rust -n 5         First 5 posts tagged 'rust'
  typub ls -T '(?i)intro'       Posts with 'intro' in title (case-insensitive)")]
    List {
        /// Sort by field: created, updated, title, status
        #[arg(short = 's', long, default_value = "created", value_name = "FIELD")]
        sort: String,

        /// Sort in ascending order (default is descending for dates)
        #[arg(long)]
        asc: bool,

        /// Filter to posts configured for this platform
        #[arg(short = 'p', long, value_name = "NAME")]
        platform: Option<String>,

        /// Filter to fully published posts only
        #[arg(short = 'P', long)]
        published: bool,

        /// Filter to posts with unpublished platforms
        #[arg(short = 'u', long)]
        pending: bool,

        /// Filter by tag (case-insensitive)
        #[arg(short = 't', long, value_name = "TAG")]
        tag: Option<String>,

        /// Filter by title (regex match)
        #[arg(short = 'T', long, value_name = "REGEX")]
        title: Option<String>,

        /// Limit output to first N posts
        #[arg(short = 'n', long, value_name = "COUNT")]
        limit: Option<usize>,
    },

    /// Development mode: serve with live reload
    #[command(visible_alias = "d")]
    Dev {
        /// Path to the post directory
        path: PathBuf,

        /// Target platform (required for dev mode)
        #[arg(short, long, value_name = "NAME")]
        platform: String,

        /// Port for dev server (default: random available)
        #[arg(long, default_value = "0")]
        port: u16,

        /// Dump intermediate output after specified stage (for debugging)
        /// Valid values: 1-10, or resolve/render/parse/transform/specialize/provision/materialize/serialize/publish/persist
        #[arg(short = 'D', long, value_name = "STAGE")]
        debug_stage: Option<String>,
    },

    /// Publish a post to platform(s)
    #[command(visible_alias = "pub")]
    #[command(long_about = "\
Publish a post to one or more platforms.

By default, publishes to all platforms configured for the post.
Use -p/--platform to target a specific platform.
Use -d/--dry-run to preview what would happen without publishing.")]
    #[command(after_help = "\
EXAMPLES:
  typub publish posts/my-post           Publish to all platforms
  typub pub posts/my-post -p ghost      Publish to Ghost only
  typub pub posts/my-post -d            Dry run (preview changes)
  typub pub posts/my-post -f            Force republish unchanged content")]
    Publish {
        /// Path to the post directory
        path: PathBuf,

        /// Target platform (publishes to all enabled if not specified)
        #[arg(short, long, value_name = "NAME")]
        platform: Option<String>,

        /// Dry run - show what would happen without publishing
        #[arg(short = 'd', long)]
        dry_run: bool,

        /// Force publish even if content hasn't changed
        #[arg(short, long)]
        force: bool,

        /// Dump intermediate output after specified stage (for debugging)
        /// Valid values: 1-10, or resolve/render/parse/transform/specialize/provision/materialize/serialize/publish/persist
        #[arg(short = 'D', long, value_name = "STAGE")]
        debug_stage: Option<String>,
    },

    /// Show detailed publish status for a single post
    Status {
        /// Path to the post directory
        path: PathBuf,

        /// Show cached asset uploads for this post
        #[arg(long)]
        assets: bool,
    },

    /// Create a new post
    New {
        /// Title of the new post
        title: String,
    },

    Tui,
}

fn main() {
    if let Err(e) = run() {
        ui::error(&format!("{:#}", e));
        std::process::exit(1);
    }
}

#[tokio::main]
async fn run() -> Result<()> {
    // Load environment variables
    let _ = dotenvy::dotenv();

    let cli = Cli::parse();

    // Initialize UI with verbose setting
    ui::init(cli.verbose);

    // Handle init command separately (doesn't need config)
    if let Commands::Init { path } = &cli.command {
        return cmd_init(path).await;
    }

    let (config_path, project_root) = resolve_config_path(cli.config.as_deref())?;

    // Load configuration
    let config = match Config::load(&config_path)? {
        ConfigLoadResult::Loaded(config) => config,
        ConfigLoadResult::DefaultsUsed(_, path) => {
            anyhow::bail!(
                "Config file not found: {}. Run 'typub init' or pass --config <path>.",
                path
            );
        }
    };

    match cli.command {
        Commands::Init { .. } => unreachable!(),
        Commands::List {
            sort,
            asc,
            platform,
            published,
            pending,
            tag,
            title,
            limit,
        } => {
            cmd_list(
                &config,
                &project_root,
                &sort,
                asc,
                platform,
                published,
                pending,
                tag,
                title,
                limit,
            )
            .await?;
        }
        Commands::Dev {
            path,
            platform,
            port,
            debug_stage,
        } => {
            let debug_stage = parse_debug_stage(debug_stage.as_deref());
            cmd_dev(&config, &project_root, &path, &platform, port, debug_stage).await?;
        }
        Commands::Publish {
            path,
            platform,
            dry_run,
            force,
            debug_stage,
        } => {
            let debug_stage = parse_debug_stage(debug_stage.as_deref());
            cmd_publish(
                &config,
                &project_root,
                &path,
                platform.as_deref(),
                dry_run,
                force,
                debug_stage,
            )
            .await?;
        }
        Commands::Status { path, assets } => {
            cmd_status(&config, &project_root, &path, assets).await?;
        }
        Commands::New { title } => {
            cmd_new(&config, &title).await?;
        }
        Commands::Tui => {
            typub_tui::run_with_root(&config, &project_root).await?;
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn cmd_list(
    config: &Config,
    project_root: &Path,
    sort_arg: &str,
    asc: bool,
    platform_filter: Option<String>,
    published_only: bool,
    pending_only: bool,
    tag_filter: Option<String>,
    title_filter: Option<String>,
    limit: Option<usize>,
) -> Result<()> {
    use comfy_table::{Attribute, Cell, Color, Table, presets::UTF8_FULL_CONDENSED};
    use std::collections::BTreeSet;

    let posts: Vec<content::Content> = content::discover_all_with_logging(&config.content_dir)?;
    let tracker = StatusTracker::load(project_root)?;

    if posts.is_empty() {
        ui::info("No posts found");
        return Ok(());
    }

    // Convert to PostInfo for sorting/filtering
    type PlatformStatus = std::collections::HashMap<String, (bool, Option<String>)>;
    let mut post_infos: Vec<(content::PostInfo, PlatformStatus)> = posts
        .iter()
        .map(|post| {
            let status: PlatformStatus = tracker.get_status(post);
            let info = content::PostInfo::from_content(post, status.clone());
            (info, status)
        })
        .collect();

    // Parse sort field
    let sort_field = match sort_arg.to_lowercase().as_str() {
        "created" => sorting::SortField::Created,
        "updated" => sorting::SortField::Updated,
        "title" => sorting::SortField::Title,
        "status" => sorting::SortField::Status,
        other => {
            anyhow::bail!(
                "Unknown sort field '{}'. Valid options: created, updated, title, status",
                other
            );
        }
    };

    // Default to Desc for dates, Asc for title
    let sort_order = if asc {
        sorting::SortOrder::Asc
    } else {
        sorting::SortOrder::Desc
    };

    // Sort by extracting infos, sorting, then putting back
    let mut infos: Vec<content::PostInfo> = post_infos.iter().map(|(i, _)| i.clone()).collect();
    sorting::sort_posts(&mut infos, sort_field, sort_order);

    // Rebuild with sorted order
    let info_map: std::collections::HashMap<
        std::path::PathBuf,
        (content::PostInfo, PlatformStatus),
    > = post_infos
        .into_iter()
        .map(|(info, status)| (info.path.clone(), (info, status)))
        .collect();
    post_infos = infos
        .into_iter()
        .filter_map(|info| {
            info_map
                .get(&info.path)
                .map(|(i, s)| (i.clone(), s.clone()))
        })
        .collect();

    // Build filter
    let title_regex: Option<regex::Regex> = title_filter
        .as_ref()
        .map(|pat| regex::Regex::new(pat).with_context(|| format!("Invalid title regex: {}", pat)))
        .transpose()?;

    let filter = sorting::PostFilter {
        platform: platform_filter,
        published: if published_only {
            Some(true)
        } else if pending_only {
            Some(false)
        } else {
            None
        },
        tag: tag_filter,
        title_regex,
    };

    // Apply filter
    if filter.is_active() {
        post_infos.retain(|(info, _)| filter.matches(info));
    }

    // Apply limit
    if let Some(n) = limit {
        post_infos.truncate(n);
    }

    if post_infos.is_empty() {
        ui::info("No posts match the filter");
        return Ok(());
    }

    // Collect all unique platforms across filtered posts
    let mut all_platforms: BTreeSet<String> = BTreeSet::new();
    for (_, status) in &post_infos {
        for platform in status.keys() {
            all_platforms.insert(platform.clone());
        }
    }
    let platforms: Vec<String> = all_platforms.into_iter().collect();

    // Determine if we need compact mode based on terminal width
    // Fixed columns: Date (12) + Title (43) + borders/padding (~10) = ~65 chars
    // Each platform column: full name length + 3 (padding/border) or short code (4)
    let terminal_width = crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80) as usize;
    let fixed_width = 65;
    let full_name_width: usize = platforms.iter().map(|p| p.len() + 3).sum();
    let compact_mode = fixed_width + full_name_width > terminal_width;

    // Build platform display names (short codes for compact mode)
    let mut unknown_counter = 0usize;
    let platform_display: Vec<(String, String)> = platforms
        .iter()
        .map(|name| {
            let display = if compact_mode {
                if let Some(code) = adapters::platform_short_code(name) {
                    code.to_string()
                } else {
                    unknown_counter += 1;
                    format!("p{}", unknown_counter)
                }
            } else {
                name.clone()
            };
            (name.clone(), display)
        })
        .collect();

    // Build table
    let mut table = Table::new();
    table.load_preset(UTF8_FULL_CONDENSED);

    // Header row
    let mut header = vec![
        Cell::new("Date").add_attribute(Attribute::Bold),
        Cell::new("Title").add_attribute(Attribute::Bold),
    ];
    for (_, display) in &platform_display {
        header.push(Cell::new(display).add_attribute(Attribute::Bold));
    }
    table.set_header(header);

    // Data rows
    for (info, status) in &post_infos {
        let date = info.created.format("%Y-%m-%d").to_string();
        let title = if info.title.len() > 40 {
            format!("{}...", &info.title[..37])
        } else {
            info.title.clone()
        };

        let mut row = vec![Cell::new(date), Cell::new(title)];
        for (platform, _) in &platform_display {
            let is_local = adapters::is_local_output_platform(platform);
            let cell = match status.get(platform) {
                Some(_) if is_local => Cell::new("—").fg(Color::Blue),
                Some((true, _)) => Cell::new("●").fg(Color::Green),
                Some((false, _)) => Cell::new("○").fg(Color::DarkGrey),
                None => Cell::new("·").fg(Color::DarkGrey),
            };
            row.push(cell);
        }
        table.add_row(row);
    }

    println!("{table}");

    // Print legend in compact mode
    if compact_mode {
        let legend: Vec<String> = platform_display
            .iter()
            .map(|(name, display)| format!("{}={}", display, name))
            .collect();
        println!("\nLegend: {}", legend.join(" "));
    }

    println!(
        "\n{} posts  |  ● published  ○ pending  — local  · not configured",
        post_infos.len()
    );

    Ok(())
}

/// Resolve which platforms to target for a given command.
///
/// Priority: --platform flag > post's meta.toml platforms > typub.toml enabled platforms.
fn resolve_platforms<'a>(
    config: &'a Config,
    content: &'a content::Content,
    explicit: Option<&'a str>,
) -> Vec<&'a str> {
    if let Some(p) = explicit {
        return vec![p];
    }

    // Post-level platforms from meta.toml
    if !content.meta.platforms.is_empty() {
        return content.meta.platforms.keys().map(|s| s.as_str()).collect();
    }

    // Fall back to default platforms in typub.toml
    config
        .default_platforms()
        .into_iter()
        .map(|(id, _)| id)
        .collect()
}

fn metadata_validation_warnings(content: &content::Content, platform_id: &str) -> Vec<String> {
    let Some(capability) = adapters::adapter_capability(platform_id) else {
        return Vec::new();
    };

    let mut warnings = Vec::new();
    if !content.meta.tags.is_empty()
        && let Some(behavior) = capability.tags_gap_behavior()
    {
        let detail = match behavior {
            CapabilityGapBehavior::WarnAndDegrade => "tags in meta.toml will be ignored",
            CapabilityGapBehavior::HardError => "publish will fail due to unsupported tags",
        };
        warnings.push(format!(
            "Platform '{}' does not support tags; {}.",
            platform_id, detail
        ));
    }
    if !content.meta.categories.is_empty()
        && let Some(behavior) = capability.categories_gap_behavior()
    {
        let detail = match behavior {
            CapabilityGapBehavior::WarnAndDegrade => "categories in meta.toml will be ignored",
            CapabilityGapBehavior::HardError => "publish will fail due to unsupported categories",
        };
        warnings.push(format!(
            "Platform '{}' does not support categories; {}.",
            platform_id, detail
        ));
    }
    warnings
}

fn warn_metadata_compatibility(content: &content::Content, platform_id: &str) {
    for warning in metadata_validation_warnings(content, platform_id) {
        ui::warn(&warning);
    }
}

fn parse_debug_stage(s: Option<&str>) -> Option<typub_engine::PipelineStage> {
    s.and_then(|s| match s.parse::<typub_engine::PipelineStage>() {
        Ok(stage) => Some(stage),
        Err(_) => {
            ui::warn(&format!("Unknown debug stage: {}, ignoring", s));
            None
        }
    })
}

async fn cmd_dev(
    config: &Config,
    project_root: &Path,
    path: &Path,
    platform_id: &str,
    port: u16,
    debug_stage: Option<typub_engine::PipelineStage>,
) -> Result<()> {
    let content = content::Content::load(path)?;
    let renderer = Renderer::new_with_root(config, project_root.to_path_buf());
    let registry = adapters::AdapterRegistry::new(config)?;
    let mut ctx = adapters::PublishContext::new_with_root(config, project_root)?;

    warn_metadata_compatibility(&content, platform_id);

    ui::info(&format!(
        "Starting dev server for: {} ({})",
        content.meta.title, platform_id
    ));

    let adapter = registry.get(platform_id)?;

    // Generate initial preview
    let preview_path = pipeline::preview_single_platform(
        adapter,
        platform_id,
        &content,
        &renderer,
        &mut ctx,
        config,
        debug_stage,
    )
    .await?;

    // Start dev server
    let (actual_port, shutdown, notify) =
        dev_server::start_dev_server(&preview_path, &content.path, port)?;

    ui::info(&format!(
        "Dev server running at http://127.0.0.1:{}",
        actual_port
    ));
    ui::info("Press Ctrl+C to stop");

    // Open browser
    open::that(format!("http://127.0.0.1:{}", actual_port))?;

    // Set up file watcher for live reload
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let mut watcher =
        notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res
                && event.kind.is_modify()
            {
                let _ = tx.blocking_send(());
            }
        })?;
    watcher.watch(path, notify::RecursiveMode::Recursive)?;

    // Wait for changes and rebuild
    let debounce = std::time::Duration::from_millis(500);
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                ui::info("Shutting down dev server...");
                shutdown();
                break;
            }
            _ = rx.recv() => {
                // Debounce: wait a bit to coalesce rapid changes
                tokio::time::sleep(debounce).await;
                // Drain any pending events
                while rx.try_recv().is_ok() {}

                ui::info("Change detected, rebuilding...");

                // Rebuild
                match pipeline::preview_single_platform(
                    adapter,
                    platform_id,
                    &content,
                    &renderer,
                    &mut ctx,
                    config,
                    debug_stage,
                )
                .await
                {
                    Ok(new_path) => {
                        ui::item("rebuilt", platform_id);
                        // Notify the dev server to increment version
                        notify();
                        let _ = new_path;
                    }
                    Err(e) => {
                        ui::error(&format!("Rebuild failed: {}", e));
                    }
                }
            }
        }
    }

    ui::success("Dev server stopped");
    Ok(())
}

async fn cmd_publish(
    config: &Config,
    project_root: &Path,
    path: &Path,
    platform: Option<&str>,
    dry_run: bool,
    force: bool,
    debug_stage: Option<typub_engine::PipelineStage>,
) -> Result<()> {
    let content = content::Content::load(path)?;
    let renderer = Renderer::new_with_root(config, project_root.to_path_buf());
    let registry = adapters::AdapterRegistry::new(config)?;

    // Create context: use dry_run mode if -d flag is set
    // In dry-run mode, asset uploads are mocked (copied to temp dir)
    let mut publish_ctx = if dry_run {
        adapters::PublishContext::new_dry_run(config, project_root)?
    } else {
        adapters::PublishContext::new_with_root(config, project_root)?
    };

    let platforms = resolve_platforms(config, &content, platform);

    if platforms.is_empty() {
        anyhow::bail!(
            "No target platforms for publishing.\n\
             Add [platforms.<name>] to meta.toml, or enable platforms in typub.toml,\n\
             or specify one with --platform <name>."
        );
    }

    ui::log_publish_start(&content.meta.title, &platforms);

    let mut published_count = 0;
    for platform_id in &platforms {
        warn_metadata_compatibility(&content, platform_id);
        let adapter = registry.get(platform_id)?;

        // Validate platform configuration before attempting publish
        if let Some(platform_config) = config.get_platform(platform_id) {
            adapter.validate_config(platform_config)?;
        }

        // Check if content has changed (skip if unchanged unless --force)
        if !force && !publish_ctx.status.has_changed(&content, platform_id)? {
            ui::log_skip(platform_id, "no changes");
            continue;
        }

        if dry_run {
            // Run full local pipeline (stages 1-8) but skip publish and persist
            ui::debug(&format!("Dry-run pipeline for {}...", platform_id));
            pipeline::dry_run_single_platform(
                adapter,
                platform_id,
                &content,
                &renderer,
                &mut publish_ctx,
                config,
                debug_stage,
            )
            .await?;
            ui::log_dry_run(platform_id);
            continue;
        }

        ui::debug(&format!("Publishing to {}...", platform_id));
        let result = pipeline::publish_single_platform(
            adapter,
            platform_id,
            &content,
            &renderer,
            &mut publish_ctx,
            config,
            debug_stage,
        )
        .await?;

        ui::log_publish_success(platform_id, result.url.as_deref());
        published_count += 1;
    }

    publish_ctx.status.save()?;

    if published_count > 0 {
        ui::success(&format!(
            "Published to {} platform{}",
            published_count,
            if published_count > 1 { "s" } else { "" }
        ));
    } else if dry_run {
        ui::info("Dry run complete");
    } else {
        ui::info(
            "No platforms published (all skipped — content unchanged). Use --force to republish.",
        );
    }

    Ok(())
}

async fn cmd_status(
    _config: &Config,
    project_root: &Path,
    path: &std::path::Path,
    show_assets: bool,
) -> Result<()> {
    let tracker = StatusTracker::load(project_root)?;
    let content = content::Content::load(path)?;
    let status = tracker.get_status(&content);

    // Display detailed post information
    ui::header(&content.meta.title);
    println!("  Path:    {}", content.path.display());
    println!("  Slug:    {}", content.slug());
    println!("  Created: {}", content.meta.created);
    if let Some(updated) = content.meta.updated {
        println!("  Updated: {}", updated);
    }
    if !content.meta.tags.is_empty() {
        println!("  Tags:    {}", content.meta.tags.join(", "));
    }
    if !content.meta.categories.is_empty() {
        println!("  Categories: {}", content.meta.categories.join(", "));
    }
    println!();

    // Display platform status with URLs
    ui::header("Platform Status");
    for (platform, (published, url)) in &status {
        ui::platform_status(platform, *published, url.as_deref());
    }

    // Display asset uploads if requested
    if show_assets {
        println!();
        ui::header("Asset Uploads");

        // Get relative path prefix for this post
        let path_prefix = tracker.normalize_path(&content.path)?;
        let assets = tracker.list_assets_by_prefix(&path_prefix)?;

        if assets.is_empty() {
            ui::info("No cached asset uploads for this post");
        } else {
            use comfy_table::{Attribute, Cell, Color, Table, presets::UTF8_FULL_CONDENSED};

            let mut table = Table::new();
            table.load_preset(UTF8_FULL_CONDENSED);

            table.set_header(vec![
                Cell::new("Path").add_attribute(Attribute::Bold),
                Cell::new("Hash").add_attribute(Attribute::Bold),
                Cell::new("Uploaded").add_attribute(Attribute::Bold),
                Cell::new("URL").add_attribute(Attribute::Bold),
            ]);

            for asset in &assets {
                // Show short path (relative to post)
                let short_path = asset
                    .local_path
                    .strip_prefix(&path_prefix)
                    .unwrap_or(&asset.local_path)
                    .trim_start_matches('/');

                // Show short hash (first 8 chars)
                let short_hash = if asset.content_hash.len() >= 8 {
                    &asset.content_hash[..8]
                } else {
                    &asset.content_hash
                };

                // Show short URL (just the key)
                let short_url = if asset.remote_url.len() > 40 {
                    format!("...{}", &asset.remote_url[asset.remote_url.len() - 32..])
                } else {
                    asset.remote_url.clone()
                };

                table.add_row(vec![
                    Cell::new(short_path),
                    Cell::new(short_hash).fg(Color::DarkGrey),
                    Cell::new(&asset.uploaded_at),
                    Cell::new(short_url),
                ]);
            }

            println!("{table}");
            println!(
                "\n{} cached asset(s) — won't re-upload on next publish",
                assets.len()
            );
        }
    }

    Ok(())
}

fn resolve_config_path(config_path: Option<&Path>) -> Result<(PathBuf, PathBuf)> {
    if let Some(config_path) = config_path {
        let config_path = if config_path.is_absolute() {
            config_path.to_path_buf()
        } else {
            std::env::current_dir()?.join(config_path)
        };
        let project_root = match config_path.parent() {
            Some(parent) => parent.to_path_buf(),
            None => std::env::current_dir()?,
        };
        if !config_path.exists() {
            anyhow::bail!(
                "Config file not found: {}. Run 'typub init' or pass --config <path>.",
                config_path.display()
            );
        }
        return Ok((config_path, project_root));
    }

    let project_root = project::find_project_root(Path::new(".")).with_context(
        || "Config file not found: typub.toml. Run 'typub init' or pass --config <path>.",
    )?;
    let config_path = project_root.join(project::CONFIG_FILE_NAME);
    Ok((config_path, project_root))
}

async fn cmd_new(config: &Config, title: &str) -> Result<()> {
    let slug = title
        .to_lowercase()
        .replace(' ', "-")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect::<String>();

    let date = chrono::Local::now().format("%Y-%m-%d");
    let dir_name = format!("{}-{}", date, slug);
    let post_dir = config.content_dir.join(&dir_name);

    std::fs::create_dir_all(&post_dir)?;
    std::fs::create_dir_all(post_dir.join("assets"))?;

    // Create content.typ
    let content_typ = format!(
        r#"= {}

// Your content here...
"#,
        title
    );
    std::fs::write(post_dir.join("content.typ"), content_typ)?;

    // Create meta.toml
    let meta_toml = format!(
        r#"title = "{}"
created = {}
tags = []
categories = []

[platforms.astro]
slug = "{}"
"#,
        title, date, slug
    );
    std::fs::write(post_dir.join("meta.toml"), meta_toml)?;

    ui::success(&format!("Created new post: {}", post_dir.display()));
    ui::item("content", "content.typ");
    ui::item("metadata", "meta.toml");
    ui::item("assets", "assets/");
    Ok(())
}

async fn cmd_init(path: &Path) -> Result<()> {
    use std::fs;

    let project_dir = if path == Path::new(".") {
        std::env::current_dir()?
    } else {
        path.to_path_buf()
    };

    ui::header(&format!(
        "Initializing Contents project in {}",
        project_dir.display()
    ));

    // Check if already initialized
    let config_path = project_dir.join("typub.toml");
    if config_path.exists() {
        anyhow::bail!("Project already initialized (typub.toml exists)");
    }

    // Create directory structure
    let dirs = ["posts", "output", "templates"];
    for dir in &dirs {
        let dir_path = project_dir.join(dir);
        fs::create_dir_all(&dir_path)?;
        ui::item("created", &format!("{}/", dir));
    }

    // Create typub.toml from the canonical template (SSOT).
    let config_content = include_str!("../typub.template.toml");
    fs::write(&config_path, config_content)?;
    ui::item("created", "typub.toml");

    // Create .env
    let env_path = project_dir.join(".env");
    if !env_path.exists() {
        let env_content = r#"# API credentials (add as needed)
# See .env.template for all supported variables
# CONFLUENCE_API_KEY=your-token
# CONFLUENCE_EMAIL=your-email@example.com
"#;
        fs::write(&env_path, env_content)?;
        ui::item("created", ".env");
    }

    // Create .gitignore if not exists
    let gitignore_path = project_dir.join(".gitignore");
    if !gitignore_path.exists() {
        let gitignore_content = r#"# Environment and secrets
.env
typub.toml

# Build outputs
target/
output/
.typub/

# OS files
.DS_Store
"#;
        fs::write(&gitignore_path, gitignore_content)?;
        ui::item("created", ".gitignore");
    }

    // Create example post
    let date = chrono::Local::now().format("%Y-%m-%d");
    let example_dir = project_dir
        .join("posts")
        .join(format!("{}-hello-world", date));
    fs::create_dir_all(example_dir.join("assets"))?;

    let example_post_rel = format!("posts/{}-hello-world", date);

    let content_typ = r#"= Hello World

Welcome to typub!

This is your first post. Edit this file to get started.

== Features

- Write in *Typst* or _Markdown_
- Publish to multiple platforms
- Track publish status
"#;
    fs::write(example_dir.join("content.typ"), content_typ)?;

    let meta_toml = format!(
        r#"title = "Hello World"
created = {}
tags = ["getting-started"]
categories = []

[platforms.astro]
slug = "hello-world"
"#,
        date
    );
    fs::write(example_dir.join("meta.toml"), meta_toml)?;
    ui::item("created", &format!("{}/", example_post_rel));

    ui::success("Project initialized!");
    ui::info("Next steps:");
    ui::info("  1. Edit typub.toml to configure platforms");
    ui::info(&format!("  2. Run: contents preview {}", example_post_rel));
    ui::info(&format!("  3. Run: contents publish {}", example_post_rel));

    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::metadata_validation_warnings;
    use typub_engine::content::{Content, ContentFormat, ContentMeta};

    #[test]
    fn test_metadata_warning_for_unsupported_categories() {
        let content = Content {
            path: std::path::PathBuf::from("/tmp/post"),
            meta: ContentMeta {
                title: "Post".to_string(),
                created: chrono::NaiveDate::from_ymd_opt(2026, 2, 10).expect("valid date"),
                updated: None,
                tags: vec![],
                categories: vec!["engineering".to_string()],
                published: None,
                theme: None,
                internal_link_target: None,
                preamble: None,
                platforms: std::collections::HashMap::new(),
            },
            content_file: std::path::PathBuf::from("/tmp/post/content.typ"),
            source_format: ContentFormat::Typst,
            slides_file: None,
            assets: vec![],
        };

        let warnings = metadata_validation_warnings(&content, "notion");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("does not support categories"));
    }

    #[test]
    fn test_no_metadata_warning_for_wordpress() {
        let content = Content {
            path: std::path::PathBuf::from("/tmp/post"),
            meta: ContentMeta {
                title: "Post".to_string(),
                created: chrono::NaiveDate::from_ymd_opt(2026, 2, 10).expect("valid date"),
                updated: None,
                tags: vec!["rust".to_string()],
                categories: vec!["engineering".to_string()],
                published: None,
                theme: None,
                internal_link_target: None,
                preamble: None,
                platforms: std::collections::HashMap::new(),
            },
            content_file: std::path::PathBuf::from("/tmp/post/content.typ"),
            source_format: ContentFormat::Typst,
            slides_file: None,
            assets: vec![],
        };

        let warnings = metadata_validation_warnings(&content, "wordpress");
        assert!(warnings.is_empty());
    }
}
