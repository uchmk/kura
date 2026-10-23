//! Saving and restoring state.

use super::*;

impl App {
    pub(super) fn state_file(name: &str) -> PathBuf {
        Config::state_dir().join(name)
    }

    pub(super) fn load_state(&mut self) {
        if let Ok(text) = std::fs::read_to_string(Self::state_file("bookmarks.toml")) {
            if let Ok(f) = toml::from_str::<BookmarkFile>(&text) {
                self.bookmarks = f.bookmark;
            }
        }
        // The state files are TOML. A `.txt` of an older build is read when
        // there is no `.toml` yet, and the `.toml` written from it at once; the
        // `.txt` is left where it is.
        if let Some(f) = ito_common::kept::read::<HistoryFile>(self.kept.history.path()) {
            self.history = f.entries.into_iter().map(visit_of).collect();
        } else if let Ok(text) = std::fs::read_to_string(Self::state_file("history.txt")) {
            self.history = text.lines().map(parse_visit).collect();
            self.save_history();
        }
        for (group, txt) in Self::INPUT_HISTORY_TXT.into_iter().enumerate() {
            if let Some(f) = ito_common::kept::read::<InputFile>(self.kept.input[group].path()) {
                self.input_history[group] = f.entries.into_iter().filter(|l| !l.is_empty()).collect();
            } else if let Ok(text) = std::fs::read_to_string(Self::state_file(txt)) {
                self.input_history[group] = text.lines().filter(|l| !l.is_empty()).map(str::to_owned).collect();
                self.kept.input[group].save_toml(&InputFile { entries: self.input_history[group].clone() });
            }
        }
    }

    pub fn save_state(&self) {
        let dir = Config::state_dir();
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        let f = BookmarkFile { bookmark: self.bookmarks.clone() };
        if let Ok(text) = toml::to_string_pretty(&f) {
            let _ = std::fs::write(dir.join("bookmarks.toml"), text);
        }
        self.save_history();
    }

    pub(super) fn history_file(&self) -> HistoryFile {
        HistoryFile {
            entries: self.history.iter().map(|v| VisitRow { path: v.path.display().to_string(), hits: v.hits, at: v.at }).collect(),
        }
    }

    pub(super) fn save_history(&self) {
        self.kept.history.save_toml(&self.history_file());
    }

    pub fn on_quit(&self) {
        self.save_state();
        // The thread of the save above may not finish before the process ends.
        if let Ok(text) = toml::to_string(&self.history_file()) {
            let _ = self.kept.history.save_now(text);
        }
        if let Some(f) = &self.cwd_file {
            let _ = std::fs::write(f, self.tabs[self.active].cwd.display().to_string());
        }
        if let Some(f) = &self.chooser_file {
            let paths = self.tabs[self.active].targets();
            let text: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
            let _ = std::fs::write(f, text.join("\n"));
        }
    }
}
