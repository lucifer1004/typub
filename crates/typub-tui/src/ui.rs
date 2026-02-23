use super::app::{App, PublishState, View};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};
use typub_engine::adapters::is_local_output_platform;

pub fn draw(frame: &mut Frame, app: &App<'_>) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(frame.area());

    draw_header(frame, app, chunks[0]);
    draw_main(frame, app, chunks[1]);
    draw_footer(frame, app, chunks[2]);
    draw_overlays(frame, app);
}

fn draw_header(frame: &mut Frame, app: &App<'_>, area: Rect) {
    let title = match app.view {
        View::PostList => "typub TUI - Posts",
        View::PostDetail => "typub TUI - Post Detail",
        View::Preview => "typub TUI - Preview",
    };

    let header = Paragraph::new(title)
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::BOTTOM));

    frame.render_widget(header, area);
}

fn draw_main(frame: &mut Frame, app: &App<'_>, area: Rect) {
    match app.view {
        View::PostList => draw_post_list(frame, app, area),
        View::PostDetail => draw_post_detail(frame, app, area),
        View::Preview => draw_preview(frame, app, area),
    }
}

fn draw_post_list(frame: &mut Frame, app: &App<'_>, area: Rect) {
    let items: Vec<ListItem> = app
        .posts
        .iter()
        .enumerate()
        .map(|(i, post)| {
            let mut api_published = 0;
            let mut api_total = 0;
            let mut local_count = 0;

            for (platform, (published, _)) in &post.status {
                if is_local_output_platform(platform) {
                    local_count += 1;
                } else {
                    api_total += 1;
                    if *published {
                        api_published += 1;
                    }
                }
            }

            let status_indicator = if api_total == 0 {
                Span::styled("—", Style::default().fg(Color::Blue))
            } else if api_published == api_total {
                Span::styled("●", Style::default().fg(Color::Green))
            } else if api_published > 0 {
                Span::styled("◐", Style::default().fg(Color::Yellow))
            } else {
                Span::styled("○", Style::default().fg(Color::DarkGray))
            };

            let date = post.created.format("%Y-%m-%d").to_string();
            let title = if post.title.len() > 50 {
                format!("{}...", &post.title[..47])
            } else {
                post.title.clone()
            };

            let style = if i == app.selected_index {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let count_str = if local_count > 0 {
                format!(" ({}/{} +{}L)", api_published, api_total, local_count)
            } else {
                format!(" ({}/{})", api_published, api_total)
            };

            let line = Line::from(vec![
                status_indicator,
                Span::raw(" "),
                Span::styled(date, Style::default().fg(Color::Cyan)),
                Span::raw("  "),
                Span::raw(title),
                Span::raw(count_str),
            ]);

            ListItem::new(line).style(style)
        })
        .collect();

    let sort_indicator = format!("{} {}", app.sort_field.as_str(), app.sort_order.arrow());
    let title = format!(" {} posts | {} ", app.posts.len(), sort_indicator);

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    frame.render_widget(list, area);
}

fn draw_post_detail(frame: &mut Frame, app: &App<'_>, area: Rect) {
    let Some(post) = app.selected_post() else {
        let empty = Paragraph::new("No post selected")
            .block(Block::default().borders(Borders::ALL).title(" Detail "));
        frame.render_widget(empty, area);
        return;
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(0)])
        .split(area);

    let info_lines = vec![
        Line::from(vec![
            Span::styled("Title: ", Style::default().fg(Color::Cyan)),
            Span::raw(&post.title),
        ]),
        Line::from(vec![
            Span::styled("Slug:  ", Style::default().fg(Color::Cyan)),
            Span::raw(&post.slug),
        ]),
        Line::from(vec![
            Span::styled("Date:  ", Style::default().fg(Color::Cyan)),
            Span::raw(post.created.format("%Y-%m-%d").to_string()),
        ]),
        Line::from(vec![
            Span::styled("Tags:  ", Style::default().fg(Color::Cyan)),
            Span::raw(post.tags.join(", ")),
        ]),
    ];

    let info = Paragraph::new(info_lines)
        .block(Block::default().borders(Borders::ALL).title(" Post Info "));
    frame.render_widget(info, chunks[0]);

    let platforms = app.selected_platforms();
    let items: Vec<ListItem> = platforms
        .iter()
        .enumerate()
        .map(|(i, platform)| {
            let (published, url) = post.status.get(platform).cloned().unwrap_or((false, None));
            let is_local = is_local_output_platform(platform);

            let status = if is_local {
                Span::styled("—", Style::default().fg(Color::Blue))
            } else if published {
                Span::styled("✓", Style::default().fg(Color::Green))
            } else {
                Span::styled("✗", Style::default().fg(Color::Red))
            };

            let url_span = if is_local {
                Span::styled(" (local output)", Style::default().fg(Color::Blue))
            } else if let Some(url) = url {
                Span::styled(format!(" {url}"), Style::default().fg(Color::Cyan))
            } else {
                Span::raw("")
            };

            let style = if i == app.selected_platform_index {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };

            let line = Line::from(vec![status, Span::raw(" "), Span::raw(platform), url_span]);

            ListItem::new(line).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Platforms "))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    frame.render_widget(list, chunks[1]);
}

fn draw_preview(frame: &mut Frame, app: &App<'_>, area: Rect) {
    let text = app.preview_cache.clone().unwrap_or_default();
    let paragraph = Paragraph::new(Text::from(text))
        .block(Block::default().borders(Borders::ALL).title(" Preview "))
        .wrap(Wrap { trim: false })
        .scroll((app.preview_scroll, 0));
    frame.render_widget(paragraph, area);
}

fn draw_footer(frame: &mut Frame, app: &App<'_>, area: Rect) {
    let help = match app.view {
        View::PostList => "↑/↓: Move  Enter: Details  s/S: Sort  q: Quit",
        View::PostDetail => {
            "↑/↓: Select  Enter: No-op  p: Preview  P: Publish  A: Publish all  q: Back"
        }
        View::Preview => "↑/↓: Scroll  o: Open  q: Back",
    };
    let footer = Paragraph::new(help)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::TOP));
    frame.render_widget(footer, area);
}

fn draw_overlays(frame: &mut Frame, app: &App<'_>) {
    if let PublishState::InProgress { platform, progress } = &app.publish_state {
        let area = centered_rect(60, 20, frame.area());
        frame.render_widget(Clear, area);

        let text = format!("Publishing {}... {:.0}%", platform, progress * 100.0);
        let block = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title(" Publishing "))
            .style(Style::default().fg(Color::Yellow));
        frame.render_widget(block, area);
    }

    if let PublishState::Completed {
        success, message, ..
    } = &app.publish_state
    {
        let area = centered_rect(60, 20, frame.area());
        frame.render_widget(Clear, area);

        let title = if *success { "Success" } else { "Failed" };
        let color = if *success { Color::Green } else { Color::Red };
        let block = Paragraph::new(message.clone())
            .block(Block::default().borders(Borders::ALL).title(title))
            .style(Style::default().fg(color));
        frame.render_widget(block, area);
    }

    if let Some(message) = &app.status_message {
        let area = centered_rect(60, 20, frame.area());
        frame.render_widget(Clear, area);

        let block = Paragraph::new(message.clone())
            .block(Block::default().borders(Borders::ALL).title(" Status "))
            .style(Style::default().fg(Color::Cyan));
        frame.render_widget(block, area);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
