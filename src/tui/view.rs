//! Rendering.
//!
//! Every row is one line and clips rather than wraps: a wrapped row would push
//! the panels below it off the bottom of a narrow terminal.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use super::app::View;
use super::doctor::{Level, Report};
use super::store_view::{StoreItem, human_bytes};
use super::theme;

/// Everything the UI draws from.
///
/// Rendering takes this rather than the app itself, so it depends only on
/// data. That keeps the live socket and lockfile handles out of the draw path,
/// and lets a frame be rendered and asserted on without a terminal.
pub struct Snapshot<'a> {
    pub consumer_dir: String,
    pub view: View,
    pub selected: usize,
    pub items: &'a [StoreItem],
    pub hijacked: &'a std::collections::HashSet<String>,
    pub activity: Vec<String>,
    pub registries: Option<String>,
    pub report: &'a Report,
    pub busy: Option<String>,
    pub error: Option<String>,
}

pub fn draw(frame: &mut Frame, app: &Snapshot) {
    let area = frame.area();
    frame.render_widget(Block::default().style(Style::default().bg(theme::BG)), area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(area);

    draw_header(frame, chunks[0], app);
    match app.view {
        View::Session => draw_session(frame, chunks[1], app),
        View::Store => draw_store(frame, chunks[1], app),
        View::Doctor => draw_doctor(frame, chunks[1], app),
    }
    draw_status_bar(frame, chunks[2], app);
}

fn panel(title: &str) -> Block<'_> {
    panel_styled(title, false)
}

/// The panel holding the selection gets a brighter border, so it is obvious
/// which one the arrow keys are driving.
fn panel_styled(title: &str, focused: bool) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if focused {
            theme::BORDER_ACTIVE
        } else {
            theme::BORDER
        }))
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(theme::DIM),
        ))
}

