use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

pub trait Clock {
    fn now(&self) -> Duration;
}

#[derive(Clone, Debug)]
pub struct FakeClock {
    now: Rc<Cell<Duration>>,
}

impl FakeClock {
    pub fn new() -> Self {
        Self {
            now: Rc::new(Cell::new(Duration::ZERO)),
        }
    }

    pub fn advance(&self, duration: Duration) {
        self.now.set(self.now.get().saturating_add(duration));
    }

    pub fn set(&self, duration: Duration) {
        self.now.set(duration);
    }
}

impl Default for FakeClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Duration {
        self.now.get()
    }
}

#[derive(Debug)]
pub struct RealClock {
    started: Instant,
}

impl RealClock {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
        }
    }
}

impl Default for RealClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for RealClock {
    fn now(&self) -> Duration {
        self.started.elapsed()
    }
}
