//! The input gate of a host dialog (HS-11, FR-017; data-model "Input gate"): nothing answers the
//! dialog until 1000 ms after the first frame that shows it, a key or pointer button already down
//! when the gate started answers nothing until released and pressed again, and the 1000 ms start
//! over when the window regains focus or the dialog changes.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// How long a new dialog ignores input (feature 007, research R11).
pub const DELAY: Duration = Duration::from_millis(1000);

/// Monotonic time, injectable for tests.
pub trait Clock: Send + Sync {
    fn now(&self) -> Duration;
}

/// The system's monotonic clock.
pub struct SystemClock(Instant);

impl Default for SystemClock {
    fn default() -> Self {
        SystemClock(Instant::now())
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Duration {
        self.0.elapsed()
    }
}

/// A clock tests move by hand.
#[derive(Default)]
pub struct ManualClock(AtomicU64);

impl ManualClock {
    pub fn new() -> Arc<ManualClock> {
        Arc::new(ManualClock::default())
    }

    pub fn advance(&self, by: Duration) {
        self.0.fetch_add(by.as_millis() as u64, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Duration {
        Duration::from_millis(self.0.load(Ordering::SeqCst))
    }
}

/// The gate of the dialog on screen.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Gate {
    started_at: Option<Duration>,
    /// Keys down when the gate started, ignored until released.
    held_keys: HashSet<String>,
    /// Whether a pointer press began after the gate opened, so its click may answer.
    pointer_armed: bool,
}

impl Gate {
    /// Starts (or restarts) the count at `now`, with the keys down at that moment.
    pub fn start(&mut self, now: Duration, pressed: &HashSet<String>) {
        self.started_at = Some(now);
        self.held_keys = pressed.clone();
        self.pointer_armed = false;
    }

    pub fn is_started(&self) -> bool {
        self.started_at.is_some()
    }

    /// Whether the delay has passed.
    pub fn is_open(&self, now: Duration) -> bool {
        self.started_at
            .is_some_and(|started| now.saturating_sub(started) >= DELAY)
    }

    /// Whether a key press may answer: the gate is open, and the key is neither a repeat of one
    /// held down nor one that was down when the gate started.
    pub fn accepts_key(&self, now: Duration, key: &str, is_repeat: bool) -> bool {
        self.is_open(now) && !is_repeat && !self.held_keys.contains(key)
    }

    pub fn key_released(&mut self, key: &str) {
        self.held_keys.remove(key);
    }

    /// A pointer button went down on one of the dialog's controls.
    pub fn pointer_down(&mut self, now: Duration) {
        self.pointer_armed = self.is_open(now);
    }

    /// Whether a click may answer: its press began after the gate opened.
    pub fn accepts_click(&self, now: Duration) -> bool {
        self.is_open(now) && self.pointer_armed
    }
}