fn draw_header(frame: &mut Frame, area: Rect, app: &Snapshot) {
    let hijacking = app.hijacked.len();
    let (dot, dot_style, state) = if hijacking > 0 {
        (
            "●",
            Style::default().fg(theme::OK),
            format!("hijacking {hijacking}"),
        )
    } else {
        ("○", Style::default().fg(theme::DIM), "idle".to_string())
    };

    let registries = app
        .registries
        .clone()
        .unwrap_or_else(|| "detecting…".to_string());

    let pinned = if hijacking > 0 {
        "pinned · restores on quit"
    } else {
        "lockfile untouched"
    };

    // Setup state rides in the header so a broken CA or a stale daemon is
    // visible from every view, not only the one you have to go looking for.
    let (setup_mark, setup_colour, setup_text) = match app.report.worst() {
        Level::Ok => ("✓", theme::OK, "setup ok"),
        Level::Warn => ("!", theme::WARN, "setup has warnings"),
        Level::Bad => ("✗", theme::ERROR, "setup needs attention"),
    };

    let lines = vec![
        Line::from(vec![
            Span::styled(format!(" {dot} "), dot_style),
            Span::styled(
                state,
                Style::default()
                    .fg(theme::TEXT_STRONG)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("   {setup_mark} "),
                Style::default().fg(setup_colour),
            ),
            Span::styled(setup_text, Style::default().fg(theme::DIM)),
            Span::styled(
                format!("   smuggle · {}", app.view.title()),
                Style::default().fg(theme::DIM),
            ),
        ]),
        Line::from(vec![
            Span::styled("   project    ", Style::default().fg(theme::DIM)),
            Span::styled(app.consumer_dir.clone(), Style::default().fg(theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("   registries ", Style::default().fg(theme::DIM)),
            Span::styled(registries, Style::default().fg(theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("   lockfile   ", Style::default().fg(theme::DIM)),
            Span::styled(pinned, Style::default().fg(theme::TEXT)),
        ]),
    ];

    frame.render_widget(Paragraph::new(lines), area);
}

fn draw_session(frame: &mut Frame, area: Rect, app: &Snapshot) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(area);

    // Packages
    let inner = columns[0].inner(ratatui::layout::Margin::new(1, 1));
    frame.render_widget(panel_styled("Packages", true), columns[0]);

    let mut rows: Vec<Line> = Vec::new();
    if app.items.is_empty() {
        rows.push(Line::from(Span::styled(
            "nothing published yet — run `smuggle publish`",
            Style::default().fg(theme::DIM),
        )));
    }
    // The name takes whatever the version and size columns do not, so a wide
    // terminal shows full package names instead of truncating to a fixed width.
    let name_width = (inner.width as usize).saturating_sub(23).max(8);

    let (start, end) = window(app.selected, app.items.len(), inner.height as usize);
    for (i, item) in app.items.iter().enumerate().take(end).skip(start) {
        let on = app.hijacked.contains(&item.entry.name);
        let selected = i == app.selected;

        let row_bg = if selected { theme::PANEL } else { theme::BG };
        rows.push(Line::from(vec![
            Span::styled(
                if selected { "▸ " } else { "  " },
                Style::default().fg(theme::ACCENT).bg(row_bg),
            ),
            Span::styled(
                if on { "● " } else { "○ " },
                Style::default()
                    .fg(if on { theme::OK } else { theme::DIM })
                    .bg(row_bg),
            ),
            Span::styled(
                format!(
                    "{:<width$}",
                    truncate(&item.entry.name, name_width),
                    width = name_width
                ),
                Style::default()
                    .fg(if selected {
                        theme::TEXT_STRONG
                    } else {
                        theme::TEXT
                    })
                    .bg(row_bg),
            ),
            Span::styled(
                format!("{:<10} {:>8}", item.entry.version, human_bytes(item.bytes)),
                Style::default().fg(theme::DIM).bg(row_bg),
            ),
        ]));
    }
    frame.render_widget(Paragraph::new(rows), inner);

    // Activity
    let inner = columns[1].inner(ratatui::layout::Margin::new(1, 1));
    frame.render_widget(panel("Activity"), columns[1]);

    let lines: Vec<Line> = if app.activity.is_empty() {
        vec![Line::from(Span::styled(
            if app.hijacked.is_empty() {
                "pick a package with space to start hijacking it"
            } else {
                "no requests yet — run your package manager's install"
            },
            Style::default().fg(theme::DIM),
        ))]
    } else {
        app.activity
            .iter()
            .take(inner.height as usize)
            .map(|line| {
                let colour = if line.starts_with("served") || line.starts_with("rewrote") {
                    theme::OK
                } else {
                    theme::TEXT
                };
                Line::from(Span::styled(line.clone(), Style::default().fg(colour)))
            })
            .collect()
    };
    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_store(frame: &mut Frame, area: Rect, app: &Snapshot) {
    let inner = area.inner(ratatui::layout::Margin::new(1, 1));
    frame.render_widget(panel_styled("Store", true), area);

    let mut rows: Vec<Line> = Vec::new();
    if app.items.is_empty() {
        rows.push(Line::from(Span::styled(
            "nothing published yet — run `smuggle publish` in a package directory",
            Style::default().fg(theme::DIM),
        )));
    }
    let (start, end) = window(app.selected, app.items.len(), inner.height as usize);
    for (i, item) in app.items.iter().enumerate().take(end).skip(start) {
        let selected = i == app.selected;
        let mut spans = vec![
            Span::styled(
                if selected { "▸ " } else { "  " },
                Style::default().fg(theme::ACCENT),
            ),
            Span::styled(
                format!("{:<28}", item.entry.name),
                Style::default().fg(if selected {
                    theme::TEXT_STRONG
                } else {
                    theme::TEXT
                }),
            ),
            Span::styled(
                format!(
                    "{:<10} {:>9}  ",
                    item.entry.version,
                    human_bytes(item.bytes)
                ),
                Style::default().fg(theme::DIM),
            ),
            Span::styled(
                item.entry.source_dir.display().to_string(),
                Style::default().fg(theme::DIM),
            ),
        ];
        if item.source_missing {
            spans.push(Span::styled(
                "  ⚠ source gone",
                Style::default().fg(theme::WARN),
            ));
        }
        rows.push(Line::from(spans));
    }

    frame.render_widget(Paragraph::new(rows), inner);
}

fn draw_doctor(frame: &mut Frame, area: Rect, app: &Snapshot) {
    let inner = area.inner(ratatui::layout::Margin::new(1, 1));
    frame.render_widget(panel("Setup"), area);

    let mut rows: Vec<Line> = Vec::new();
    for check in &app.report.machine {
        rows.push(check_line(check));
    }
    rows.push(Line::from(""));
    rows.push(Line::from(Span::styled(
        "This project",
        Style::default()
            .fg(theme::TEXT_STRONG)
            .add_modifier(Modifier::BOLD),
    )));
    for check in &app.report.project {
        rows.push(check_line(check));
    }

    frame.render_widget(Paragraph::new(rows), inner);
}

fn check_line(check: &super::doctor::Check) -> Line<'static> {
    let (mark, colour) = match check.level {
        Level::Ok => ("✓", theme::OK),
        Level::Warn => ("!", theme::WARN),
        Level::Bad => ("✗", theme::ERROR),
    };
    Line::from(vec![
        Span::styled(format!(" {mark} "), Style::default().fg(colour)),
        Span::styled(
            format!("{:<22}", check.label),
            Style::default().fg(theme::TEXT),
        ),
        Span::styled(check.detail.clone(), Style::default().fg(theme::DIM)),
    ])
}

fn draw_status_bar(frame: &mut Frame, area: Rect, app: &Snapshot) {
    // Errors and progress replace the key hints, because when either is true
    // it is the only thing worth reading.
    if let Some(error) = &app.error {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!(" {error}"),
                Style::default().fg(theme::ERROR),
            ))),
            area,
        );
        return;
    }
    if let Some(busy) = &app.busy {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!(" {busy}…"),
                Style::default().fg(theme::WARN),
            ))),
            area,
        );
        return;
    }

    let mut spans = key("↑↓", "select");
    match app.view {
        View::Session => {
            spans.extend(key("space", "smuggle/unsmuggle"));
            spans.extend(key("r", "repack"));
        }
        View::Store => {
            spans.extend(key("x", "evict"));
            spans.extend(key("r", "repack"));
        }
        View::Doctor => {}
    }
    spans.extend(key("tab", "view"));
    spans.extend(key("q", "quit"));

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// The slice of a list to draw so the selection stays on screen.
fn window(selected: usize, len: usize, height: usize) -> (usize, usize) {
    if height == 0 || len == 0 {
        return (0, 0);
    }
    // Keep the selection in view by scrolling only as far as it needs.
    let start = selected
        .saturating_sub(height.saturating_sub(1))
        .min(len.saturating_sub(height).max(0));
    (start, (start + height).min(len))
}

