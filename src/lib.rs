#![deny(unsafe_code)]

pub const BUILD_VERSION: &str = env!("TTYPE_VERSION");

pub mod app;
pub mod assets;
pub mod cli;
pub mod clock;
pub mod completion;
pub mod config;
pub mod custom_text;
pub mod doctor;
pub mod domain;
pub mod replay;
pub mod result_file;
pub mod session;
pub mod stats;
pub mod storage;
pub mod text;
pub mod time;
pub mod tui;
pub mod uninstall;
pub mod update;
