//! Installation requests (specs/010-plugin-installation/data-model.md, Install request;
//! `contracts/window-commands.md`): the bodies of the window's five installation commands.
//!
//! One request at a time. A request starts only from `install_start` or `update_start`, which the
//! chrome's controls call; the page gets its token only with the dialog's model, and every grant
//! goes through `request_approve` with the choice digest the host last returned for that
//! request, checked again against what the host would install now. Nothing reaches the disk
//! before that approval: a refusal, a cancel, or the window closing keeps nothing.
//!
//! Part of the security core (constitution v2.7.0, Principle IX).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use runtime::flows::Flow;
use runtime::install::{DialogModel, Installer, Prepared, new_token};
use runtime::{PermissionKind, RuntimeError};
use serde::Serialize;
use serde_json::Value;

use crate::widgets::{Installs, Widgets};

pub const BAD_ADDRESS: &str = "bad-address";
pub const REQUEST_PENDING: &str = "request-pending";
pub const ALREADY_INSTALLED: &str = "already-installed";
pub const UNKNOWN_REQUEST: &str = "unknown-request";
pub const BAD_CHOICE: &str = "bad-choice";
pub const UNKNOWN_WIDGET: &str = "unknown-widget";
/// Answered when the app was started without what installing needs (only the tests start it
/// so).
pub const NOT_AVAILABLE: &str = "not-available";

/// The most optional choices a request takes: 32 permissions and 16 connectors.
pub const MAX_OPTIONAL: usize = 48;

/// A runtime error as the page gets it: its kind, and the host's detail, escaped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstallError {
    pub kind: &'static str,
    pub reason: String,
}

impl From<RuntimeError> for InstallError {
    fn from(error: RuntimeError) -> InstallError {
        InstallError {
            kind: error.kind().as_str(),
            reason: error.to_string(),
        }
    }
}

/// The answer to `install_start` and `update_start`.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum Started {
    Request {
        request: String,
        model: Box<DialogModel>,
    },
    None {
        none: bool,
    },
    Refused {
        refused: &'static str,
    },
    Error {
        error: InstallError,
    },
}

/// The answer to `request_judge`.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum Judged {
    Judged { warning: Vec<Flow>, choice: String },
    Refused { refused: &'static str },
}

/// The handle of the widget an approval installed or updated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApprovedWidget {
    pub handle: String,
}

/// The answer to `request_approve`.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum Approved {
    Approved { approved: ApprovedWidget },
    Refused { refused: &'static str },
    Error { error: InstallError },
}

/// The one request there may be.
#[derive(Default)]
pub struct Requests {
    state: Mutex<State>,
}

#[derive(Default)]
enum State {
    #[default]
    Idle,
    /// Fetching and compiling; set `cancel` to stop.
    Running { cancel: Arc<AtomicBool> },
    /// Shown in the dialog, waiting for the person.
    Awaiting(Box<Pending>),
}

struct Pending {
    token: String,
    prepared: Prepared,
    /// The choice digest the host last returned for this request.
    last_choice: String,
}

