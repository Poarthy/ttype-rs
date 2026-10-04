use std::io::{self, Write};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use thiserror::Error;

use crate::assets::builtin_provider;
use crate::cli::ThemeName;
use crate::clock::{FakeClock, RealClock};
use crate::config::{self, Paths};
use crate::domain::{RunResult, TestConfig, TestKind, TextMode};
use crate::replay::{Replay, ReplayEvent};
use crate::session::{KeystrokeStatus, Session, SessionState};
use crate::stats;
use crate::storage::{self, StatsSummary, StoredResult};
use crate::text::TextProvider;

const RESULTS_KEY_GRACE: Duration = Duration::from_millis(700);
const REPLAY_TICK: Duration = Duration::from_millis(50);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Screen {
    Test,
    Result,
    Help,
    ModePicker,
    Settings,
    LanguagePicker,
    Replay,
}

#[derive(Clone, Debug)]
struct Theme {
    correct: Style,
    incorrect: Style,
    pending: Style,
    cursor: Style,
    title: Style,
    wpm: Style,
    raw: Style,
    accuracy: Style,
    error: Style,
    time: Style,
    help: Style,
    selected: Style,
    border: Style,
}

impl Theme {
    fn from_name(name: &str) -> Self {
        match name {
            "monokai" => Self {
                correct: Style::default().fg(Color::Rgb(166, 226, 46)),
                incorrect: Style::default()
                    .fg(Color::Rgb(249, 38, 114))
                    .bg(Color::Rgb(62, 61, 50)),
                pending: Style::default().fg(Color::Rgb(117, 113, 94)),
                cursor: Style::default()
                    .fg(Color::Rgb(248, 248, 242))
                    .bg(Color::Rgb(73, 72, 62))
                    .add_modifier(Modifier::UNDERLINED),
                title: Style::default()
                    .fg(Color::Rgb(248, 248, 242))
                    .add_modifier(Modifier::BOLD),
                wpm: Style::default()
                    .fg(Color::Rgb(102, 217, 239))
                    .add_modifier(Modifier::BOLD),
                raw: Style::default()
                    .fg(Color::Rgb(174, 129, 255))
                    .add_modifier(Modifier::BOLD),
                accuracy: Style::default()
                    .fg(Color::Rgb(166, 226, 46))
                    .add_modifier(Modifier::BOLD),
                error: Style::default()
                    .fg(Color::Rgb(249, 38, 114))
                    .add_modifier(Modifier::BOLD),
                time: Style::default()
                    .fg(Color::Rgb(230, 219, 116))
                    .add_modifier(Modifier::BOLD),
                help: Style::default().fg(Color::Rgb(117, 113, 94)),
                selected: Style::default()
                    .fg(Color::Rgb(248, 248, 242))
                    .bg(Color::Rgb(73, 72, 62)),
                border: Style::default().fg(Color::Rgb(117, 113, 94)),
            },
            "dracula" => Self {
                correct: Style::default().fg(Color::Rgb(80, 250, 123)),
                incorrect: Style::default()
                    .fg(Color::Rgb(255, 85, 85))
                    .bg(Color::Rgb(68, 71, 90)),
                pending: Style::default().fg(Color::Rgb(98, 114, 164)),
                cursor: Style::default()
                    .fg(Color::Rgb(248, 248, 242))
                    .bg(Color::Rgb(68, 71, 90))
                    .add_modifier(Modifier::UNDERLINED),
                title: Style::default()
                    .fg(Color::Rgb(255, 121, 198))
                    .add_modifier(Modifier::BOLD),
                wpm: Style::default()
                    .fg(Color::Rgb(139, 233, 253))
                    .add_modifier(Modifier::BOLD),
                raw: Style::default()
                    .fg(Color::Rgb(189, 147, 249))
                    .add_modifier(Modifier::BOLD),
                accuracy: Style::default()
                    .fg(Color::Rgb(80, 250, 123))
                    .add_modifier(Modifier::BOLD),
                error: Style::default()
                    .fg(Color::Rgb(255, 85, 85))
                    .add_modifier(Modifier::BOLD),
                time: Style::default()
                    .fg(Color::Rgb(241, 250, 140))
                    .add_modifier(Modifier::BOLD),
                help: Style::default().fg(Color::Rgb(98, 114, 164)),
                selected: Style::default()
                    .fg(Color::Rgb(248, 248, 242))
                    .bg(Color::Rgb(189, 147, 249)),
                border: Style::default().fg(Color::Rgb(98, 114, 164)),
            },
            _ => Self {
                correct: Style::default().fg(Color::Indexed(2)),
                incorrect: Style::default()
                    .fg(Color::Indexed(1))
                    .bg(Color::Indexed(236)),
                pending: Style::default().fg(Color::Indexed(245)),
                cursor: Style::default()
                    .fg(Color::Indexed(15))
                    .bg(Color::Indexed(240))
                    .add_modifier(Modifier::UNDERLINED),
                title: Style::default()
                    .fg(Color::Indexed(15))
                    .add_modifier(Modifier::BOLD),
                wpm: Style::default()
                    .fg(Color::Indexed(6))
                    .add_modifier(Modifier::BOLD),
                raw: Style::default()
                    .fg(Color::Indexed(5))
                    .add_modifier(Modifier::BOLD),
                accuracy: Style::default()
                    .fg(Color::Indexed(2))
                    .add_modifier(Modifier::BOLD),
                error: Style::default()
                    .fg(Color::Indexed(1))
                    .add_modifier(Modifier::BOLD),
                time: Style::default()
                    .fg(Color::Indexed(3))
                    .add_modifier(Modifier::BOLD),
                help: Style::default().fg(Color::Indexed(243)),
                selected: Style::default()
                    .fg(Color::Indexed(229))
                    .bg(Color::Indexed(57)),
                border: Style::default().fg(Color::Indexed(240)),
            },
        }
    }
}

