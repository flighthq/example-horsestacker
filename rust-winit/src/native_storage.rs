//! Small native replacement for the browser's `localStorage` preferences.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct NativePreferences {
    path: Option<PathBuf>,
    values: HashMap<String, String>,
}

impl NativePreferences {
    pub fn load(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let values = fs::read_to_string(&path)
            .ok()
            .map(|contents| {
                contents
                    .lines()
                    .filter_map(|line| line.split_once('='))
                    .map(|(key, value)| (key.to_owned(), value.to_owned()))
                    .collect()
            })
            .unwrap_or_default();
        Self {
            path: Some(path),
            values,
        }
    }

    pub fn memory() -> Self {
        Self::default()
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.values.insert(key.into(), value.into());
        self.flush();
    }

    fn flush(&self) {
        let Some(path) = &self.path else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let mut entries: Vec<_> = self.values.iter().collect();
        entries.sort_by_key(|(key, _)| key.as_str());
        let contents = entries
            .into_iter()
            .map(|(key, value)| format!("{key}={value}\n"))
            .collect::<String>();
        let _ = fs::write(path, contents);
    }
}