/// Clip to a column width, since a wrapped row would push the panel's bottom
/// border off a narrow terminal.
fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    let kept: String = text.chars().take(width.saturating_sub(1)).collect();
    format!("{kept}…")
}

fn key(k: &str, label: &str) -> Vec<Span<'static>> {
    vec![
        Span::styled(
            format!(" {k}"),
            Style::default()
                .fg(theme::TEXT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {label}   "), Style::default().fg(theme::DIM)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::StoreEntry;
    use crate::tui::doctor::Check;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::collections::HashSet;
    use std::path::PathBuf;

    fn item(name: &str, missing: bool) -> StoreItem {
        StoreItem {
            entry: StoreEntry {
                name: name.into(),
                version: "1.0.0".into(),
                source_dir: PathBuf::from("/src/thing"),
                dependencies: Default::default(),
            },
            bytes: 2048,
            source_missing: missing,
        }
    }

    fn render(snapshot: &Snapshot) -> String {
        render_at(snapshot, 100, 24)
    }

    fn render_at(snapshot: &Snapshot, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| draw(frame, snapshot)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn base<'a>(
        items: &'a [StoreItem],
        hijacked: &'a HashSet<String>,
        report: &'a Report,
    ) -> Snapshot<'a> {
        Snapshot {
            consumer_dir: "/code/app".into(),
            view: View::Session,
            selected: 0,
            items,
            hijacked,
            activity: vec![],
            registries: Some("https://registry.npmjs.org/".into()),
            report,
            busy: None,
            error: None,
        }
    }

    fn empty_report() -> Report {
        Report {
            machine: vec![Check {
                level: Level::Ok,
                label: "local CA".into(),
                detail: "present".into(),
            }],
            project: vec![],
        }
    }

    /// Prints a frame so the layout can be eyeballed: `cargo test -- --nocapture frame`
    #[test]
    fn frame() {
        let items = vec![item("@sentry/browser", false), item("villus", false)];
        let hijacked = HashSet::from(["@sentry/browser".to_string()]);
        let report = empty_report();
        let mut snapshot = base(&items, &hijacked, &report);
        snapshot.activity = vec![
            "served @sentry/browser from the store (935974 bytes)".into(),
            "rewrote packument integrity for @sentry/browser".into(),
        ];
        println!("{}", render(&snapshot));
    }

    #[test]
    fn an_idle_session_says_how_to_start() {
        let items = vec![item("@scope/pkg", false)];
        let hijacked = HashSet::new();
        let report = empty_report();
        let out = render(&base(&items, &hijacked, &report));

        assert!(out.contains("idle"), "{out}");
        assert!(out.contains("@scope/pkg"), "{out}");
        assert!(out.contains("lockfile untouched"), "{out}");
        // The empty activity panel has to explain itself, or a session that is
        // doing nothing looks identical to one that is broken.
        assert!(out.contains("pick a package"), "{out}");
    }

    #[test]
    fn a_wide_panel_shows_full_package_names() {
        // The name column takes whatever the other columns leave, so a wide
        // terminal should not truncate a name that easily fits.
        let items = vec![item("@sentry/browser-utils", false)];
        let hijacked = HashSet::new();
        let report = empty_report();
        let snapshot = base(&items, &hijacked, &report);

        let wide = render_at(&snapshot, 160, 24);
        assert!(wide.contains("@sentry/browser-utils"), "{wide}");
        assert!(wide.contains("2 KB"), "size must survive too:\n{wide}");

        // The same name does not fit a narrow one, and is clipped rather than
        // pushing the size off the edge.
        let narrow = render_at(&snapshot, 80, 24);
        assert!(narrow.contains('…'), "{narrow}");
        assert!(narrow.contains("2 KB"), "{narrow}");
    }

    #[test]
    fn the_window_keeps_the_selection_on_screen() {
        // Short list: no scrolling.
        assert_eq!(window(0, 3, 10), (0, 3));
        // Selection past the bottom scrolls just far enough.
        assert_eq!(window(12, 40, 10), (3, 13));
        // Selection near the top does not scroll.
        assert_eq!(window(2, 40, 10), (0, 10));
        // Never scrolls past the end.
        assert_eq!(window(39, 40, 10), (30, 40));
        assert_eq!(window(0, 0, 10), (0, 0));
    }

    #[test]
    fn a_long_list_scrolls_to_the_selection() {
        let items: Vec<StoreItem> = (0..60)
            .map(|i| item(&format!("pkg-{i:02}"), false))
            .collect();
        let hijacked = HashSet::new();
        let report = empty_report();
        let mut snapshot = base(&items, &hijacked, &report);
        snapshot.selected = 55;
        let out = render(&snapshot);

        assert!(out.contains("pkg-55"), "selection must be visible:\n{out}");
        assert!(
            !out.contains("pkg-00"),
            "should have scrolled past the top:\n{out}"
        );
    }

    #[test]
    fn package_rows_keep_the_size_visible_in_a_narrow_panel() {
        // The columns have to fit the packages panel; an overflowing row loses
        // the size off the right edge.
        let items = vec![item("@scope/a-fairly-long-package-name", false)];
        let hijacked = HashSet::new();
        let report = empty_report();
        let out = render(&base(&items, &hijacked, &report));

        assert!(out.contains("2 KB"), "size was clipped:\n{out}");
        assert!(out.contains('…'), "long name should be truncated:\n{out}");
    }

    #[test]
    fn a_hijacked_session_reports_the_pin_and_waits_for_an_install() {
        let items = vec![item("@scope/pkg", false)];
        let hijacked = HashSet::from(["@scope/pkg".to_string()]);
        let report = empty_report();
        let out = render(&base(&items, &hijacked, &report));

        assert!(out.contains("hijacking 1"), "{out}");
        assert!(out.contains("restores on quit"), "{out}");
        assert!(out.contains("no requests yet"), "{out}");
    }

    #[test]
    fn the_store_view_flags_a_missing_source() {
        let items = vec![item("gone", true)];
        let hijacked = HashSet::new();
        let report = empty_report();
        let mut snapshot = base(&items, &hijacked, &report);
        snapshot.view = View::Store;
        let out = render(&snapshot);

        assert!(out.contains("source gone"), "{out}");
        assert!(out.contains("evict"), "{out}");
    }

    #[test]
    fn a_failing_check_shows_in_the_header_from_any_view() {
        let items = vec![];
        let hijacked = HashSet::new();
        let report = Report {
            machine: vec![Check {
                level: Level::Bad,
                label: "daemon".into(),
                detail: "not installed".into(),
            }],
            project: vec![],
        };
        let out = render(&base(&items, &hijacked, &report));
        assert!(out.contains("setup needs attention"), "{out}");
    }

    #[test]
    fn an_error_replaces_the_key_hints() {
        let items = vec![];
        let hijacked = HashSet::new();
        let report = empty_report();
        let mut snapshot = base(&items, &hijacked, &report);
        snapshot.error = Some("something broke".into());
        let out = render(&snapshot);

        assert!(out.contains("something broke"), "{out}");
        assert!(
            !out.contains("quit"),
            "hints should give way to the error:\n{out}"
        );
    }
}