#[derive(Debug, Error)]
pub enum TuiError {
    #[error("terminal I/O: {0}")]
    Io(#[from] io::Error),
    #[error("session: {0}")]
    Session(#[from] crate::session::SessionError),
    #[error("cached language: {0}")]
    Language(#[from] crate::langcache::LanguageError),
    #[error("cached language data directory is unavailable")]
    NoLanguageData,
}

struct ReplayPlayback {
    session: Session,
    clock: FakeClock,
    events: Vec<ReplayEvent>,
    target: String,
    config: TestConfig,
    next: usize,
    elapsed: Duration,
    speed: u8,
    paused: bool,
}

impl ReplayPlayback {
    fn new(config: TestConfig, recording: Replay) -> Result<Self, TuiError> {
        let mut display_config = config;
        display_config.blind = false;
        display_config.zen = false;
        let clock = FakeClock::new();
        let session = Session::new(
            display_config.clone(),
            recording.target.clone(),
            clock.clone(),
        )?;
        Ok(Self {
            session,
            clock,
            events: recording.events,
            target: recording.target,
            config: display_config,
            next: 0,
            elapsed: Duration::ZERO,
            speed: 1,
            paused: false,
        })
    }

    fn advance(&mut self) {
        if self.paused {
            return;
        }
        self.elapsed = self
            .elapsed
            .saturating_add(REPLAY_TICK.saturating_mul(u32::from(self.speed)));
        while self.next < self.events.len() && self.events[self.next].offset <= self.elapsed {
            let event = self.events[self.next].clone();
            self.clock.set(event.offset);
            self.session.apply_event(&event);
            self.next += 1;
        }
        self.clock.set(self.elapsed);
        self.session.tick();
    }

    fn restart(&mut self) -> Result<(), TuiError> {
        self.clock = FakeClock::new();
        self.session = Session::new(self.config.clone(), self.target.clone(), self.clock.clone())?;
        self.next = 0;
        self.elapsed = Duration::ZERO;
        self.paused = false;
        Ok(())
    }

    fn done(&self) -> bool {
        self.next >= self.events.len()
    }
}

/// Single-threaded crossterm event loop for the active typing test.
pub struct TuiApp {
    session: Session,
    provider: TextProvider,
    custom_target: Option<String>,
    custom_word_lines: Option<Vec<usize>>,
    paths: Option<Paths>,
    screen: Screen,
    show_live_stats: bool,
    mode_index: usize,
    settings_index: usize,
    settings_draft: Option<TestConfig>,
    language_index: usize,
    language_ids: Vec<String>,
    language_return: Screen,
    quit_on_finish: bool,
    no_save: bool,
    result_save_attempted: bool,
    finished_at: Option<Instant>,
    replay: Option<ReplayPlayback>,
    notice: String,
    update_messages: Option<Receiver<crate::update::AutoUpdateNotice>>,
    update_install_active: bool,
}

impl TuiApp {
    pub fn new(session: Session) -> Self {
        let mode_index = mode_index(session.config().text_mode);
        Self {
            custom_target: (session.config().text_mode == TextMode::Custom)
                .then(|| session.target().to_owned()),
            custom_word_lines: (session.config().text_mode == TextMode::Custom)
                .then(|| session.word_lines().to_vec()),
            session,
            provider: builtin_provider(),
            paths: None,
            screen: Screen::Test,
            show_live_stats: true,
            mode_index,
            settings_index: 0,
            settings_draft: None,
            language_index: 0,
            language_ids: vec![String::new()],
            language_return: Screen::Settings,
            quit_on_finish: false,
            no_save: false,
            result_save_attempted: false,
            finished_at: None,
            replay: None,
            notice: String::new(),
            update_messages: None,
            update_install_active: false,
        }
    }

    pub fn with_paths(mut self, paths: Paths) -> Self {
        self.language_ids
            .extend(crate::langcache::LanguageCache::new(&paths.data).installed());
        self.paths = Some(paths);
        self
    }
    pub fn with_notice(mut self, notice: String) -> Self {
        self.notice = notice;
        self
    }
    pub fn with_quit_on_finish(mut self, enabled: bool) -> Self {
        self.quit_on_finish = enabled;
        self
    }
    pub fn with_no_save(mut self, enabled: bool) -> Self {
        self.no_save = enabled;
        self
    }
    pub fn with_update_messages(
        mut self,
        receiver: Receiver<crate::update::AutoUpdateNotice>,
    ) -> Self {
        self.update_messages = Some(receiver);
        self
    }
    pub fn session(&self) -> &Session {
        &self.session
    }

    pub fn run(self) -> Result<Session, TuiError> {
        self.run_with_writer(io::stdout())
    }

    pub fn run_with_writer<W: Write>(mut self, mut output: W) -> Result<Session, TuiError> {
        enable_raw_mode()?;
        if let Err(error) = execute!(output, EnterAlternateScreen) {
            let _ = disable_raw_mode();
            return Err(TuiError::Io(error));
        }
        let mut terminal = Terminal::new(CrosstermBackend::new(output))?;
        let run_result = self.run_loop(&mut terminal);
        let restore_result = restore_terminal(&mut terminal);
        self.wait_for_update();
        match (run_result, restore_result) {
            (Err(error), _) => Err(error),
            (Ok(()), Err(error)) => Err(error),
            (Ok(()), Ok(())) => Ok(self.session),
        }
    }

    fn run_loop<W: Write>(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<W>>,
    ) -> Result<(), TuiError> {
        loop {
            terminal.draw(|frame| self.draw(frame))?;
            if event::poll(Duration::from_millis(50))?
                && let Event::Key(key) = event::read()?
                && key.kind != KeyEventKind::Release
                && self.handle_key(key)?
            {
                return Ok(());
            }
            if self.tick() {
                return Ok(());
            }
        }
    }

    fn tick(&mut self) -> bool {
        self.poll_update_messages();
        match self.screen {
            Screen::Test => {
                self.session.tick();
                if self.session.state() == SessionState::Finished {
                    self.show_results();
                    return self.quit_on_finish;
                }
            }
            Screen::Replay => {
                if let Some(replay) = &mut self.replay {
                    replay.advance();
                }
            }
            _ => {}
        }
        false
    }

    fn poll_update_messages(&mut self) {
        let Some(receiver) = &self.update_messages else {
            return;
        };
        match receiver.try_recv() {
            Ok(crate::update::AutoUpdateNotice::StartingInstall(_)) => {
                self.update_install_active = true;
            }
            Ok(crate::update::AutoUpdateNotice::Available(version)) => {
                self.notice = format!("ttype {version} is available; run ttype update");
                self.update_install_active = false;
                self.update_messages = None;
            }
            Ok(crate::update::AutoUpdateNotice::Installed(version)) => {
                self.notice = format!("ttype {version} is installed and runs from the next launch");
                self.update_install_active = false;
                self.update_messages = None;
            }
            Err(TryRecvError::Disconnected) => {
                self.update_install_active = false;
                self.update_messages = None;
            }
            Err(TryRecvError::Empty) => {}
        }
    }

    fn wait_for_update(&mut self) {
        let Some(receiver) = self.update_messages.take() else {
            return;
        };
        let mut install_active = self.update_install_active;
        while let Ok(notice) = receiver.try_recv() {
            install_active = matches!(notice, crate::update::AutoUpdateNotice::StartingInstall(_));
        }
        if install_active {
            let _ = receiver.recv_timeout(Duration::from_secs(30));
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<bool, TuiError> {
        match self.screen {
            Screen::Test => self.handle_test_key(key),
            Screen::Result => self.handle_result_key(key),
            Screen::Help => Ok(self.handle_help_key(key)),
            Screen::ModePicker => self.handle_mode_key(key),
            Screen::Settings => self.handle_settings_key(key),
            Screen::LanguagePicker => self.handle_language_key(key),
            Screen::Replay => self.handle_replay_key(key),
        }
    }

    fn handle_test_key(&mut self, key: KeyEvent) -> Result<bool, TuiError> {
        if matches!(key.code, KeyCode::Esc) || ctrl(key, 'c') {
            return Ok(true);
        }
        if ctrl(key, 's') {
            self.open_settings();
            return Ok(false);
        }
        if ctrl(key, 'o') {
            self.show_live_stats = !self.show_live_stats;
            return Ok(false);
        }
        if key.code == KeyCode::Char('?') && self.session.state() == SessionState::Ready {
            self.screen = Screen::Help;
            return Ok(false);
        }
        match key.code {
            KeyCode::Backspace => {
                self.session.backspace();
            }
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.session.delete_word();
            }
            KeyCode::Tab if self.session.config().text_mode.commits_words_on_space() => {
                self.restart_current()?;
            }
            KeyCode::Enter if self.session.config().text_mode.commits_words_on_space() => {
                self.restart_current()?;
            }
            KeyCode::Tab => self.session.input_char('\t'),
            KeyCode::Enter => self.session.input_char('\n'),
            KeyCode::Char(character)
                if !key
                    .modifiers
                    .contains(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                self.session.input_char(character)
            }
            _ => {}
        }
        if self.session.state() == SessionState::Finished {
            self.show_results();
            return Ok(self.quit_on_finish);
        }
        Ok(false)
    }

    fn handle_result_key(&mut self, key: KeyEvent) -> Result<bool, TuiError> {
        if self
            .finished_at
            .is_some_and(|finished| finished.elapsed() < RESULTS_KEY_GRACE)
        {
            return Ok(false);
        }
        if matches!(
            key.code,
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q')
        ) || ctrl(key, 'c')
        {
            return Ok(true);
        }
        match key.code {
            KeyCode::Tab | KeyCode::Enter | KeyCode::Char('r') | KeyCode::Char('R') => {
                self.restart_current()?
            }
            KeyCode::Char('p') | KeyCode::Char('P') => {
                self.replay = Some(ReplayPlayback::new(
                    self.session.config().clone(),
                    Replay {
                        target: self.session.target().to_owned(),
                        events: self.session.events().to_vec(),
                    },
                )?);
                self.screen = Screen::Replay;
            }
            KeyCode::Char('S') | KeyCode::Char('s')
                if ctrl(key, 's') || matches!(key.code, KeyCode::Char('S')) =>
            {
                self.open_settings()
            }
            KeyCode::Char('M') => self.open_mode_picker(),
            KeyCode::Char('L') => self.open_language_picker(Screen::Result),
            _ => {}
        }
        Ok(false)
    }

    fn handle_help_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => self.screen = Screen::Test,
            KeyCode::Char('S') => self.open_settings(),
            KeyCode::Char('M') => self.open_mode_picker(),
            _ => {}
        }
        false
    }

    fn handle_mode_key(&mut self, key: KeyEvent) -> Result<bool, TuiError> {
        let modes = TextMode::all();
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.screen = if self.session.state() == SessionState::Finished {
                    Screen::Result
                } else {
                    Screen::Test
                }
            }
            KeyCode::Up | KeyCode::Left | KeyCode::Char('k') | KeyCode::Char('h') => {
                self.mode_index = (self.mode_index + modes.len() - 1) % modes.len()
            }
            KeyCode::Down | KeyCode::Right | KeyCode::Char('j') | KeyCode::Char('l') => {
                self.mode_index = (self.mode_index + 1) % modes.len()
            }
            KeyCode::Enter => {
                let mut config = self.session.config().clone();
                config.text_mode = modes[self.mode_index];
                self.restart_with_config(config)?;
                self.persist_defaults();
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_settings_key(&mut self, key: KeyEvent) -> Result<bool, TuiError> {
        let last = SETTINGS_LABELS.len() - 1;
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.settings_draft = None;
                self.screen = if self.session.state() == SessionState::Finished {
                    Screen::Result
                } else {
                    Screen::Test
                };
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.settings_index = self.settings_index.saturating_sub(1)
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.settings_index = (self.settings_index + 1).min(last)
            }
            KeyCode::Left | KeyCode::Char('h') => self.adjust_setting(-1),
            KeyCode::Right | KeyCode::Char('l') => self.adjust_setting(1),
            KeyCode::Enter => {
                if self.settings_index == 3 {
                    self.open_language_picker(Screen::Settings);
                } else if let Some(config) = self.settings_draft.take() {
                    self.restart_with_config(config)?;
                    self.persist_defaults();
                }
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_language_key(&mut self, key: KeyEvent) -> Result<bool, TuiError> {
        let count = self.language_ids.len();
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.screen = self.language_return
            }
            KeyCode::Enter => {
                let selected = self
                    .language_ids
                    .get(self.language_index)
                    .cloned()
                    .unwrap_or_default();
                if self.language_return == Screen::Settings {
                    if let Some(draft) = &mut self.settings_draft {
                        draft.language = selected;
                    }
                    self.screen = Screen::Settings;
                } else {
                    let mut next = self.session.config().clone();
                    next.language = selected;
                    if let Err(error) = self.restart_with_config(next) {
                        self.notice = format!("couldn't change language: {error}");
                        self.screen = self.language_return;
                    }
                }
            }
            KeyCode::Up | KeyCode::Char('k') if count > 0 => {
                self.language_index = (self.language_index + count - 1) % count
            }
            KeyCode::Down | KeyCode::Char('j') if count > 0 => {
                self.language_index = (self.language_index + 1) % count
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_replay_key(&mut self, key: KeyEvent) -> Result<bool, TuiError> {
        if matches!(
            key.code,
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q')
        ) {
            self.replay = None;
            self.screen = Screen::Result;
            return Ok(false);
        }
        if let Some(replay) = &mut self.replay {
            match key.code {
                KeyCode::Char(' ') => replay.paused = !replay.paused,
                KeyCode::Char('1') => replay.speed = 1,
                KeyCode::Char('2') => replay.speed = 2,
                KeyCode::Char('4') => replay.speed = 4,
                KeyCode::Char('r') | KeyCode::Char('R') => replay.restart()?,
                _ => {}
            }
        }
        Ok(false)
    }

    fn open_mode_picker(&mut self) {
        self.mode_index = mode_index(self.session.config().text_mode);
        self.screen = Screen::ModePicker;
    }
    fn open_settings(&mut self) {
        self.settings_draft = Some(self.session.config().clone());
        self.settings_index = 0;
        self.screen = Screen::Settings;
    }

    fn open_language_picker(&mut self, return_to: Screen) {
        let current = self.settings_draft.as_ref().map_or_else(
            || self.session.config().language.as_str(),
            |config| config.language.as_str(),
        );
        self.language_index = self
            .language_ids
            .iter()
            .position(|id| id == current)
            .unwrap_or(0);
        self.language_return = return_to;
        self.screen = Screen::LanguagePicker;
    }

    fn adjust_setting(&mut self, delta: i32) {
        let Some(draft) = &mut self.settings_draft else {
            return;
        };
        match self.settings_index {
            0 => {
                draft.kind = if draft.kind == TestKind::Words {
                    TestKind::Timed
                } else {
                    TestKind::Words
                };
                if draft.kind == TestKind::Words && draft.word_count == 0 {
                    draft.word_count = 25;
                }
            }
            1 if draft.kind == TestKind::Words => {
                draft.word_count = (draft.word_count as i32 + delta * 5).clamp(10, 1000) as usize;
            }
            1 => {
                let seconds = draft.duration.as_secs() as i32 + delta * 15;
                draft.duration = Duration::from_secs(seconds.max(15) as u64);
            }
            2 => {
                let modes = TextMode::all();
                let current = mode_index(draft.text_mode) as i32;
                let next = (current + delta).rem_euclid(modes.len() as i32) as usize;
                draft.text_mode = modes[next];
            }
            4 => {
                if draft.width == 0 && delta > 0 {
                    draft.width = 60;
                } else {
                    draft.width = (draft.width as i32 + delta * 10).clamp(0, 100) as usize;
                }
            }
            5 => {
                let names = ["default", "monokai", "dracula"];
                let index = names
                    .iter()
                    .position(|name| *name == draft.theme)
                    .unwrap_or(0) as i32;
                let next = (index + delta).rem_euclid(names.len() as i32) as usize;
                draft.theme = names[next].to_owned();
            }
            6 => draft.punctuation = !draft.punctuation,
            7 => draft.numbers = !draft.numbers,
            8 => draft.blind = !draft.blind,
            9 => draft.zen = !draft.zen,
            10 => draft.min_wpm = (draft.min_wpm + delta * 5).clamp(0, 300),
            _ => {}
        }
    }

    fn restart_current(&mut self) -> Result<(), TuiError> {
        self.session.restart()?;
        self.screen = Screen::Test;
        self.result_save_attempted = false;
        self.finished_at = None;
        Ok(())
    }

    fn restart_with_config(&mut self, config: TestConfig) -> Result<(), TuiError> {
        let source: Box<dyn crate::text::TextSource> = if config.text_mode == TextMode::Custom {
            Box::new(crate::text::StaticText::with_word_lines(
                self.custom_target.clone().unwrap_or_default(),
                self.custom_word_lines.clone().unwrap_or_default(),
            ))
        } else {
            Box::new(self.provider_for_config(&config)?)
        };
        self.session = Session::from_source(config, source, RealClock::new())?;
        self.screen = Screen::Test;
        self.result_save_attempted = false;
        self.finished_at = None;
        self.notice.clear();
        Ok(())
    }

    fn provider_for_config(&self, config: &TestConfig) -> Result<TextProvider, TuiError> {
        if config.text_mode != TextMode::Words || config.language.is_empty() {
            return Ok(self.provider.clone());
        }
        let paths = self.paths.as_ref().ok_or(TuiError::NoLanguageData)?;
        let words = crate::langcache::LanguageCache::new(&paths.data).words(&config.language)?;
        Ok(self
            .provider
            .clone()
            .with_mode_items(TextMode::Words, words))
    }

    fn persist_defaults(&mut self) {
        let Some(paths) = &self.paths else {
            return;
        };
        match config::load(paths) {
            Ok(mut settings) => {
                let current = self.session.config();
                settings.default_duration = current.duration.as_secs();
                settings.default_word_count = if current.is_words_mode() {
                    current.word_count
                } else {
                    0
                };
                settings.default_width = current.width;
                settings.theme = current.theme.clone();
                settings.language = current.language.clone();
                settings.default_mode = current.text_mode;
                settings.punctuation = current.punctuation;
                settings.numbers = current.numbers;
                settings.blind = current.blind;
                settings.zen = current.zen;
                settings.default_min_wpm = current.min_wpm;
                if let Err(error) = config::save(paths, &settings) {
                    self.notice = format!("settings not saved: {error}");
                }
            }
            Err(error) => self.notice = format!("settings not saved: {error}"),
        }
    }

    fn show_results(&mut self) {
        self.persist_completed_result();
        self.screen = Screen::Result;
        self.finished_at = Some(Instant::now());
    }

    // Go writes history and the replay as soon as a session finishes, before
    // the user dismisses the result view. A failed replay write does not undo
    // a saved result because playback is optional there as well.
    fn persist_completed_result(&mut self) {
        if self.result_save_attempted {
            return;
        }
        self.result_save_attempted = true;
        if self.no_save {
            self.notice = "not saved".to_owned();
            return;
        }
        let Some(paths) = self.paths.clone() else {
            return;
        };
        let result = match self.session.result() {
            Ok(result) => result,
            Err(error) => {
                self.notice = format!("result not saved: {error}");
                return;
            }
        };
        let id = match storage::save_result(&paths, &result) {
            Ok(id) => id,
            Err(error) => {
                self.notice = format!("result not saved: {error}");
                return;
            }
        };
        if !self.session.events().is_empty() {
            let replay = Replay {
                target: self.session.target().to_owned(),
                events: self.session.events().to_vec(),
            };
            let _ = storage::save_replay(&paths, &id, &replay);
        }
    }

    fn draw(&self, frame: &mut Frame) {
        let area = frame.area();
        frame.render_widget(Clear, area);
        match self.screen {
            Screen::Test => draw_test(
                frame,
                area,
                &self.session,
                &Theme::from_name(&self.session.config().theme),
                self.show_live_stats,
                &self.notice,
            ),
            Screen::Result => draw_result(
                frame,
                area,
                self.session.result().ok().as_ref(),
                &Theme::from_name(&self.session.config().theme),
                &self.notice,
            ),
            Screen::Help => draw_overlay(
                frame,
                area,
                "Help",
                help_text(),
                &Theme::from_name(&self.session.config().theme),
            ),
            Screen::ModePicker => draw_mode_picker(
                frame,
                area,
                self.mode_index,
                &Theme::from_name(&self.session.config().theme),
            ),
            Screen::Settings => draw_settings(
                frame,
                area,
                self.settings_draft.as_ref(),
                self.settings_index,
                &Theme::from_name(&self.session.config().theme),
            ),
            Screen::LanguagePicker => draw_language_picker(
                frame,
                area,
                &self.language_ids,
                self.language_index,
                &Theme::from_name(&self.session.config().theme),
            ),
            Screen::Replay => {
                if let Some(replay) = &self.replay {
                    draw_replay(
                        frame,
                        area,
                        replay,
                        &Theme::from_name(&self.session.config().theme),
                    );
                }
            }
        }
    }
}

const SETTINGS_LABELS: [&str; 11] = [
    "test", "length", "mode", "language", "width", "theme", "punct", "numbers", "blind", "zen",
    "min wpm",
];

pub fn blind_reveal_end(target: &[char], cursor: usize) -> usize {
    let mut start = cursor.min(target.len());
    while start > 0 && target[start - 1] != ' ' {
        start -= 1;
    }
    start
}

fn draw_test(
    frame: &mut Frame,
    area: Rect,
    session: &Session,
    theme: &Theme,
    show_live: bool,
    notice: &str,
) {
    let mut lines = Vec::new();
    if !session.config().zen {
        lines.push(hud_line(session, theme, show_live));
        lines.push(Line::default());
    }
    lines.extend(render_target(session, theme));
    if !session.config().zen {
        lines.push(Line::default());
        let hint = if !notice.is_empty() {
            notice.to_owned()
        } else if session.state() == SessionState::Ready {
            "start typing to begin".to_owned()
        } else {
            String::new()
        };
        lines.push(Line::styled(hint, theme.help));
        lines.push(Line::styled("? help  tab/enter restart  esc/ctrl+c quit  ctrl+backspace/ctrl+w delete word  ctrl+o live stats", theme.help));
    }
    let width = typing_width(area, session.config());
    let text = Text::from(lines);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border)
        .title(Span::styled("ttype", theme.title));
    let centered = centered_rect(width, area.height.saturating_sub(2).max(3), area);
    frame.render_widget(
        Paragraph::new(text).block(block).wrap(Wrap { trim: false }),
        centered,
    );
}

fn hud_line(session: &Session, theme: &Theme, show_live: bool) -> Line<'static> {
    let mut spans = Vec::new();
    let (wpm, raw_wpm) = hud_pace(session);
    if show_live && session.config().blind {
        spans.push(Span::styled("raw ", theme.help));
        spans.push(Span::styled(hud_number(raw_wpm), theme.raw));
    } else if show_live {
        spans.push(Span::styled("wpm ", theme.help));
        spans.push(Span::styled(hud_number(wpm), theme.wpm));
        spans.push(Span::styled("  raw ", theme.help));
        spans.push(Span::styled(hud_number(raw_wpm), theme.raw));
        spans.push(Span::styled("  acc ", theme.help));
        spans.push(Span::styled(
            format!("{:.0}%", session.live_accuracy()),
            theme.accuracy,
        ));
        spans.push(Span::styled("  err ", theme.help));
        spans.push(Span::styled(
            session.keystrokes().1.to_string(),
            theme.error,
        ));
    }
    spans.push(Span::styled("  ", theme.help));
    let timing = if session.config().is_words_mode() {
        let (done, total) = session.words_progress();
        format!("{done}/{total} · {}", format_clock(session.elapsed()))
    } else {
        format_clock(session.remaining())
    };
    spans.push(Span::styled(timing, theme.time));
    Line::from(spans)
}

/// The HUD does not extrapolate a just-started run. The Go reference rescales
/// its live WPM values until one full second has elapsed before display.
pub fn hud_pace(session: &Session) -> (f64, f64) {
    let elapsed = session.elapsed();
    if elapsed.is_zero() || elapsed >= Duration::from_secs(1) {
        (session.live_wpm(), session.live_raw_wpm())
    } else {
        let scale = elapsed.as_secs_f64();
        (session.live_wpm() * scale, session.live_raw_wpm() * scale)
    }
}

fn hud_number(value: f64) -> String {
    format!("{:.0}", value.clamp(0.0, 999.0))
}

fn render_target(session: &Session, theme: &Theme) -> Vec<Line<'static>> {
    let target = session.target_chars();
    let cursor = session.cursor();
    let revealed = if session.config().blind {
        blind_reveal_end(target, cursor)
    } else {
        cursor
    };
    let mut lines = vec![Line::default()];
    for (index, expected) in target.iter().enumerate() {
        if *expected == '\n' {
            lines.push(Line::default());
            continue;
        }
        let hidden = session.config().blind && index < cursor && index >= revealed;
        let character = if hidden {
            "·".to_owned()
        } else {
            expected.to_string()
        };
        let style = if index == cursor {
            theme.cursor
        } else if index < cursor && !hidden {
            match session.status_at(index) {
                KeystrokeStatus::Correct => theme.correct,
                KeystrokeStatus::Incorrect => theme.incorrect,
                KeystrokeStatus::Pending => theme.pending,
            }
        } else {
            theme.pending
        };
        if let Some(line) = lines.last_mut() {
            line.spans.push(Span::styled(character, style));
        }
        if !hidden {
            for extra in session.extras_at(index) {
                if let Some(line) = lines.last_mut() {
                    line.spans
                        .push(Span::styled(extra.to_string(), theme.incorrect));
                }
            }
        }
    }
    if cursor >= target.len()
        && let Some(line) = lines.last_mut()
    {
        line.spans.push(Span::styled(" ", theme.cursor));
    }
    lines
}

pub fn render_result_text(result: &RunResult, width: u16) -> String {
    let theme = Theme::from_name(&result.config.theme);
    let mut output = vec![if result.failed {
        "Test Failed".to_owned()
    } else {
        "Test Complete".to_owned()
    }];
    output.push(if result.config.is_words_mode() {
        format!(
            "{} words · {}",
            result.config.word_count, result.config.text_mode
        )
    } else {
        format!(
            "{}s · {}",
            result.config.duration.as_secs(),
            result.config.text_mode
        )
    });
    output.push(format!(
        "\nWPM {:.0}    ACC {:.0}%",
        result.wpm, result.accuracy
    ));
    output.push(format!(
        "raw {:.0} · consistency {:.0}% · errors {} · chars {}/{}/{}/{} · time {}",
        result.raw_wpm,
        result.consistency,
        result.keystrokes_incorrect.max(result.incorrect),
        result.correct,
        result.incorrect,
        (result.total_chars - result.correct - result.incorrect).max(0),
        result.skipped,
        format_clock(result.duration)
    ));
    let chart = render_chart(
        &result.wpm_history,
        &result.raw_wpm_history,
        &result.error_history,
        width.saturating_sub(4) as usize,
    );
    if !chart.is_empty() {
        output.push(format!("\n{chart}"));
    }
    let heatmap = render_heatmap(&result.char_errors, 8);
    if !heatmap.is_empty() {
        output.push(format!("missed {heatmap}"));
    }
    let _ = theme;
    output.join("\n")
}

fn draw_result(
    frame: &mut Frame,
    area: Rect,
    result: Option<&RunResult>,
    theme: &Theme,
    notice: &str,
) {
    let mut lines = Vec::new();
    if let Some(result) = result {
        lines.push(Line::styled(
            if result.failed {
                "Test Failed"
            } else {
                "Test Complete"
            },
            if result.failed {
                theme.error
            } else {
                theme.title
            },
        ));
        let subtitle = if result.config.is_words_mode() {
            format!(
                "{} words · {}",
                result.config.word_count, result.config.text_mode
            )
        } else {
            format!(
                "{}s · {}",
                result.config.duration.as_secs(),
                result.config.text_mode
            )
        };
        lines.push(Line::styled(subtitle, theme.help));
        if result.failed && !result.failure_reason.is_empty() {
            lines.push(Line::styled(result.failure_reason.clone(), theme.error));
        }
        lines.push(Line::default());
        lines.push(Line::from(vec![
            Span::styled("WPM ", theme.help),
            Span::styled(format!("{:.0}", result.wpm), theme.wpm),
            Span::styled("     ACC ", theme.help),
            Span::styled(format!("{:.0}%", result.accuracy), theme.accuracy),
        ]));
        lines.push(Line::from(vec![
            Span::styled("raw ", theme.help),
            Span::styled(format!("{:.0}", result.raw_wpm), theme.raw),
            Span::styled(" · consistency ", theme.help),
            Span::styled(format!("{:.0}%", result.consistency), theme.accuracy),
            Span::styled(" · errors ", theme.help),
            Span::styled(
                result
                    .keystrokes_incorrect
                    .max(result.incorrect)
                    .to_string(),
                theme.error,
            ),
        ]));
        lines.push(Line::styled(
            format!(
                "chars {}/{}/{}/{} · time {}",
                result.correct,
                result.incorrect,
                (result.total_chars - result.correct - result.incorrect).max(0),
                result.skipped,
                format_clock(result.duration)
            ),
            theme.help,
        ));
        let chart = render_chart(
            &result.wpm_history,
            &result.raw_wpm_history,
            &result.error_history,
            area.width.saturating_sub(10) as usize,
        );
        if !chart.is_empty() {
            lines.push(Line::default());
            lines.extend(
                chart
                    .lines()
                    .map(|line| Line::styled(line.to_owned(), theme.wpm)),
            );
        }
        let heatmap = render_heatmap(&result.char_errors, 8);
        if !heatmap.is_empty() {
            lines.push(Line::default());
            lines.push(Line::from(vec![
                Span::styled("missed ", theme.title),
                Span::styled(heatmap, theme.help),
            ]));
        }
    } else {
        lines.push(Line::styled("Result unavailable", theme.error));
    }
    if !notice.is_empty() {
        lines.push(Line::default());
        lines.push(Line::styled(notice.to_owned(), theme.error));
    }
    lines.push(Line::default());
    lines.push(Line::styled(
        "tab/enter/r restart  p replay  ctrl+c/esc/q quit  S settings  M mode  L language",
        theme.help,
    ));
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border)
        .title(Span::styled("ttype", theme.title));
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .block(block)
            .wrap(Wrap { trim: false }),
        centered_rect(
            area.width.min(90),
            area.height.saturating_sub(2).max(3),
            area,
        ),
    );
}

