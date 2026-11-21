//! The one-call import path: `Api::import_tally_http(host, port, company)` run on the API worker,
//! with the `ImportSummary` shown through its `Display` and any failure shown as the error.
//!
//! This sits beside the wizard, whose steps (detect, choose, pull, map, reconcile) are unchanged.
//! The API call pulls, maps and commits in one go, so none of the wizard's per-step figures are
//! needed here. The screen shows only what the API returned (rule C4).

use ledgerly_api::ImportSummary;
use ledgerly_core::{Outcome, Unsupported};

use super::types::TALLY_PORT;
use crate::screens::wire::{Done, Latest, unsupported_text};
use crate::worker::ApiWorker;

/// What the one-call import shows now.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum DirectState {
    /// Nothing has been run.
    #[default]
    Idle,
    /// The call is running on the worker.
    Running,
    /// Tally was read and the events were committed.
    Imported(ImportSummary),
    /// The API refused (for example the company already holds data), with its reason.
    Refused(Unsupported),
    /// The call failed (Tally unreachable, bad XML, a commit error, no company open).
    Failed(String),
}

impl DirectState {
    /// The text the screen shows for this state, or `None` when there is nothing to show. A
    /// summary is shown through its `Display`; a refusal through `unsupported_text`.
    #[must_use]
    pub fn text(&self) -> Option<String> {
        match self {
            DirectState::Idle => None,
            DirectState::Running => Some("Importing from Tally...".to_owned()),
            DirectState::Imported(summary) => Some(summary.to_string()),
            DirectState::Refused(u) => Some(unsupported_text(u)),
            DirectState::Failed(message) => Some(format!("Error: {message}")),
        }
    }
}

/// The form fields and the running call of the one-call import.
#[derive(Debug)]
pub(super) struct DirectImport {
    pub(super) host: String,
    pub(super) port: String,
    pub(super) company: String,
    pub(super) state: DirectState,
    latest: Latest<Result<Outcome<ImportSummary>, String>>,
}

impl Default for DirectImport {
    fn default() -> Self {
        Self {
            host: "localhost".to_owned(),
            port: TALLY_PORT.to_string(),
            company: String::new(),
            state: DirectState::Idle,
            latest: Latest::default(),
        }
    }
}

impl DirectImport {
    /// Checks the fields and starts the call. A bad field is a `Failed` state and nothing runs.
    pub(super) fn start(&mut self, worker: Option<&ApiWorker>) {
        let host = self.host.trim().to_owned();
        let company = self.company.trim().to_owned();
        if host.is_empty() {
            self.state = DirectState::Failed("enter the host Tally runs on".to_owned());
            return;
        }
        if company.is_empty() {
            self.state = DirectState::Failed("enter the name of the Tally company".to_owned());
            return;
        }
        let Ok(port) = self.port.trim().parse::<u16>() else {
            self.state = DirectState::Failed(format!(
                "\"{}\" is not a port number (1 to 65535)",
                self.port.trim()
            ));
            return;
        };
        let Some(handle) = worker else {
            self.state =
                DirectState::Failed("no company file is open to import into".to_owned());
            return;
        };
        self.state = DirectState::Running;
        self.latest.start(handle, move |api| {
            api.import_tally_http(&host, port, &company).map_err(|e| {
                format!("could not import company \"{company}\" from Tally at {host}:{port}: {e}")
            })
        });
    }

    /// Takes a finished answer, if there is one.
    pub(super) fn tick(&mut self) {
        match self.latest.poll() {
            Some(Done::Ready(Ok(Outcome::Value(summary)))) => {
                self.state = DirectState::Imported(summary);
            }
            Some(Done::Ready(Ok(Outcome::Unsupported(u)))) => {
                self.state = DirectState::Refused(u);
            }
            Some(Done::Ready(Err(message))) => self.state = DirectState::Failed(message),
            Some(Done::Lost) => {
                self.state = DirectState::Failed(
                    "the background worker stopped before the import finished".to_owned(),
                );
            }
            None => {}
        }
    }
}
