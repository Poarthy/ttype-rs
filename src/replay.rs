use std::time::Duration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplayEventKind {
    Rune,
    Backspace,
    DeleteWord,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplayEvent {
    pub offset: Duration,
    pub kind: ReplayEventKind,
    pub character: Option<char>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Replay {
    pub target: String,
    pub events: Vec<ReplayEvent>,
}
