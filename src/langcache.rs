use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

const MANIFEST_FILE: &str = "_manifest.json";

#[derive(Debug, Error)]
pub enum LanguageError {
    #[error("invalid language id {0:?}")]
    InvalidId(String),
    #[error("read cached language {id:?}: {source}")]
    Read {
        id: String,
        #[source]
        source: std::io::Error,
    },
    #[error("parse cached language {id:?}: {source}")]
    Parse {
        id: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("cached language {0:?} has an empty word list")]
    Empty(String),
}

#[derive(Debug, Deserialize)]
struct LanguageFile {
    words: Vec<String>,
}

/// Reads language lists that were already downloaded by a compatible ttype.
/// The Rust port deliberately never populates this cache from the network.
#[derive(Clone, Debug)]
pub struct LanguageCache {
    directory: PathBuf,
}

impl LanguageCache {
    pub fn new(data_dir: impl AsRef<Path>) -> Self {
        Self {
            directory: data_dir.as_ref().join("languages"),
        }
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn cached(&self, id: &str) -> bool {
        id.is_empty() || self.language_path(id).is_some_and(|path| path.is_file())
    }

    pub fn installed(&self) -> Vec<String> {
        let Ok(entries) = fs::read_dir(&self.directory) else {
            return Vec::new();
        };
        let mut ids: Vec<_> = entries
            .flatten()
            .filter_map(|entry| {
                let file_type = entry.file_type().ok()?;
                if !file_type.is_file() {
                    return None;
                }
                let name = entry.file_name();
                let name = name.to_str()?;
                let id = name.strip_suffix(".json")?;
                (id != MANIFEST_FILE.trim_end_matches(".json") && valid_id(id))
                    .then(|| id.to_owned())
            })
            .collect();
        ids.sort();
        ids
    }

    pub fn words(&self, id: &str) -> Result<Vec<String>, LanguageError> {
        let path = self
            .language_path(id)
            .ok_or_else(|| LanguageError::InvalidId(id.to_owned()))?;
        let source = fs::read_to_string(path).map_err(|source| LanguageError::Read {
            id: id.to_owned(),
            source,
        })?;
        let list: LanguageFile =
            serde_json::from_str(&source).map_err(|source| LanguageError::Parse {
                id: id.to_owned(),
                source,
            })?;
        if list.words.is_empty() {
            return Err(LanguageError::Empty(id.to_owned()));
        }
        Ok(list.words)
    }

    fn language_path(&self, id: &str) -> Option<PathBuf> {
        valid_id(id).then(|| self.directory.join(format!("{id}.json")))
    }
}

pub fn display_name(id: &str) -> String {
    if id.is_empty() {
        "english (built-in)".to_owned()
    } else {
        id.replace('_', " ")
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}