const MIN_BRAILLE_CHART_WIDTH: usize = 40;
const MIN_BRAILLE_CHART_HEIGHT: usize = 5;
const MIN_CHART_HEIGHT: usize = 2;
const BRAILLE_BITS: [[u8; 2]; 4] = [[0x01, 0x08], [0x02, 0x10], [0x04, 0x20], [0x40, 0x80]];

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum ChartSeries {
    None,
    Raw,
    Net,
    Error,
}

struct BraillePlot {
    dots: Vec<Vec<u8>>,
    series: Vec<Vec<ChartSeries>>,
    cols: usize,
    rows: usize,
    scale: f64,
}

impl BraillePlot {
    fn new(cols: usize, rows: usize, scale: f64) -> Self {
        Self {
            dots: vec![vec![0; cols]; rows],
            series: vec![vec![ChartSeries::None; cols]; rows],
            cols,
            rows,
            scale,
        }
    }

    fn pixel_width(&self) -> usize {
        self.cols * 2
    }

    fn pixel_height(&self) -> usize {
        self.rows * 4
    }

    fn y(&self, value: f64) -> usize {
        let value = value.max(0.0);
        ((value / self.scale * (self.pixel_height() - 1) as f64).round() as usize)
            .min(self.pixel_height() - 1)
    }

