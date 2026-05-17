use crate::config::Config;
use crate::project::{ActivityLevel, Project, ProjectType};
use crate::scanner::{scan_directory, ScanResult};
use anyhow::Result;
use chrono::Utc;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
    Frame, Terminal,
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{io, path::PathBuf, time::Duration, time::Instant};

/// Filter modes for the project list
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterMode {
    All,
    Active,
    Stable,
    Stale,
    Game,
    Tool,
    Web,
}

impl FilterMode {
    pub fn label(&self) -> &'static str {
        match self {
            FilterMode::All => "All",
            FilterMode::Active => "Active",
            FilterMode::Stable => "Stable",
            FilterMode::Stale => "Stale",
            FilterMode::Game => "Game",
            FilterMode::Tool => "Tool",
            FilterMode::Web => "Web",
        }
    }
}

/// Application mode
#[derive(Debug, Clone, PartialEq, Eq)]
enum AppMode {
    Overview,
    Detail(usize),
    Searching,
}

/// The main TUI application
pub struct TuiApp {
    pub projects: Vec<Project>,
    pub filtered: Vec<usize>, // indices into projects
    pub selected: usize,       // index into filtered
    pub filter: FilterMode,
    pub search: String,
    mode: AppMode,
    pub config: Config,
    pub scan_result: Option<ScanResult>,
    pub show_help: bool,
    pub scroll_offset: u16,
    pub status_message: String,
}

impl TuiApp {
    pub fn new(config: Config) -> Self {
        Self {
            projects: Vec::new(),
            filtered: Vec::new(),
            selected: 0,
            filter: FilterMode::All,
            search: String::new(),
            mode: AppMode::Overview,
            config,
            scan_result: None,
            show_help: false,
            scroll_offset: 0,
            status_message: "Scanning projects...".to_string(),
        }
    }

    pub fn run(&mut self, scan_path: PathBuf) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;

        // Scan projects
        let mut scan_result = scan_directory(&scan_path, &self.config)?;
        let duration_ms = scan_result.scan_duration_ms;
        self.projects = std::mem::take(&mut scan_result.projects);
        self.scan_result = Some(scan_result);
        self.apply_filter();
        self.status_message = format!(
            "Scanned {} projects in {}ms. {}",
            self.projects.len(),
            duration_ms,
            if self.projects.is_empty() {
                "No projects found."
            } else {
                ""
            }
        );

        // Main loop
        let tick_rate = Duration::from_millis(250);
        let mut last_tick = Instant::now();

