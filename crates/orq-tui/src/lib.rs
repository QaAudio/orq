use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use orq_core::Store;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Terminal;
use std::io::stdout;
use std::time::Duration;

pub fn run_watch(store: &Store, workspace: &str) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = watch_loop(&mut terminal, store, workspace);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn watch_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    store: &Store,
    workspace: &str,
) -> Result<()> {
    loop {
        let tasks = store.list_tasks(workspace, None, 40).unwrap_or_default();
        let events = store
            .list_events(workspace, None, 40, None)
            .unwrap_or_default();
        let tables = store.list_poi_tables(workspace).unwrap_or_default();
        let mut poi_lines = Vec::new();
        for t in tables.iter().take(5) {
            let pois = store.list_pois(workspace, &t.name, None, 8).unwrap_or_default();
            for p in pois {
                poi_lines.push(format!(
                    "{}/{} v{} {}",
                    t.name, p.key, p.version, p.state
                ));
            }
        }

        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Percentage(40),
                    Constraint::Percentage(30),
                    Constraint::Percentage(30),
                ])
                .split(f.area());

            let header = Paragraph::new(Line::from(vec![
                Span::styled(
                    " orq watch ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!("workspace={workspace}  q=quit")),
            ]))
            .block(Block::default().borders(Borders::ALL).title("orq"));
            f.render_widget(header, chunks[0]);

            let task_items: Vec<ListItem> = tasks
                .iter()
                .map(|t| {
                    ListItem::new(format!(
                        "{:<8} {:<12} {:<10} {}",
                        &t.id[..8.min(t.id.len())],
                        t.status.as_str(),
                        t.name,
                        t.command
                    ))
                })
                .collect();
            f.render_widget(
                List::new(task_items).block(Block::default().borders(Borders::ALL).title("tasks")),
                chunks[1],
            );

            let poi_items: Vec<ListItem> = poi_lines
                .iter()
                .map(|l| ListItem::new(l.as_str()))
                .collect();
            f.render_widget(
                List::new(poi_items).block(Block::default().borders(Borders::ALL).title("pois")),
                chunks[2],
            );

            let ev_items: Vec<ListItem> = events
                .iter()
                .rev()
                .take(30)
                .map(|e| ListItem::new(format!("{} {}", e.kind, e.payload)))
                .collect();
            f.render_widget(
                List::new(ev_items).block(Block::default().borders(Borders::ALL).title("events")),
                chunks[3],
            );
        })?;

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                    break;
                }
            }
        }
    }
    Ok(())
}