    fn set(&mut self, pixel_x: usize, pixel_y: usize, series: ChartSeries) {
        let from_top = self.pixel_height() - 1 - pixel_y;
        let row = from_top / 4;
        let column = pixel_x / 2;
        self.dots[row][column] |= BRAILLE_BITS[from_top % 4][pixel_x % 2];
        if series > self.series[row][column] {
            self.series[row][column] = series;
        }
    }

    fn line(&mut self, values: &[f64], series: ChartSeries) {
        let mut previous = None;
        for pixel_x in 0..self.pixel_width() {
            let current = self.y(sample_at(values, pixel_x, self.pixel_width()));
            let (low, high) = previous.map_or((current, current), |last: usize| {
                (last.min(current), last.max(current))
            });
            for pixel_y in low..=high {
                self.set(pixel_x, pixel_y, series);
            }
            previous = Some(current);
        }
    }

    fn band(&mut self, values: &[f64], series: ChartSeries) {
        for pixel_x in 0..self.pixel_width() {
            let top = self.y(sample_at(values, pixel_x, self.pixel_width()));
            for pixel_y in 0..=top {
                self.set(pixel_x, pixel_y, series);
            }
        }
    }

    fn mark_errors(&mut self, history: &[f64], raw_history: &[f64], error_history: &[i32]) {
        if error_history.len() < 2 {
            return;
        }
        for (index, count) in error_history.iter().enumerate() {
            if *count <= 0 {
                continue;
            }
            let Some(value) = raw_history
                .get(index)
                .or_else(|| history.get(index))
                .copied()
            else {
                continue;
            };
            let pixel_x = index * (self.pixel_width() - 1) / (error_history.len() - 1);
            let row = (self.pixel_height() - 1 - self.y(value)) / 4;
            self.series[row][pixel_x / 2] = ChartSeries::Error;
        }
    }

