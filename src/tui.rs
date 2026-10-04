use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use thiserror::Error;

use crate::session::{Session, SessionState};

#[derive(Debug, Error)]
pub enum TuiError {
    #[error("terminal I/O: {0}")]
    Io(#[from] io::Error),
}

/// Owns the single terminal event loop. No task, thread, or async runtime is
/// started: ticks and input are multiplexed by crossterm polling.
pub struct TuiApp {
    session: Session,
    show_help: bool,
    show_live_stats: bool,
}

impl TuiApp {
    pub fn new(session: Session) -> Self {
        Self {
            session,
            show_help: false,
            show_live_stats: true,
        }
    }

    pub fn run(mut self) -> Result<Session, TuiError> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
        let run_result = self.run_loop(&mut terminal);
        let restore_result = restore_terminal(terminal);
        match (run_result, restore_result) {
            (Err(error), _) => Err(error),
            (Ok(()), Err(error)) => Err(error),
            (Ok(()), Ok(())) => Ok(self.session),
        }
    }

    fn run_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<(), TuiError> {
        loop {
            terminal.draw(|frame| self.draw(frame))?;
            if event::poll(Duration::from_millis(100))?
                && let Event::Key(key) = event::read()?
                && self.handle_key(key)
            {
                return Ok(());
            }
            self.session.tick();
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if matches!(self.session.state(), SessionState::Finished) {
            return matches!(key.code, KeyCode::Esc | KeyCode::Char('q'));
        }
        match key.code {
            KeyCode::Esc => true,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => true,
            KeyCode::Backspace => {
                self.session.backspace();
                false
            }
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.session.delete_word();
                false
            }
            KeyCode::Tab | KeyCode::Enter => self.session.restart().is_err(),
            KeyCode::Char('?') if matches!(self.session.state(), SessionState::Ready) => {
                self.show_help = !self.show_help;
                false
            }
            KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.show_live_stats = !self.show_live_stats;
                false
            }
            KeyCode::Char(character) => {
                self.session.input_char(character);
                false
            }
            _ => false,
        }
    }

    fn draw(&self, frame: &mut Frame) {
        let area = frame.area();
        let text = if self.show_help {
            "ttype\n\nEsc quit  Tab/Enter restart  Backspace delete\nCtrl+W delete word  Ctrl+O live stats\n? toggles this help before typing"
                .to_owned()
        } else if matches!(self.session.state(), SessionState::Finished) {
            match self.session.result() {
                Ok(result) => format!(
                    "Test {}\n\nWPM {:.0}   ACC {:.0}%\nraw {:.0}   consistency {:.0}%\nchars {}/{}/{}/{}\n\nEsc/q quit",
                    if result.failed { "Failed" } else { "Complete" },
                    result.wpm,
                    result.accuracy,
                    result.raw_wpm,
                    result.consistency,
                    result.correct,
                    result.incorrect,
                    result.total_chars - result.correct - result.incorrect,
                    result.skipped
                ),
                Err(error) => format!("result unavailable: {error}"),
            }
        } else {
            let mut body = String::new();
            if self.show_live_stats && !self.session.config().zen {
                body.push_str(&format!(
                    "wpm {:.0}   raw {:.0}   acc {:.0}%   {}\n\n",
                    self.session.live_wpm(),
                    self.session.live_raw_wpm(),
                    self.session.live_accuracy(),
                    if self.session.config().is_words_mode() {
                        let (done, total) = self.session.words_progress();
                        format!("words {done}/{total}")
                    } else {
                        format!("time {}s", self.session.remaining().as_secs())
                    }
                ));
            }
            body.push_str(&render_target(&self.session));
            body
        };
        frame.render_widget(
            Paragraph::new(text)
                .block(Block::default().borders(Borders::ALL).title("ttype"))
                .wrap(ratatui::widgets::Wrap { trim: false }),
            area,
        );
    }
}

fn render_target(session: &Session) -> String {
    session
        .target_chars()
        .iter()
        .enumerate()
        .map(|(index, expected)| match session.input_chars().get(index) {
            None => *expected,
            Some(actual) if *actual == *expected => *expected,
            Some('\0') => '·',
            Some(_) => '×',
        })
        .collect()
}

fn restore_terminal(mut terminal: Terminal<CrosstermBackend<Stdout>>) -> Result<(), TuiError> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