        let res = loop {
            terminal.draw(|f| self.render(f))?;

            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or(Duration::from_secs(0));

            if event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match self.handle_key(key.code, key.modifiers) {
                            Action::Continue => {}
                            Action::Quit => break Ok(()),
                            Action::Refresh => {
                                let mut scan_result = scan_directory(&scan_path, &self.config)?;
                                let duration = scan_result.scan_duration_ms;
                                self.projects = std::mem::take(&mut scan_result.projects);
                                self.scan_result = Some(scan_result);
                                self.apply_filter();
                                self.status_message =
                                    format!("Rescanned {} projects in {}ms", self.projects.len(), duration);
                            }
                        }
                    }
                }
            }

            if last_tick.elapsed() >= tick_rate {
                last_tick = Instant::now();
            }
        };

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        res
    }

    fn handle_key(&mut self, key: KeyCode, _modifiers: KeyModifiers) -> Action {
        match self.mode {
            AppMode::Overview => match key {
                KeyCode::Char('q') | KeyCode::Esc => return Action::Quit,
                KeyCode::Char('h') | KeyCode::Char('?') => {
                    self.show_help = !self.show_help;
                }
                KeyCode::Char('/') => {
                    self.mode = AppMode::Searching;
                    self.search.clear();
                }
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    return Action::Refresh;
                }
                KeyCode::Enter => {
                    if !self.filtered.is_empty() && self.selected < self.filtered.len() {
                        self.mode = AppMode::Detail(self.filtered[self.selected]);
                    }
                }
                KeyCode::Tab => {
                    self.cycle_filter();
                }
                KeyCode::Char('1') => self.filter = FilterMode::All,
                KeyCode::Char('2') => self.filter = FilterMode::Active,
                KeyCode::Char('3') => self.filter = FilterMode::Stable,
                KeyCode::Char('4') => self.filter = FilterMode::Stale,
                KeyCode::Char('5') => self.filter = FilterMode::Game,
                KeyCode::Char('6') => self.filter = FilterMode::Tool,
                KeyCode::Char('7') => self.filter = FilterMode::Web,
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.selected > 0 {
                        self.selected -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if self.selected < self.filtered.len().saturating_sub(1) {
                        self.selected += 1;
                    }
                }
                KeyCode::Left => {
                    if self.selected >= 3 {
                        self.selected -= 3;
                    }
                }
                KeyCode::Right => {
                    let max = self.filtered.len().saturating_sub(1);
                    if self.selected + 3 <= max {
                        self.selected += 3;
                    } else {
                        self.selected = max;
                    }
                }
                KeyCode::Home | KeyCode::Char('g') => {
                    self.selected = 0;
                    self.scroll_offset = 0;
                }
                KeyCode::End | KeyCode::Char('G') => {
                    self.selected = self.filtered.len().saturating_sub(1);
                }
                KeyCode::PageDown => {
                    self.selected = self.selected.saturating_add(8).min(self.filtered.len().saturating_sub(1));
                }
                KeyCode::PageUp => {
                    self.selected = self.selected.saturating_sub(8);
                }
                _ => {}
            },
            AppMode::Detail(idx) => match key {
                KeyCode::Esc | KeyCode::Left | KeyCode::Char('q') => {
                    self.mode = AppMode::Overview;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if idx > 0 {
                        self.mode = AppMode::Detail(idx - 1);
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if idx + 1 < self.projects.len() {
                        self.mode = AppMode::Detail(idx + 1);
                    }
                }
                _ => {}
            },
            AppMode::Searching => match key {
                KeyCode::Esc => {
                    self.mode = AppMode::Overview;
                    self.search.clear();
                    self.apply_filter();
                }
                KeyCode::Enter => {
                    self.mode = AppMode::Overview;
                    self.apply_filter();
                }
                KeyCode::Backspace => {
                    self.search.pop();
                    self.apply_filter();
                }
                KeyCode::Char(c) => {
                    self.search.push(c);
                    self.apply_filter();
                }
                _ => {}
            }
        }
        Action::Continue
    }

    fn cycle_filter(&mut self) {
        self.filter = match self.filter {
            FilterMode::All => FilterMode::Active,
            FilterMode::Active => FilterMode::Stable,
            FilterMode::Stable => FilterMode::Stale,
            FilterMode::Stale => FilterMode::Game,
            FilterMode::Game => FilterMode::Tool,
            FilterMode::Tool => FilterMode::Web,
            FilterMode::Web => FilterMode::All,
        };
        self.apply_filter();
    }

    fn apply_filter(&mut self) {
        self.filtered = self
            .projects
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                // Filter by type
                let type_match = match self.filter {
                    FilterMode::Game => matches!(p.project_type, ProjectType::Godot),
                    FilterMode::Tool => matches!(
                        p.project_type,
                        ProjectType::Rust
                            | ProjectType::Python
                            | ProjectType::ChromeExtension
                            | ProjectType::Nix
                            | ProjectType::Docker
                    ),
                    FilterMode::Web => matches!(
                        p.project_type,
                        ProjectType::TypeScript | ProjectType::JavaScript
                    ),
                    _ => true,
                };

                // Filter by activity
                let activity_match = match self.filter {
                    FilterMode::Active => p.activity == ActivityLevel::Active,
                    FilterMode::Stable => p.activity == ActivityLevel::Stable,
                    FilterMode::Stale => p.activity == ActivityLevel::Stale,
                    _ => true,
                };

                // Search filter
                let search_match = if self.search.is_empty() {
                    true
                } else {
                    let q = self.search.to_lowercase();
                    p.name.to_lowercase().contains(&q)
                        || p.project_type.label().to_lowercase().contains(&q)
                        || p
                            .agent
                            .as_ref()
                            .and_then(|a| a.current_phase.as_ref())
                            .map(|ph| ph.to_lowercase().contains(&q))
                            .unwrap_or(false)
                };

                type_match && activity_match && search_match
            })
            .map(|(i, _)| i)
            .collect();

        // Reset selection if out of bounds
        if !self.filtered.is_empty() && self.selected >= self.filtered.len() {
            self.selected = self.filtered.len() - 1;
        }
    }

    fn render(&mut self, frame: &mut Frame) {
        let size = frame.area();
        if size.width < 40 || size.height < 10 {
            let text = Text::from("Terminal too small. Minimum 40x10.");
            frame.render_widget(
                Paragraph::new(text).alignment(Alignment::Center).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Panoptic"),
                ),
                size,
            );
            return;
        }

        match self.mode {
            AppMode::Overview => self.render_overview(frame, size),
            AppMode::Detail(idx) => self.render_detail(frame, size, idx),
            AppMode::Searching => {
                self.render_overview(frame, size);
                self.render_search_overlay(frame, size);
            }
        }

        if self.show_help {
            self.render_help(frame, size);
        }
    }

    fn render_overview(&self, frame: &mut Frame, area: Rect) {
        let vertical = Layout::vertical([
            Constraint::Length(3),  // Header
            Constraint::Length(3),  // Filter bar
            Constraint::Min(0),     // Project grid
            Constraint::Length(1),  // Status bar
        ]);
        let [header_area, filter_area, grid_area, status_area] = vertical.areas(area);

        self.render_header(frame, header_area);
        self.render_filter_bar(frame, filter_area);
        self.render_project_grid(frame, grid_area);
        self.render_status_bar(frame, status_area);
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let active_count = self
            .projects
            .iter()
            .filter(|p| p.activity == ActivityLevel::Active)
            .count();
        let dirty_count = self
            .projects
            .iter()
            .filter(|p| p.git.as_ref().map(|g| g.is_dirty).unwrap_or(false))
            .count();

        let mut title_spans = vec![
            Span::styled("◉ ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled("panoptic", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::raw(format!("  —  {} projects", self.projects.len())),
        ];

        if dirty_count > 0 {
            title_spans.push(Span::raw("  "));
            title_spans.push(Span::styled(
                format!("● {} dirty", dirty_count),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ));
        }

        if active_count > 0 {
            title_spans.push(Span::raw("  "));
            title_spans.push(Span::styled(
                format!("● {} active", active_count),
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            ));
        }

        let title_line = Line::from(title_spans);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan));

        frame.render_widget(
            Paragraph::new(title_line)
                .block(block)
                .alignment(Alignment::Left),
            area,
        );
    }

    fn render_filter_bar(&self, frame: &mut Frame, area: Rect) {
        let filters = [
            ("1", FilterMode::All),
            ("2", FilterMode::Active),
            ("3", FilterMode::Stable),
            ("4", FilterMode::Stale),
            ("5", FilterMode::Game),
            ("6", FilterMode::Tool),
            ("7", FilterMode::Web),
        ];

        let mut spans = Vec::new();
        for (key, filter) in &filters {
            let is_active = *filter == self.filter;

            if is_active {
                spans.push(Span::styled(
                    format!(" [●] {} ", filter.label()),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD | Modifier::REVERSED),
                ));
            } else {
                spans.push(Span::styled(
                    format!(" [{}] {} ", key, filter.label()),
                    Style::default().fg(Color::DarkGray),
                ));
            }
            spans.push(Span::raw(" "));
        }

        // Search indicator
        spans.push(Span::raw("│ "));
        if self.search.is_empty() {
            spans.push(Span::styled(
                "[/] search",
                Style::default().fg(Color::DarkGray),
            ));
        } else {
            spans.push(Span::styled(
                format!("/{}/", self.search),
                Style::default().fg(Color::Yellow),
            ));
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray));

        frame.render_widget(
            Paragraph::new(Line::from(spans)).block(block),
            area,
        );
    }

    fn render_project_grid(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if self.filtered.is_empty() {
            let msg = if self.search.is_empty() {
                "No projects found."
            } else {
                "No projects match your search."
            };
            frame.render_widget(
                Paragraph::new(Text::from(msg))
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(Color::DarkGray)),
                inner,
            );
            return;
        }

        // Calculate grid dimensions
        let card_width: u16 = 22;
        let cols = (inner.width / (card_width + 2)).max(1);
        let rows = (self.filtered.len() as u16).div_ceil(cols);

        // Find our selected position
        let sel_row = self.selected as u16 / cols;
        let _sel_col = self.selected as u16 % cols;

        // Calculate visible range
        let max_visible_rows = inner.height / 10; // each card row is ~10 lines
        let scroll = if max_visible_rows > 0 {
            (sel_row).saturating_sub(max_visible_rows / 2).min(rows.saturating_sub(max_visible_rows))
        } else {
            0
        };

        let start_row = scroll;
        let end_row = (scroll + max_visible_rows).min(rows);

        // Render project cards for visible rows
        for row in start_row..end_row {
            for col in 0..cols {
                let idx = (row * cols + col) as usize;
                if idx >= self.filtered.len() {
                    break;
                }

                let project_idx = self.filtered[idx];
                let project = &self.projects[project_idx];
                let is_selected = idx == self.selected;

                let x = inner.x + col * (card_width + 2);
                let y = inner.y + (row - start_row) * 10;
                let card_area = Rect::new(x, y, card_width.min(inner.width.saturating_sub(col * (card_width + 2))), 9);

                if card_area.width < 10 || card_area.height < 5 {
                    continue;
                }

                self.render_project_card(frame, card_area, project, is_selected);
            }
        }
    }

    fn render_project_card(&self, frame: &mut Frame, area: Rect, project: &Project, selected: bool) {
        // Border style based on activity and selection
        let border_style = if selected {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            match project.activity {
                ActivityLevel::Active => Style::default().fg(Color::Green),
                ActivityLevel::Stable => Style::default().fg(Color::Yellow),
                ActivityLevel::Stale => Style::default().fg(Color::DarkGray),
                ActivityLevel::Done => Style::default().fg(Color::Blue),
                ActivityLevel::Archived => Style::default().fg(Color::DarkGray),
            }
        };

        let project_type_color = project.project_type_color();
        let activity_color = project.activity_color();

        let mut lines = Vec::new();

        // Name line
        let name_style = if selected {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        };

        let name = if project.name.len() as u16 > area.width.saturating_sub(4) {
            format!("{}…", &project.name[..(area.width as usize - 5)])
        } else {
            project.name.clone()
        };
        lines.push(Line::from(Span::styled(name, name_style)));

        // Divider
        lines.push(Line::from(
            Span::styled("─".repeat(area.width as usize - 2), Style::default().fg(Color::DarkGray)),
        ));

        // Activity + Type
        let activity_dot = match project.activity {
            ActivityLevel::Active => "●",
            ActivityLevel::Stable => "○",
            ActivityLevel::Stale => "◌",
            ActivityLevel::Done => "✓",
            ActivityLevel::Archived => "✗",
        };
        lines.push(Line::from(vec![
            Span::styled(activity_dot, activity_color),
            Span::raw(" "),
            Span::styled(project.activity.label(), activity_color),
            Span::raw("  "),
            Span::styled(project.project_type.label(), project_type_color),
        ]));

        // Size + files
        let size_text = format!("{}  {} files", project.size_human(), project.file_count_human());
        lines.push(Line::from(
            Span::styled(size_text, Style::default().fg(Color::DarkGray)),
        ));

        // Git status
        if let Some(git) = &project.git {
            let git_color = if git.is_dirty {
                Color::Red
            } else if git.ahead > 0 || git.behind > 0 {
                Color::Yellow
            } else {
                Color::Green
            };
            let git_symbol = if git.is_dirty { "⚡" } else { "✓" };
            let branch = if git.branch.len() as u16 > 12 {
                format!("{}…", &git.branch[..11])
            } else {
                git.branch.clone()
            };
            lines.push(Line::from(vec![
                Span::styled(git_symbol, Style::default().fg(git_color)),
                Span::raw(" "),
                Span::styled(branch, Style::default().fg(Color::DarkGray)),
            ]));
        }

        // Agent context
        if let Some(agent) = &project.agent {
            // Description (from README)
            if let Some(desc) = &agent.description {
                let desc_short = if desc.len() as u16 > area.width.saturating_sub(6) {
                    format!("{}…", &desc[..(area.width as usize - 7)])
                } else {
                    desc.clone()
                };
                lines.push(Line::from(
                    Span::styled(desc_short, Style::default().fg(Color::DarkGray)),
                ));
            }
            // Phase
            if let Some(phase) = &agent.current_phase {
                let phase_short = if phase.len() as u16 > area.width.saturating_sub(6) {
                    format!("{}…", &phase[..(area.width as usize - 7)])
                } else {
                    phase.clone()
                };
                lines.push(Line::from(
                    Span::styled(phase_short, Style::default().fg(Color::Magenta).add_modifier(Modifier::ITALIC)),
                ));
            }
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(if selected {
                BorderType::Double
            } else {
                BorderType::Plain
            })
            .border_style(border_style);

        frame.render_widget(
            Paragraph::new(Text::from(lines))
                .block(block)
                .alignment(Alignment::Left),
            area,
        );
    }

    fn render_status_bar(&self, frame: &mut Frame, area: Rect) {
        let mut spans = vec![
            Span::styled(" ↑↓←→ nav ", Style::default().fg(Color::DarkGray)),
            Span::styled(" / search ", Style::default().fg(Color::DarkGray)),
            Span::styled(" Tab filter ", Style::default().fg(Color::DarkGray)),
            Span::styled(" ↵ detail ", Style::default().fg(Color::DarkGray)),
            Span::styled(" r refresh ", Style::default().fg(Color::DarkGray)),
            Span::styled(" ? help ", Style::default().fg(Color::DarkGray)),
            Span::styled(" q quit ", Style::default().fg(Color::DarkGray)),
        ];

        // Status message on the right
        if !self.status_message.is_empty() {
            spans.push(Span::raw("  │  "));
            spans.push(Span::styled(
                &self.status_message,
                Style::default().fg(Color::DarkGray),
            ));
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray));

        frame.render_widget(
            Paragraph::new(Line::from(spans)).block(block),
            area,
        );
    }

    fn render_detail(&self, frame: &mut Frame, area: Rect, project_idx: usize) {
        if project_idx >= self.projects.len() {
            return;
        }
        let project = &self.projects[project_idx];

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan))
            .title(format!(" {} ", project.name))
            .title_alignment(Alignment::Center);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Split into left and right panes
        let horizontal = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]);
        let [left, right] = horizontal.areas(inner);

        // Left pane: project info + git
        let left_layout = Layout::vertical([
            Constraint::Length(8),  // Project info
            Constraint::Min(0),     // Git state
        ]);
        let [info_area, git_area] = left_layout.areas(left);

        self.render_detail_info(frame, info_area, project);
        if project.git.is_some() {
            self.render_detail_git(frame, git_area, project);
        }

        // Right pane: agent context + activity
        let right_layout = Layout::vertical([
            Constraint::Min(0),     // Agent context
            Constraint::Length(3),  // Keybindings
        ]);
        let [agent_area, keys_area] = right_layout.areas(right);

        if project.agent.is_some() {
            self.render_detail_agent(frame, agent_area, project);
        } else {
            frame.render_widget(
                Paragraph::new(Text::from("No agent context found.\n(CLAUDE.md, AGENTS.md, brief.md, etc.)"))
                    .style(Style::default().fg(Color::DarkGray))
                    .alignment(Alignment::Center),
                agent_area,
            );
        }

        // Keybindings
        frame.render_widget(
            Paragraph::new(Text::from(vec![
                Line::from(vec![
                    Span::styled(" ↑/k prev  ", Style::default().fg(Color::DarkGray)),
                    Span::styled(" ↓/j next  ", Style::default().fg(Color::DarkGray)),
                    Span::styled(" Esc/q back  ", Style::default().fg(Color::DarkGray)),
                ])
            ]))
            .alignment(Alignment::Center),
            keys_area,
        );
    }

    fn render_detail_info(&self, frame: &mut Frame, area: Rect, project: &Project) {
        let mut lines = vec![
            Line::from(vec![
                Span::styled("Path:     ", Style::default().fg(Color::DarkGray)),
                Span::raw(project.path.to_string_lossy().to_string()),
            ]),
            Line::from(vec![
                Span::styled("Type:     ", Style::default().fg(Color::DarkGray)),
                Span::styled(project.project_type.label(), project.project_type_color()),
                Span::raw("  "),
                Span::styled(project.activity.label(), project.activity_color()),
            ]),
            Line::from(vec![
                Span::styled("Size:     ", Style::default().fg(Color::DarkGray)),
                Span::raw(format!("{}  ({} files)", project.size_human(), project.file_count)),
            ]),
        ];

        if let Some(created) = project.created {
            let days = (Utc::now() - created).num_days();
            lines.push(Line::from(vec![
                Span::styled("Created:  ", Style::default().fg(Color::DarkGray)),
                Span::raw(format!("{}d ago", days)),
            ]));
        }

        let days = project.days_since_modified();
        lines.push(Line::from(vec![
            Span::styled("Modified: ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{}d ago", days)),
        ]));

        if let Some(git) = &project.git {
            if let Some(msg) = &git.last_commit_message {
                let msg_short = if msg.len() > 50 {
                    format!("{}…", &msg[..49])
                } else {
                    msg.clone()
                };
                lines.push(Line::from(vec![
                    Span::styled("Last cmt: ", Style::default().fg(Color::DarkGray)),
                    Span::raw(msg_short),
                ]));
            }
        }

        // Show project description from README in the info pane
        if let Some(agent) = &project.agent {
            if let Some(desc) = &agent.description {
                let desc_display = if desc.len() > 60 {
                    format!("{}…", &desc[..59])
                } else {
                    desc.clone()
                };
                lines.push(Line::from(vec![
                    Span::styled("Desc:     ", Style::default().fg(Color::DarkGray)),
                    Span::styled(desc_display, Style::default().fg(Color::White).add_modifier(Modifier::ITALIC)),
                ]));
            }
        }

        let block = Block::default()
            .title(" Info ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue));

        frame.render_widget(
            Paragraph::new(Text::from(lines)).block(block),
            area,
        );
    }

    fn render_detail_git(&self, frame: &mut Frame, area: Rect, project: &Project) {
        if let Some(git) = &project.git {
            let mut lines = vec![
                Line::from(vec![
                    Span::styled("Branch:  ", Style::default().fg(Color::DarkGray)),
                    Span::styled(&git.branch, Style::default().fg(Color::Yellow)),
                ]),
                Line::from(vec![
                    Span::styled("Status:  ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        if git.is_dirty { "● Dirty" } else { "✓ Clean" },
                        Style::default().fg(if git.is_dirty { Color::Red } else { Color::Green }),
                    ),
                ]),
            ];

            if git.staged > 0 || git.unstaged > 0 || git.untracked > 0 {
                let changes = format!(
                    "  (+{} staged, +{} unstaged, {} untracked)",
                    git.staged, git.unstaged, git.untracked
                );
                lines.push(Line::from(Span::styled(
                    changes,
                    Style::default().fg(Color::DarkGray),
                )));
            }

            if git.ahead > 0 || git.behind > 0 {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("Ahead: {}  Behind: {}", git.ahead, git.behind),
                        Style::default().fg(Color::Yellow),
                    ),
                ]));
            }

            if let Some(time) = git.last_commit_time {
                let days = (Utc::now() - time).num_days();
                if let Some(author) = &git.last_commit_author {
                    lines.push(Line::from(vec![
                        Span::styled("Author:  ", Style::default().fg(Color::DarkGray)),
                        Span::raw(author),
                        Span::raw(format!(" ({}d ago)", days)),
                    ]));
                }
            }

            lines.push(Line::from(vec![
                Span::styled("Commits: ", Style::default().fg(Color::DarkGray)),
                Span::raw(format!("{} total", git.total_commits)),
                Span::raw(format!("  ({} stashes)", git.stash_count)),
            ]));

            if git.has_remote {
                lines.push(Line::from(Span::styled(
                    "✓ Remote configured",
                    Style::default().fg(Color::Green),
                )));
            }

            let block = Block::default()
                .title(" Git ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow));

            frame.render_widget(
                Paragraph::new(Text::from(lines)).block(block),
                area,
            );
        }
    }

    fn render_detail_agent(&self, frame: &mut Frame, area: Rect, project: &Project) {
        if let Some(agent) = &project.agent {
            let mut lines = Vec::new();

            // Description from README
            if let Some(desc) = &agent.description {
                let desc_short = if desc.len() > 55 {
                    format!("{}…", &desc[..54])
                } else {
                    desc.clone()
                };
                lines.push(Line::from(vec![
                    Span::styled("About:  ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                    Span::raw(desc_short),
                ]));
                lines.push(Line::from(Span::raw("")));
            }

            if let Some(phase) = &agent.current_phase {
                lines.push(Line::from(vec![
                    Span::styled("Phase: ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                    Span::raw(phase),
                ]));
                lines.push(Line::from(Span::raw("")));
            }

            if let Some(task) = &agent.current_task {
                lines.push(Line::from(vec![
                    Span::styled("Task:  ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::raw(task),
                ]));
                lines.push(Line::from(Span::raw("")));
            }

            if agent.checklist_total > 0 {
                let pct = agent.checklist_done as f64 / agent.checklist_total as f64 * 100.0;
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("Progress: {}/{} ({:.0}%)", agent.checklist_done, agent.checklist_total, pct),
                        Style::default().fg(Color::Green),
                    ),
                ]));
                lines.push(Line::from(Span::raw("")));
            }

            if !agent.next_steps.is_empty() {
                lines.push(Line::from(Span::styled(
                    "Next Steps:",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                )));
                for step in agent.next_steps.iter().take(8) {
                    let display = if step.len() > 45 {
                        format!("{}…", &step[..44])
                    } else {
                        step.clone()
                    };
                    lines.push(Line::from(vec![
                        Span::styled("  • ", Style::default().fg(Color::Yellow)),
                        Span::raw(display),
                    ]));
                }
                if agent.next_steps.len() > 8 {
                    lines.push(Line::from(Span::styled(
                        format!("  … and {} more", agent.next_steps.len() - 8),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
                lines.push(Line::from(Span::raw("")));
            }

            if !agent.blockers.is_empty() {
                lines.push(Line::from(Span::styled(
                    "Blockers:",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )));
                for blocker in &agent.blockers {
                    lines.push(Line::from(vec![
                        Span::styled("  ✗ ", Style::default().fg(Color::Red)),
                        Span::raw(blocker),
                    ]));
                }
                lines.push(Line::from(Span::raw("")));
            }

            if !agent.recent_decisions.is_empty() {
                lines.push(Line::from(Span::styled(
                    "Decisions:",
                    Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
                )));
                for decision in agent.recent_decisions.iter().take(3) {
                    lines.push(Line::from(vec![
                        Span::styled("  → ", Style::default().fg(Color::Blue)),
                        Span::raw(decision),
                    ]));
                }
            }

            let block = Block::default()
                .title(" Agent Context ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Magenta));

            frame.render_widget(
                Paragraph::new(Text::from(lines)).block(block).wrap(Wrap { trim: false }),
                area,
            );
        }
    }

    fn render_search_overlay(&self, frame: &mut Frame, area: Rect) {
        let overlay_area = Rect::new(
            area.width / 4,
            area.height / 3,
            area.width / 2,
            3,
        );

        let block = Block::default()
            .title(" Search ")
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(Color::Cyan))
            .style(Style::default().bg(Color::Black));

        frame.render_widget(
            Clear,
            overlay_area,
        );

        frame.render_widget(
            Paragraph::new(Text::from(vec![Line::from(vec![
                Span::raw("> "),
                Span::styled(
                    &self.search,
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                ),
                Span::styled("█", Style::default().fg(Color::Cyan)),
            ])]))
            .block(block),
            overlay_area,
        );
    }

    fn render_help(&self, frame: &mut Frame, area: Rect) {
        let help_area = Rect::new(
            area.width / 6,
            area.height / 6,
            area.width * 2 / 3,
            area.height * 2 / 3,
        );

        let help_text = vec![
            Line::from(Span::styled("Panoptic Help", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
            Line::from(Span::raw("")),
            Line::from(Span::styled("Overview", Style::default().fg(Color::Yellow))),
            Line::from(Span::raw("  ↑/k, ↓/j    Navigate projects")),
            Line::from(Span::raw("  ←/→          Navigate grid")),
            Line::from(Span::raw("  Enter        Open detail view")),
            Line::from(Span::raw("  Tab          Cycle filter")),
            Line::from(Span::raw("  1-7          Quick filter")),
            Line::from(Span::raw("  /            Search")),
            Line::from(Span::raw("  r            Refresh scan")),
            Line::from(Span::raw("  q/Esc        Quit")),
            Line::from(Span::raw("  ?/h         Toggle help")),
            Line::from(Span::raw("")),
            Line::from(Span::styled("Detail View", Style::default().fg(Color::Yellow))),
            Line::from(Span::raw("  ↑/k, ↓/j    Previous/next project")),
            Line::from(Span::raw("  Esc/q        Back to overview")),
            Line::from(Span::raw("")),
            Line::from(Span::styled("Search Mode", Style::default().fg(Color::Yellow))),
            Line::from(Span::raw("  Type to search project names, types, and phases")),
            Line::from(Span::raw("  Enter/ESC    Close search")),
        ];

        let block = Block::default()
            .title(" Help ")
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(Color::Cyan))
            .style(Style::default().bg(Color::Black));

        frame.render_widget(Clear, help_area);
        frame.render_widget(
            Paragraph::new(Text::from(help_text))
                .block(block)
                .alignment(Alignment::Left),
            help_area,
        );
    }
}

enum Action {
    Continue,
    Quit,
    Refresh,
}

// Color helper traits
impl Project {
    pub fn project_type_color(&self) -> Color {
        match self.project_type {
            ProjectType::Rust => Color::Red,
            ProjectType::TypeScript => Color::Blue,
            ProjectType::JavaScript => Color::Yellow,
            ProjectType::Python => Color::Green,
            ProjectType::Godot => Color::Cyan,
            ProjectType::Go => Color::Magenta,
            ProjectType::Nix => Color::Blue,
            ProjectType::Docker => Color::Rgb(0, 105, 185),
            ProjectType::ChromeExtension => Color::Rgb(0, 200, 100),
            ProjectType::Generic => Color::DarkGray,
            ProjectType::Unknown => Color::DarkGray,
        }
    }

    pub fn activity_color(&self) -> Color {
        match self.activity {
            ActivityLevel::Active => Color::Green,
            ActivityLevel::Stable => Color::Yellow,
            ActivityLevel::Stale => Color::DarkGray,
            ActivityLevel::Done => Color::Blue,
            ActivityLevel::Archived => Color::DarkGray,
        }
    }
}