    fn render_row(&self, row: usize) -> String {
        self.dots[row]
            .iter()
            .enumerate()
            .map(|(column, dots)| match self.series[row][column] {
                ChartSeries::Error => '×',
                _ if *dots == 0 => ' ',
                _ => char::from_u32(0x2800 + u32::from(*dots)).unwrap_or(' '),
            })
            .collect()
    }
}

/// Render the reference result chart shape without terminal styling. The
/// thresholds, interpolation, and braille-dot mapping match Go's chart.go.
pub fn render_result_chart(
    history: &[f64],
    raw_history: &[f64],
    error_history: &[i32],
    width: usize,
    height: usize,
    unicode_capable: bool,
) -> String {
    if history.len() < 2 || height < MIN_CHART_HEIGHT || width < 8 {
        return String::new();
    }
    let peak = history.iter().copied().fold(0.0, f64::max);
    let header = format!("wpm over time · peak {:.0}", peak);
    if width < MIN_BRAILLE_CHART_WIDTH || height < MIN_BRAILLE_CHART_HEIGHT || !unicode_capable {
        return format!(
            "{header}\n{}",
            render_sparkline(&downsample_series(history, width))
        );
    }
    format!(
        "{header}\n{}",
        render_braille_chart(history, raw_history, error_history, width, height - 1)
    )
}