impl Requests {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Widgets {
    fn installer<'a>(
        &'a self,
        installs: &'a Installs,
        cancel: Option<&'a AtomicBool>,
    ) -> Installer<'a> {
        Installer {
            runtime: &self.runtime,
            source: installs.source.as_ref(),
            compiler: &installs.compiler,
            layout: &installs.layout,
            cancel,
        }
    }

    /// Starts a request, or `None` when one is already running or waiting.
    fn begin(&self) -> Option<Arc<AtomicBool>> {
        let mut state = self.requests.state();
        if !matches!(*state, State::Idle) {
            return None;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        *state = State::Running {
            cancel: Arc::clone(&cancel),
        };
        Some(cancel)
    }

    /// Ends a running request: waiting for the person with `prepared`, or idle.
    fn settle(
        &self,
        cancel: &Arc<AtomicBool>,
        outcome: Result<Option<Prepared>, RuntimeError>,
        installs: &Installs,
    ) -> Started {
        let mut state = self.requests.state();
        let still_ours =
            matches!(&*state, State::Running { cancel: running } if Arc::ptr_eq(running, cancel));
        if !still_ours || cancel.load(Ordering::SeqCst) {
            // The window closed while it ran: the safe choice was taken
            *state = State::Idle;
            return Started::Refused {
                refused: UNKNOWN_REQUEST,
            };
        }
        match outcome {
            Ok(None) => {
                *state = State::Idle;
                Started::None { none: true }
            }
            Ok(Some(prepared)) => {
                let token = match new_token() {
                    Ok(token) => token,
                    Err(error) => {
                        *state = State::Idle;
                        return Started::Error {
                            error: error.into(),
                        };
                    }
                };
                let model = self.installer(installs, None).model(&prepared, &[]);
                *state = State::Awaiting(Box::new(Pending {
                    token: token.clone(),
                    prepared,
                    last_choice: model.choice.clone(),
                }));
                Started::Request {
                    request: token,
                    model: Box::new(model),
                }
            }
            Err(error) => {
                *state = State::Idle;
                Started::Error {
                    error: error.into(),
                }
            }
        }
    }

    /// `install_start`: the latest release of the repository at `address`, fetched, compiled and
    /// checked, and the dialog's model of it.
    pub fn install_start(&self, address: &str) -> Started {
        let Some(installs) = &self.installs else {
            return Started::Refused {
                refused: NOT_AVAILABLE,
            };
        };
        let Some(repository) = Installer::parse_address(address) else {
            return Started::Refused {
                refused: BAD_ADDRESS,
            };
        };
        let id = repository.plugin_id();
        let recorded = installs.layout.installed().get(&id);
        if !matches!(recorded, Ok(None)) {
            return Started::Refused {
                refused: ALREADY_INSTALLED,
            };
        }
        let Some(cancel) = self.begin() else {
            return Started::Refused {
                refused: REQUEST_PENDING,
            };
        };
        let outcome = self
            .installer(installs, Some(&cancel))
            .prepare_install(&repository)
            .map(Some);
        self.settle(&cancel, outcome, installs)
    }

    /// `update_start`: the installed widget `handle`'s newer release, if there is one.
    pub fn update_start(&self, handle: &str) -> Started {
        let Some(installs) = &self.installs else {
            return Started::Refused {
                refused: NOT_AVAILABLE,
            };
        };
        let Some(id) = self.installed_id(handle) else {
            return Started::Refused {
                refused: UNKNOWN_WIDGET,
            };
        };
        let record = match installs.layout.installed().get(&id) {
            Ok(Some(record)) => record,
            Ok(None) => {
                return Started::Refused {
                    refused: UNKNOWN_WIDGET,
                };
            }
            Err(error) => {
                return Started::Error {
                    error: error.into(),
                };
            }
        };
        let Some(cancel) = self.begin() else {
            return Started::Refused {
                refused: REQUEST_PENDING,
            };
        };
        let outcome = self
            .installer(installs, Some(&cancel))
            .prepare_update(&id, &record);
        self.settle(&cancel, outcome, installs)
    }

    /// `request_judge`: the warning and the choice digest for `optional_on`.
    pub fn request_judge(&self, request: &str, optional_on: &Value) -> Judged {
        let Some(installs) = &self.installs else {
            return Judged::Refused {
                refused: NOT_AVAILABLE,
            };
        };
        let mut state = self.requests.state();
        let State::Awaiting(pending) = &mut *state else {
            return Judged::Refused {
                refused: UNKNOWN_REQUEST,
            };
        };
        if pending.token != request {
            return Judged::Refused {
                refused: UNKNOWN_REQUEST,
            };
        }
        let Some(names) = choices(&pending.prepared, optional_on) else {
            return Judged::Refused {
                refused: BAD_CHOICE,
            };
        };
        let model = self
            .installer(installs, None)
            .model(&pending.prepared, &names);
        pending.last_choice = model.choice.clone();
        Judged::Judged {
            warning: model.warning,
            choice: model.choice,
        }
    }

    /// `request_approve`: installs what the dialog showed, when `choice` is the digest the host
    /// last returned for this request and still what it would install now.
    pub fn request_approve(&self, request: &str, choice: &str, optional_on: &Value) -> Approved {
        let Some(installs) = &self.installs else {
            return Approved::Refused {
                refused: NOT_AVAILABLE,
            };
        };
        let mut state = self.requests.state();
        let State::Awaiting(pending) = &*state else {
            return Approved::Refused {
                refused: UNKNOWN_REQUEST,
            };
        };
        if pending.token != request {
            return Approved::Refused {
                refused: UNKNOWN_REQUEST,
            };
        }
        let Some(names) = choices(&pending.prepared, optional_on) else {
            return Approved::Refused {
                refused: BAD_CHOICE,
            };
        };
        if pending.last_choice != choice {
            return Approved::Error {
                error: InstallError {
                    kind: "approval-changed",
                    reason: "the choice approved is not the one the dialog last showed".to_owned(),
                },
            };
        }
        let installer = self.installer(installs, None);
        let installed = match installer.commit(&pending.prepared, choice, &names) {
            Ok(installed) => installed,
            Err(error) => {
                return Approved::Error {
                    error: error.into(),
                };
            }
        };
        let id = pending.prepared.id();
        // The token is spent: the request is over whatever the load says
        *state = State::Idle;
        drop(state);
        let plugin = installer
            .load(&id, &installed, Arc::clone(&installs.grants))
            .map_err(|error| error.kind().as_str().to_owned());
        let handle = self.show_installed(&id, plugin);
        Approved::Approved {
            approved: ApprovedWidget { handle },
        }
    }

    /// `request_cancel`: drops the request; an unknown token is ignored, as cancelling is always
    /// safe.
    pub fn request_cancel(&self, request: &str) {
        let mut state = self.requests.state();
        if matches!(&*state, State::Awaiting(pending) if pending.token == request) {
            *state = State::Idle;
        }
    }

    /// The window closed or the app quits: a running compile is stopped, and a request waiting
    /// for the person is dropped (FR-023).
    pub fn stop_requests(&self) {
        let mut state = self.requests.state();
        match &*state {
            State::Running { cancel } => cancel.store(true, Ordering::SeqCst),
            State::Awaiting(_) => *state = State::Idle,
            State::Idle => {}
        }
    }
}

/// The optionals the page turned on: a list of at most [`MAX_OPTIONAL`] names, each once, each
/// an optional permission or connector the prepared package declares; `None` otherwise.
fn choices(prepared: &Prepared, optional_on: &Value) -> Option<Vec<String>> {
    let names = optional_on.as_array()?;
    if names.len() > MAX_OPTIONAL {
        return None;
    }
    let manifest = prepared.checked.manifest();
    let mut chosen: Vec<String> = Vec::new();
    for name in names {
        let name = name.as_str()?;
        let permission = manifest.permission(name).is_some_and(|permission| {
            !permission.required && PermissionKind::from_name(name).is_some()
        });
        let connector = manifest
            .connector(name)
            .is_some_and(|connector| !connector.required);
        if !(permission || connector) || chosen.iter().any(|seen| seen == name) {
            return None;
        }
        chosen.push(name.to_owned());
    }
    Some(chosen)
}
