//! Application state.
//!
//! The UI owns the session: nothing is hijacked until you pick it, and quitting
//! ends the session exactly as ctrl-c does, restoring the lockfile and putting
//! the real packages back.

use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};

use crate::net::control::{DaemonStatus, Event, Reply};
use crate::net::hijack;
use crate::{lockfile, session, store};

use super::doctor;
use super::store_view::{self, StoreItem};

/// How many activity lines to keep. Enough to see what an install did without
/// growing without bound over a long session.
const ACTIVITY_LIMIT: usize = 500;

#[derive(Clone, Copy, PartialEq)]
pub enum View {
    Session,
    Store,
    Doctor,
}

impl View {
    pub fn title(self) -> &'static str {
        match self {
            View::Session => "session",
            View::Store => "store",
            View::Doctor => "doctor",
        }
    }
}

/// Work that must not block the event loop.
pub enum Work {
    /// The registration changed; re-pin and reinstall.
    Applied(Result<String, String>),
    Repacked(Result<String, String>),
}

pub struct App {
    pub consumer_dir: PathBuf,
    pub view: View,
    pub selected: usize,
    pub items: Vec<StoreItem>,
    /// Packages currently hijacked. Empty until the user picks something.
    pub hijacked: HashSet<String>,
    pub activity: VecDeque<String>,
    pub status: Option<DaemonStatus>,
    pub report: doctor::Report,
    pub busy: Option<String>,
    pub error: Option<String>,
    pub should_quit: bool,

    registration: session::Registration,
    pin: Option<lockfile::Pin>,
    lock: lockfile::Lockfile,
    work_tx: Sender<Work>,
    pub work_rx: Receiver<Work>,
}

impl App {
    pub fn new(
        consumer_dir: PathBuf,
        registration: session::Registration,
        lock: lockfile::Lockfile,
        pin: lockfile::Pin,
    ) -> Self {
        let (work_tx, work_rx) = channel();
        let report = doctor::run(&consumer_dir, None);

        Self {
            consumer_dir,
            view: View::Session,
            selected: 0,
            items: store_view::load(),
            hijacked: HashSet::new(),
            activity: VecDeque::new(),
            status: None,
            report,
            busy: None,
            error: None,
            should_quit: false,
            registration,
            pin: Some(pin),
            lock,
            work_tx,
            work_rx,
        }
    }