pub fn render_chart(history: &[f64], raw_history: &[f64], errors: &[i32], width: usize) -> String {
    render_result_chart(history, raw_history, errors, width, 6, unicode_capable())
}

fn render_braille_chart(
    history: &[f64],
    raw_history: &[f64],
    error_history: &[i32],
    width: usize,
    height: usize,
) -> String {
    let maximum = history
        .iter()
        .chain(raw_history)
        .copied()
        .fold(0.0, f64::max);
    let scale = (maximum / 10.0).ceil().mul_add(10.0, 0.0).max(10.0);
    let top_label = format!("{scale:.0}");
    let gutter = top_label.len();
    let mut plot = BraillePlot::new(width - gutter - 1, height - 1, scale);
    if raw_history.len() > 1 {
        plot.band(raw_history, ChartSeries::Raw);
    }
    plot.line(history, ChartSeries::Net);
    plot.mark_errors(history, raw_history, error_history);

    let mut output = String::new();
    for row in 0..plot.rows {
        let label = if row == 0 {
            top_label.as_str()
        } else if row == plot.rows - 1 {
            "0"
        } else if row == plot.rows / 2 {
            &format!("{:.0}", scale / 2.0)
        } else {
            ""
        };
        if label.is_empty() {
            output.push_str(&" ".repeat(gutter));
            output.push('│');
        } else {
            output.push_str(&format!("{label:>gutter$}┤"));
        }
        output.push_str(&plot.render_row(row));
        if row + 1 < plot.rows {
            output.push('\n');
        }
    }
    output.push('\n');
    output.push_str(&chart_x_axis(history.len() - 1, gutter, plot.cols));
    output
}

fn sample_at(values: &[f64], pixel_x: usize, pixel_width: usize) -> f64 {
    let position = pixel_x as f64 * (values.len() - 1) as f64 / (pixel_width - 1) as f64;
    let index = position as usize;
    if index >= values.len() - 1 {
        return values[values.len() - 1];
    }
    let fraction = position - index as f64;
    values[index] * (1.0 - fraction) + values[index + 1] * fraction
}

fn chart_x_axis(seconds: usize, gutter: usize, columns: usize) -> String {
    let left = "0s";
    let right = format!("{seconds}s");
    let dashes = columns.saturating_sub(left.len() + right.len());
    format!(
        "{}└{left}{}{}",
        " ".repeat(gutter),
        "─".repeat(dashes),
        right
    )
}

fn render_sparkline(values: &[f64]) -> String {
    const CHARS: &[u8] = b" .:-=+#";
    let maximum = values.iter().copied().fold(1.0, f64::max);
    values
        .iter()
        .map(|value| {
            let index =
                ((*value / maximum * (CHARS.len() - 1) as f64) as usize).min(CHARS.len() - 1);
            CHARS[index] as char
        })
        .collect()
}

fn downsample_series(values: &[f64], maximum: usize) -> Vec<f64> {
    if maximum < 1 || values.len() <= maximum {
        return values.to_vec();
    }
    if maximum == 1 {
        return values[..1].to_vec();
    }
    (0..maximum)
        .map(|index| {
            let point = index as f64 * (values.len() - 1) as f64 / (maximum - 1) as f64;
            values[point.round() as usize]
        })
        .collect()
}

fn unicode_capable() -> bool {
    for key in ["LC_ALL", "LC_CTYPE", "LANG"] {
        let Ok(value) = std::env::var(key) else {
            continue;
        };
        if value.is_empty() {
            continue;
        }
        let value = value.to_ascii_uppercase();
        return value.contains("UTF-8") || value.contains("UTF8");
    }
    true
}

pub fn render_heatmap(errors: &std::collections::BTreeMap<String, i32>, limit: usize) -> String {
    stats::top_char_errors(errors, limit)
        .into_iter()
        .map(|entry| {
            let character = if entry.character == " " {
                "space"
            } else {
                &entry.character
            };
            format!("{character}×{}", entry.count)
        })
        .collect::<Vec<_>>()
        .join("  ")
}

