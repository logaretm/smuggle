//! The terminal UI.
//!
//! `smuggle ui` owns a session: nothing is hijacked until you pick it, and
//! quitting ends the session the way ctrl-c does, restoring the lockfile and
//! reinstalling the published packages.

pub mod app;
pub mod doctor;
pub mod store_view;
pub mod theme;
pub mod view;

use std::path::Path;
use std::sync::mpsc::{Sender, TryRecvError, channel};
use std::time::Duration;

use ratatui::crossterm::event::{self, KeyCode, KeyEventKind, KeyModifiers};

use crate::net::control::{Reply, Request, version};
use crate::{lockfile, session};

use app::{App, View};

/// How often the loop wakes to redraw when nothing has happened. Also the
/// cadence at which the daemon is asked for its status.
const TICK: Duration = Duration::from_millis(200);
const STATUS_EVERY: u32 = 10;

pub fn run(consumer_dir: &Path, extra_registries: &[String]) -> Result<(), String> {
    // Checked before anything is registered or pinned: initialising a terminal
    // that is not there panics, and would leave a session behind.
    if !std::io::IsTerminal::is_terminal(&std::io::stdout()) {
        return Err(
            "`smuggle ui` needs an interactive terminal. Use `smuggle` for a plain session.".into(),
        );
    }

    let consumer_dir = consumer_dir
        .canonicalize()
        .map_err(|e| format!("invalid path: {e}"))?;

    // Fail before taking over the terminal, so the reason stays readable.
    let lock = lockfile::detect(&consumer_dir)?;
    let registries = session::resolve_registries_quietly(&consumer_dir, extra_registries);

    // Register with nothing hijacked. The UI decides what to smuggle.
    let (replies_tx, replies_rx) = channel::<Reply>();
    let registration = session::start_with_sink(&[], &registries, false, Some(replies_tx.clone()))?;
    let pin = lockfile::pin(&lock, &Default::default(), false)?;

    let mut app = App::new(consumer_dir, registration, lock, pin);
    let status_tx = replies_tx;

    let mut terminal =
        ratatui::try_init().map_err(|e| format!("could not take over the terminal: {e}"))?;
    let outcome = event_loop(&mut terminal, &mut app, &replies_rx, &status_tx);
    ratatui::restore();

    app.shutdown();
    outcome
}

fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    replies: &std::sync::mpsc::Receiver<Reply>,
    status_tx: &Sender<Reply>,
) -> Result<(), String> {
    let mut ticks: u32 = 0;

    loop {
        terminal
            .draw(|frame| view::draw(frame, &app.snapshot()))
            .map_err(|e| format!("draw failed: {e}"))?;

        if event::poll(TICK).map_err(|e| format!("input failed: {e}"))? {
            if let Ok(event::Event::Key(key)) = event::read() {
                if key.kind == KeyEventKind::Press {
                    handle_key(app, key.code, key.modifiers);
                }
            }
        }

        loop {
            match replies.try_recv() {
                Ok(reply) => app.on_reply(reply),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }

        while let Ok(work) = app.work_rx.try_recv() {
            app.on_work(work);
        }

        ticks = ticks.wrapping_add(1);
        if ticks.is_multiple_of(STATUS_EVERY) {
            poll_status(status_tx);
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

/// Ask the daemon what it is doing on a short-lived connection, and feed the
/// answer back through the same channel the event stream uses.
fn poll_status(tx: &Sender<Reply>) {
    let tx = tx.clone();
    std::thread::spawn(move || {
        use std::io::{BufRead, BufReader, Write};

        let Ok(stream) =
            std::os::unix::net::UnixStream::connect(crate::net::control::socket_path())
        else {
            return;
        };
        let request =
            serde_json::to_string(&Request::Status { version: version() }).unwrap_or_default();
        if writeln!(&stream, "{request}").is_err() {
            return;
        }

        let mut line = String::new();
        if BufReader::new(&stream).read_line(&mut line).is_ok() {
            if let Ok(reply) = serde_json::from_str::<Reply>(line.trim()) {
                let _ = tx.send(reply);
            }
        }
    });
}

fn handle_key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    match code {
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => app.should_quit = true,
        KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,

        KeyCode::Tab => {
            app.view = match app.view {
                View::Session => View::Store,
                View::Store => View::Doctor,
                View::Doctor => View::Session,
            };
            app.refresh();
        }

        KeyCode::Up | KeyCode::Char('k') => app.move_selection(-1),
        KeyCode::Down | KeyCode::Char('j') => app.move_selection(1),

        KeyCode::Char(' ') if app.view == View::Session => app.toggle_selected(),
        KeyCode::Char('r') => app.repack_selected(),
        KeyCode::Char('x') if app.view == View::Store => app.evict_selected(),

        _ => {}
    }
}