    /// The data the UI draws from, with the live handles left behind.
    pub fn snapshot(&self) -> super::view::Snapshot<'_> {
        super::view::Snapshot {
            consumer_dir: self.consumer_dir.display().to_string(),
            view: self.view,
            selected: self.selected,
            items: &self.items,
            hijacked: &self.hijacked,
            activity: self.activity.iter().cloned().collect(),
            registries: self
                .status
                .as_ref()
                .map(|s| s.registries.join(", "))
                .filter(|r| !r.is_empty()),
            report: &self.report,
            busy: self.busy.clone(),
            error: self.error.clone(),
        }
    }

    pub fn selected_item(&self) -> Option<&StoreItem> {
        self.items.get(self.selected)
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.items.is_empty() {
            return;
        }
        let last = self.items.len() - 1;
        self.selected = match delta {
            d if d < 0 => self.selected.saturating_sub(d.unsigned_abs()),
            d => (self.selected + d as usize).min(last),
        };
    }

    pub fn push_activity(&mut self, line: String) {
        self.activity.push_front(format!("{}  {line}", clock()));
        self.activity.truncate(ACTIVITY_LIMIT);
    }

    pub fn on_reply(&mut self, reply: Reply) {
        match reply {
            Reply::Event { event } => {
                if let Event::Error { message } = &event {
                    self.error = Some(message.clone());
                }
                self.push_activity(event.to_line());
            }
            Reply::Log { line } => self.push_activity(line),
            Reply::Error { message } => self.error = Some(message),
            Reply::Status { status } => {
                self.report = doctor::run(&self.consumer_dir, Some(&status));
                self.status = Some(status);
            }
            Reply::Ok => {}
        }
    }

    /// Toggle whether the selected package is hijacked, then apply the change.
    pub fn toggle_selected(&mut self) {
        if self.busy.is_some() {
            return;
        }
        let Some(item) = self.selected_item() else {
            return;
        };
        let name = item.entry.name.clone();

        if !self.hijacked.remove(&name) {
            self.hijacked.insert(name.clone());
        }
        self.apply(format!("applying {name}"));
    }

    /// Push the current set to the daemon, re-pin the lockfile, and reinstall.
    /// The install runs off the event loop so the UI keeps drawing.
    fn apply(&mut self, label: String) {
        let packages: Vec<String> = {
            let mut v: Vec<String> = self.hijacked.iter().cloned().collect();
            v.sort();
            v
        };

        if let Err(e) = self.registration.set_packages(&packages) {
            self.error = Some(e);
            return;
        }

        let Some(pin) = &self.pin else { return };
        let hashes = tarball_hashes(&packages);
        let pinned = match pin.set(&hashes) {
            Ok(count) => count,
            Err(e) => {
                self.error = Some(e);
                return;
            }
        };

        self.busy = Some(label);
        self.error = None;
        self.push_activity(format!(
            "pinned {pinned} lockfile entr{}",
            if pinned == 1 { "y" } else { "ies" }
        ));

        let dir = self.consumer_dir.clone();
        let kind = self.lock.kind;
        let tx = self.work_tx.clone();
        std::thread::spawn(move || {
            let result = lockfile::install(&dir, kind)
                .map(|()| "install finished".to_string())
                .map_err(|e| e.to_string());
            let _ = tx.send(Work::Applied(result));
        });
    }

    pub fn repack_selected(&mut self) {
        if self.busy.is_some() {
            return;
        }
        let Some(item) = self.selected_item() else {
            return;
        };
        if item.source_missing {
            self.error = Some(format!(
                "{} was packed from {}, which no longer exists",
                item.entry.name,
                item.entry.source_dir.display()
            ));
            return;
        }

        let dir = item.entry.source_dir.clone();
        self.busy = Some(format!("repacking {}", item.entry.name));
        let tx = self.work_tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Work::Repacked(store_view::repack(&dir)));
        });
    }

    /// Remove the selected package from the store. Refuses while it is
    /// hijacked, since the proxy would then have nothing to serve.
    pub fn evict_selected(&mut self) {
        let Some(item) = self.selected_item() else {
            return;
        };
        let name = item.entry.name.clone();

        if self.hijacked.contains(&name) {
            self.error = Some(format!("{name} is hijacked — unsmuggle it first"));
            return;
        }

        match store::remove(&name) {
            Ok(()) => {
                self.push_activity(format!("evicted {name} from the store"));
                self.items = store_view::load();
                self.selected = self.selected.min(self.items.len().saturating_sub(1));
            }
            Err(e) => self.error = Some(e),
        }
    }

    pub fn on_work(&mut self, work: Work) {
        self.busy = None;
        match work {
            Work::Applied(Ok(line)) => self.push_activity(line),
            Work::Repacked(Ok(name)) => {
                self.items = store_view::load();
                self.push_activity(format!("repacked {name}"));
                // The tarball changed, so the pin has to follow it.
                if self.hijacked.contains(&name) {
                    self.apply(format!("reinstalling {name}"));
                }
            }
            Work::Applied(Err(e)) | Work::Repacked(Err(e)) => self.error = Some(e),
        }
    }

    pub fn refresh(&mut self) {
        self.items = store_view::load();
        self.report = doctor::run(&self.consumer_dir, self.status.as_ref());
    }

    /// End the session the way ctrl-c does: unpin, stop interception, and put
    /// the real packages back.
    pub fn shutdown(self) {
        let App {
            pin,
            registration,
            consumer_dir,
            lock,
            hijacked,
            ..
        } = self;

        drop(pin);
        drop(registration);

        // Only worth reinstalling if something was actually swapped out.
        if !hijacked.is_empty() {
            session::wait_for_interception_to_stop();
            let _ = lockfile::install(&consumer_dir, lock.kind);
        }
    }
}

/// Wall-clock time for the activity feed. Falls back to UTC when the local
/// offset cannot be determined, which is possible in a threaded process.
fn clock() -> String {
    let now = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    format!("{:02}:{:02}:{:02}", now.hour(), now.minute(), now.second())
}

/// The integrity of what the proxy will serve for each package, which is what
/// the lockfile has to claim for the fetch to be accepted.
fn tarball_hashes(names: &[String]) -> std::collections::HashMap<String, String> {
    names
        .iter()
        .filter_map(|name| {
            let tarball = store::load_tarball(name).ok()?;
            Some((name.clone(), hijack::integrity_of(&tarball)))
        })
        .collect()
}