pub fn render_stats_text(summary: &StatsSummary, trend: &[f64], width: usize) -> String {
    let mut filters = Vec::new();
    let mut titles = Vec::new();
    if !summary.filter_mode.is_empty() {
        filters.push(format!("mode \"{}\"", summary.filter_mode));
        titles.push(format!("Mode: {}", summary.filter_mode));
    }
    if !summary.filter_tag.is_empty() {
        filters.push(format!("tag \"{}\"", summary.filter_tag));
        titles.push(format!("Tag: {}", summary.filter_tag));
    }
    if summary.total_tests == 0 {
        return if filters.is_empty() {
            "No test history yet.\n".to_owned()
        } else {
            format!("No tests recorded for {}.\n", filters.join(" and "))
        };
    }

    let title = if titles.is_empty() {
        "All tests".to_owned()
    } else {
        titles.join(" · ")
    };
    let mut lines = vec![title, String::new()];
    for (label, value) in [
        ("tests", summary.total_tests.to_string()),
        ("avg wpm", format!("{:.2}", summary.average_wpm)),
        ("avg acc", format!("{:.2}%", summary.average_accuracy)),
        ("best wpm", format!("{:.2}", summary.best_wpm)),
        ("last 10", format!("{:.2}", summary.recent_average_wpm)),
    ] {
        lines.push(format!("  {label:<9}{value}"));
    }
    if filters.is_empty() && summary.personal_best.best_wpm > 0.0 {
        lines.push(String::new());
        lines.push(format!(
            "  all-time  {:.2} wpm · {:.2}% accuracy",
            summary.personal_best.best_wpm, summary.personal_best.best_accuracy
        ));
    }
    if trend.len() > 1 {
        let sample = downsample_series(trend, width.min(72).saturating_sub(4).max(8));
        lines.push(String::new());
        lines.push("  trend".to_owned());
        lines.push(format!("  {}", render_sparkline_ascii(&sample)));
    }
    lines.join("\n") + "\n"
}

fn render_sparkline_ascii(values: &[f64]) -> String {
    const CHARS: &[u8] = b" .:-=+#";
    let maximum = values.iter().copied().fold(1.0, f64::max);
    values
        .iter()
        .map(|value| {
            CHARS[((value / maximum * (CHARS.len() - 1) as f64) as usize).min(CHARS.len() - 1)]
                as char
        })
        .collect()
}

fn draw_overlay(frame: &mut Frame, area: Rect, title: &str, body: String, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border)
        .title(Span::styled(title, theme.title));
    frame.render_widget(
        Paragraph::new(body).block(block).wrap(Wrap { trim: false }),
        centered_rect(
            area.width.min(76),
            area.height.saturating_sub(4).clamp(5, 30),
            area,
        ),
    );
}
fn help_text() -> String {
    "During a test\n  ?                help (before typing)\n  tab/enter        restart word tests\n  ctrl+backspace   delete word\n  ctrl+o           show/hide live stats\n  ctrl+s           settings\n  esc/ctrl+c       quit\n\nResults\n  tab/enter/r      restart\n  p                replay at original speed\n  S / M / L        settings / mode / language\n\nReplay\n  space pause  1/2/4 speed  r restart  esc/q back\n\nS settings  M modes  esc/q back".to_owned()
}

fn draw_mode_picker(frame: &mut Frame, area: Rect, selected: usize, theme: &Theme) {
    let mut lines = vec![
        Line::styled("Select practice mode", theme.help),
        Line::default(),
    ];
    for (index, mode) in TextMode::all().iter().enumerate() {
        lines.push(Line::styled(
            format!("{} {mode}", if index == selected { ">" } else { " " }),
            if index == selected {
                theme.selected
            } else {
                theme.help
            },
        ));
    }
    lines.push(Line::default());
    lines.push(Line::styled(
        "up/down select  enter apply and restart  esc/q cancel",
        theme.help,
    ));
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border)
        .title(Span::styled("Mode", theme.title));
    frame.render_widget(
        Paragraph::new(Text::from(lines)).block(block),
        centered_rect(48, area.height.min(20), area),
    );
}

fn draw_settings(
    frame: &mut Frame,
    area: Rect,
    config: Option<&TestConfig>,
    selected: usize,
    theme: &Theme,
) {
    let Some(config) = config else {
        return;
    };
    let values = [
        if config.kind == TestKind::Words {
            "words".to_owned()
        } else {
            "timed".to_owned()
        },
        if config.kind == TestKind::Words {
            format!("{} words", config.word_count)
        } else {
            format!("{}s", config.duration.as_secs())
        },
        config.text_mode.to_string(),
        if config.language.is_empty() {
            "english (built-in)".to_owned()
        } else {
            config.language.clone()
        },
        if config.width == 0 {
            "auto".to_owned()
        } else {
            config.width.to_string()
        },
        config.theme.clone(),
        on_off(config.punctuation).to_owned(),
        on_off(config.numbers).to_owned(),
        on_off(config.blind).to_owned(),
        on_off(config.zen).to_owned(),
        if config.min_wpm == 0 {
            "off".to_owned()
        } else {
            config.min_wpm.to_string()
        },
    ];
    let mut lines = vec![
        Line::styled(
            "Changes restart the current test and are saved as defaults.",
            theme.help,
        ),
        Line::default(),
    ];
    for (index, (label, value)) in SETTINGS_LABELS.iter().zip(values.iter()).enumerate() {
        lines.push(Line::styled(
            format!(
                "{} {label:<10} {value}",
                if index == selected { ">" } else { " " }
            ),
            if index == selected {
                theme.selected
            } else {
                theme.help
            },
        ));
    }
    lines.push(Line::default());
    lines.push(Line::styled(
        "up/down select  left/right adjust  enter apply  esc/q cancel",
        theme.help,
    ));
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border)
        .title(Span::styled("Settings", theme.title));
    frame.render_widget(
        Paragraph::new(Text::from(lines)).block(block),
        centered_rect(62, area.height.min(21), area),
    );
}

fn draw_language_picker(
    frame: &mut Frame,
    area: Rect,
    ids: &[String],
    selected: usize,
    theme: &Theme,
) {
    let mut lines = vec!["Built-in and cached languages".to_owned(), String::new()];
    for (index, id) in ids.iter().enumerate() {
        let marker = if id.is_empty() { "" } else { "  cached" };
        lines.push(format!(
            "{} {}{marker}",
            if index == selected { ">" } else { " " },
            crate::langcache::display_name(id),
        ));
    }
    lines.push(String::new());
    lines.push("This offline build reads lists already stored in".to_owned());
    lines.push("$XDG_DATA_HOME/ttype/languages.".to_owned());
    lines.push(String::new());
    lines.push("up/down select  enter apply  esc/q back".to_owned());
    draw_overlay(frame, area, "Language", lines.join("\n"), theme);
}

fn draw_replay(frame: &mut Frame, area: Rect, replay: &ReplayPlayback, theme: &Theme) {
    let status = if replay.paused {
        "paused"
    } else if replay.done() {
        "end"
    } else {
        "playing"
    };
    let mut lines = vec![
        Line::styled(
            format!("replay · {}x · {status}", replay.speed),
            theme.title,
        ),
        Line::default(),
    ];
    lines.extend(render_target(&replay.session, theme));
    lines.push(Line::default());
    lines.push(Line::styled(
        "space pause  1/2/4 speed  r restart  esc/q back",
        theme.help,
    ));
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border)
        .title(Span::styled("ttype", theme.title));
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .block(block)
            .wrap(Wrap { trim: false }),
        centered_rect(
            typing_width(area, &replay.config),
            area.height.saturating_sub(2).max(3),
            area,
        ),
    );
}

fn typing_width(area: Rect, config: &TestConfig) -> u16 {
    area.width
        .min(if config.width == 0 {
            100
        } else {
            config.width as u16
        })
        .max(10)
}
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width.min(area.width),
        height.min(area.height),
    )
}
fn format_clock(duration: Duration) -> String {
    let rounded_seconds = (duration.as_millis().saturating_add(500) / 1_000) as u64;
    format!("{}:{:02}", rounded_seconds / 60, rounded_seconds % 60)
}
fn on_off(value: bool) -> &'static str {
    if value { "on" } else { "off" }
}
fn ctrl(key: KeyEvent, character: char) -> bool {
    key.code == KeyCode::Char(character) && key.modifiers.contains(KeyModifiers::CONTROL)
}
fn mode_index(mode: TextMode) -> usize {
    TextMode::all()
        .iter()
        .position(|candidate| *candidate == mode)
        .unwrap_or(0)
}

/// Interactive history and replay browser. It uses the same single-threaded
/// event loop as the test screen and reads the replay sidecars by result ID.
pub fn run_history(
    paths: &Paths,
    rows: Vec<StoredResult>,
    theme_name: ThemeName,
) -> Result<(), TuiError> {
    let mut browser = HistoryBrowser {
        paths: paths.clone(),
        rows,
        selected: 0,
        replay: None,
        notice: String::new(),
        theme: Theme::from_name(theme_name.as_str()),
    };
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    if let Err(error) = execute!(stdout, EnterAlternateScreen) {
        let _ = disable_raw_mode();
        return Err(TuiError::Io(error));
    }
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    let result = loop {
        terminal.draw(|frame| browser.draw(frame))?;
        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
            && key.kind != KeyEventKind::Release
            && browser.handle(key)?
        {
            break Ok(());
        }
        if let Some(replay) = &mut browser.replay {
            replay.advance();
        }
    };
    let restored = restore_terminal(&mut terminal);
    match (result, restored) {
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}

struct HistoryBrowser {
    paths: Paths,
    rows: Vec<StoredResult>,
    selected: usize,
    replay: Option<ReplayPlayback>,
    notice: String,
    theme: Theme,
}
impl HistoryBrowser {
    fn handle(&mut self, key: KeyEvent) -> Result<bool, TuiError> {
        if let Some(replay) = &mut self.replay {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => self.replay = None,
                KeyCode::Char(' ') => replay.paused = !replay.paused,
                KeyCode::Char('1') => replay.speed = 1,
                KeyCode::Char('2') => replay.speed = 2,
                KeyCode::Char('4') => replay.speed = 4,
                KeyCode::Char('r') | KeyCode::Char('R') => replay.restart()?,
                _ => {}
            }
            return Ok(false);
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(true),
            KeyCode::Up | KeyCode::Char('k') => self.selected = self.selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.rows.is_empty() {
                    self.selected = (self.selected + 1).min(self.rows.len() - 1);
                }
            }
            KeyCode::Enter => self.start_replay()?,
            _ => {}
        }
        Ok(false)
    }
    fn start_replay(&mut self) -> Result<(), TuiError> {
        let Some(row) = self.rows.get(self.selected) else {
            return Ok(());
        };
        match storage::load_replay(&self.paths, &row.id) {
            Ok(recording) => {
                self.replay = Some(ReplayPlayback::new(row.test_config(), recording)?);
                self.notice.clear();
            }
            Err(error) => self.notice = format!("no replay saved for this result: {error}"),
        }
        Ok(())
    }
    fn draw(&self, frame: &mut Frame) {
        if let Some(replay) = &self.replay {
            draw_replay(frame, frame.area(), replay, &self.theme);
            return;
        }
        let area = frame.area();
        let mut lines = vec![Line::styled("History", self.theme.title), Line::default()];
        if self.rows.is_empty() {
            lines.push(Line::styled("No test history yet.", self.theme.help));
        } else {
            lines.push(Line::styled(
                "Date              Mode       Lang        Test     WPM    Raw   Acc  Err",
                self.theme.help,
            ));
            for (index, row) in self.rows.iter().enumerate() {
                let language = history_language(&row.language);
                let date = history_date(&row.timestamp);
                let text = format!(
                    "{date:<16}  {:<9}  {language:<10}  {:<6}  {:>5.0}  {:>5.0}  {:>3.0}%  {:>3}",
                    row.text_mode(),
                    row.test_label(),
                    row.wpm,
                    row.raw_wpm,
                    row.accuracy,
                    row.keystrokes_incorrect.max(row.incorrect)
                );
                lines.push(Line::styled(
                    text,
                    if index == self.selected {
                        self.theme.selected
                    } else {
                        self.theme.help
                    },
                ));
            }
        }
        if !self.notice.is_empty() {
            lines.push(Line::default());
            lines.push(Line::styled(self.notice.clone(), self.theme.error));
        }
        lines.push(Line::default());
        lines.push(Line::styled(
            "up/down select  enter watch replay  esc/q quit",
            self.theme.help,
        ));
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.theme.border)
            .title(Span::styled("ttype", self.theme.title));
        frame.render_widget(
            Paragraph::new(Text::from(lines))
                .block(block)
                .wrap(Wrap { trim: false }),
            centered_rect(
                area.width.min(100),
                area.height.saturating_sub(2).max(3),
                area,
            ),
        );
    }
}

fn history_language(id: &str) -> String {
    let name = if id.is_empty() {
        "english".to_owned()
    } else {
        id.replace('_', " ")
    };
    let chars: Vec<_> = name.chars().collect();
    if chars.len() <= 10 {
        name
    } else {
        format!("{}…", chars[..9].iter().collect::<String>())
    }
}
fn history_date(timestamp: &str) -> String {
    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let month = timestamp
        .get(5..7)
        .and_then(|value| value.parse::<usize>().ok())
        .and_then(|index| months.get(index.saturating_sub(1)))
        .copied()
        .unwrap_or("???");
    let day = timestamp.get(8..10).unwrap_or("??").trim_start_matches('0');
    let time = timestamp.get(11..16).unwrap_or("??:??");
    format!("{month} {day:>2} {time}")
}

fn restore_terminal<W: Write>(
    terminal: &mut Terminal<CrosstermBackend<W>>,
) -> Result<(), TuiError> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use crate::clock::FakeClock;
    use crate::config::Paths;
    use crate::domain::{TestConfig, TestKind};
    use crate::session::Session;
    use crate::storage;

    use super::TuiApp;

    #[test]
    fn completed_run_is_persisted_before_the_result_screen_is_dismissed() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let root =
            std::env::temp_dir().join(format!("ttype-tui-finish-{}-{suffix}", std::process::id()));
        let paths = Paths {
            config: root.join("config"),
            data: root.join("data"),
        };
        let clock = FakeClock::new();
        let mut session = match Session::new(
            TestConfig {
                kind: TestKind::Words,
                word_count: 1,
                ..TestConfig::default()
            },
            "x",
            clock,
        ) {
            Ok(session) => session,
            Err(error) => panic!("create session: {error}"),
        };
        session.input_char('x');

        let mut app = TuiApp::new(session).with_paths(paths.clone());
        app.show_results();
        let rows = match storage::list_results(&paths, 0) {
            Ok(rows) => rows,
            Err(error) => panic!("list results: {error}"),
        };
        assert_eq!(rows.len(), 1);
        assert!(storage::load_replay(&paths, &rows[0].id).is_ok());

        app.show_results();
        let rows = match storage::list_results(&paths, 0) {
            Ok(rows) => rows,
            Err(error) => panic!("list results after duplicate finish: {error}"),
        };
        assert_eq!(rows.len(), 1);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn ordinary_background_check_never_delays_tui_exit() {
        let clock = FakeClock::new();
        let session = match Session::new(TestConfig::default(), "x", clock) {
            Ok(session) => session,
            Err(error) => panic!("create session: {error}"),
        };
        let (sender, receiver) = mpsc::channel();
        let mut app = TuiApp::new(session).with_update_messages(receiver);
        let started = Instant::now();
        app.wait_for_update();
        assert!(started.elapsed() < Duration::from_secs(1));
        drop(sender);
    }
}
