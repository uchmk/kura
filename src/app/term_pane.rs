//! The terminal pane: starting it, feeding it, pumping its output.

use super::*;

impl App {
    /// Open the terminal pane, close it, or move the keys in and out of it.
    ///
    /// `None` is the toggle a key presses: open it and take the keys, or give
    /// them back when it already has them.
    pub(super) fn terminal(&mut self, what: Tri) {
        if what == Some(false) {
            // Ending the shell ends whatever runs under it, and on screen that
            // looks just like `<C-t>` hiding the pane -- so a running program
            // is asked about first, and a prompt is not (Q21).
            if let Some(t) = self.term.as_ref().filter(|t| t.busy()) {
                // An empty title (ConPTY before the program sets one) falls back to the shell's own name.
                let what = match (t.title.is_empty(), self.term_shell.is_empty()) {
                    (false, _) => format!("`{}`", program_label(&t.title)),
                    (true, false) => format!("`{}`", self.term_shell),
                    (true, true) => "A program".to_owned(),
                };
                self.overlay = Overlay::Confirm(ConfirmOverlay {
                    title: "End the shell?".into(),
                    body: vec![format!("{what} is still running in the terminal, and ends with it.")],
                    options: vec![('y', "End it".into()), ('n', "Keep it".into())],
                    action: ConfirmAction::EndShell,
                    dest: None,
                });
                return;
            }
            let none = self.term.is_none();
            self.end_shell();
            if none {
                self.toast("No terminal to close");
            }
            return;
        }
        if self.term.is_some() {
            self.term_focus = what.unwrap_or(!self.term_focus);
            return;
        }
        let cwd = self.tabs[self.active].cwd.clone();
        let ctx = self.ctx.clone();
        // The real shape arrives with the first frame that draws it; this is
        // only what the shell starts life believing.
        let size = crate::terminal::Size::new(80, 24);
        // Empty means the default: `pwsh` on Windows when it is installed,
        // otherwise whatever the platform starts (Q29). A name here is the way
        // to ask for a particular one.
        let shell = match self.cfg.term.shell.is_empty() {
            false => Some((self.cfg.term.shell.clone(), self.cfg.term.args.clone())),
            // The arguments apply to the automatic pick too (#442); with no
            // program named there is nothing for them to go to.
            true => crate::terminal::default_shell().map(|s| (s, self.cfg.term.args.clone())),
        };
        let label = crate::terminal::shell_label(shell.as_ref().map(|(p, _)| p.as_str()));
        let program = shell.as_ref().map(|(p, _)| p.clone());
        let args = shell.as_ref().map(|(_, a)| a.clone()).unwrap_or_default();
        let log = std::env::var_os("KURA_PTY_LOG").map(PathBuf::from);
        let env: Vec<(String, String)> = self.cfg.term.env.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        match crate::terminal::Terminal::spawn_with_env(&cwd, size, (8, 16), shell, log.as_deref(), env, move || ctx.request_repaint()) {
            Ok(t) => {
                self.term = Some(t);
                self.term_focus = true;
                // The arguments the shell started with, from `KURA_TERM_ARGS`
                // or `[term] args`: a run could not see they had arrived (#267, #281).
                let shown = match args.is_empty() {
                    false => format!("{label} {}", args.join(" ")),
                    true => label.clone(),
                };
                self.toast(format!("Started {shown} — <C-t> back to the list"));
                crate::runinfo::remember_shell(&label);
                self.term_shell = label;
            }
            // Name the shell: `Terminal failed: … (os error 2)` did not say
            // which program was missing (#173). Looked up only now, after the
            // start failed, so a shell found some way other than `PATH` is
            // never refused on a guess.
            Err(e) => {
                // `reason` is what `kura env` repeats after its own
                // "did not start:"; the OS's own sentence is left out when the
                // missing program is named (it is in the OS language).
                let (reason, said) = match program.filter(|p| util::locate(p).is_none()) {
                    Some(p) => {
                        // A path was given, so PATH was never searched.
                        let r = match p.contains(['/', '\\']) {
                            true => format!("`{p}` does not exist"),
                            false => format!("`{p}` was not found on PATH"),
                        };
                        let s = format!("Terminal failed: {r} — set [term] shell to one that is");
                        (r, s)
                    }
                    None => (format!("{label}: {e}"), format!("Terminal failed ({label}): {e}")),
                };
                crate::runinfo::remember_shell_failed(&reason);
                self.error(said);
            }
        }
    }

    /// Drop the terminal, which sends the shell its shutdown, and say so: the
    /// pane going away looks the same as `<C-t>` hiding it otherwise.
    pub(super) fn end_shell(&mut self) {
        self.term_pending = None;
        let had = self.term.take().is_some();
        self.term_focus = false;
        self.max_term = false;
        if had {
            // Replace the `Started …` still up: the newer news hid behind it (#280).
            let started = format!("Started {}", self.term_shell);
            self.toast_instead(&[started.as_str()], "Ended the shell".into());
        }
    }

    /// Type the selection into the shell, quoted so a path with a space in it
    /// arrives as one word. Nothing is run: the line is left for the user to
    /// put a command in front of.
    ///
    /// With the pane closed, it is opened first (Q35). A shell that is still
    /// reading its profile can drop what is typed at it, so the line waits
    /// until the shell has marked its prompt (OSC 133), or written something
    /// and then gone quiet for 800 ms (Q39, #250), or five seconds, whichever
    /// comes first.
    pub(super) fn term_send_paths(&mut self) {
        let paths = self.tabs[self.active].targets();
        if paths.is_empty() {
            return;
        }
        let opened = self.term.is_none();
        if opened {
            self.terminal(Some(true));
        }
        let Some(term) = &self.term else { return };
        let how = term.quoting();
        let line: Vec<String> = paths.iter().map(|p| crate::terminal::quote(&p.to_string_lossy(), how)).collect();
        let bytes = format!(" {}", line.join(" ")).into_bytes();
        match !opened && term.has_drawn() {
            true => term.send(bytes),
            false => self.term_pending = Some((bytes, Instant::now())),
        }
        self.term_focus = true;
    }

    /// Look for `needle` in the terminal's scrollback and put the match on
    /// screen. Repeating the command walks the matches; running out wraps.
    pub(super) fn term_find(&mut self, needle: &str, back: bool) {
        let needle = needle.trim();
        if needle.is_empty() {
            return;
        }
        self.term_needle = needle.to_owned();
        let Some(term) = &mut self.term else { return };
        match term.search(needle, back) {
            Some(false) => return,
            Some(true) => return self.toast("Wrapped"),
            None => {}
        }
        // Nothing from here on; start again from the view.
        term.end_search();
        match term.search(needle, back) {
            Some(_) => self.toast("Wrapped"),
            None => self.error(format!("No match for {needle}")),
        }
    }

    /// The file under the cursor, or nothing-shaped when there is none.
    pub(super) fn hovered_path(&self) -> PathBuf {
        self.tabs[self.active].current.hovered().map_or_else(PathBuf::new, |e| e.path.clone())
    }

    /// Show picture `n` of the hovered file.
    ///
    /// The old picture is left up while the new one is drawn. A command takes
    /// long enough to see, and blinking to "loading" and back on every press
    /// of `<A-j>` is worse than a moment of the previous page.
    pub(super) fn go_to_picture(&mut self, n: i64) {
        if self.preview.n == n {
            return;
        }
        self.preview.n = n;
        self.preview.pending_since = None;
        self.request_preview(true);
    }

    /// Follow the shell: put the pane where it says it is.
    ///
    /// The other direction, and the useful one when a command has moved the
    /// shell somewhere the pane knows nothing about. It needs the shell to
    /// report its directory (OSC 7), which most do out of the box and some
    /// have to be told to.
    pub(super) fn term_pull_cwd(&mut self) {
        let Some(term) = &self.term else {
            self.error("The terminal is not open");
            return;
        };
        let Some(cwd) = term.shell_cwd.clone() else {
            // Naming the obstacle alone leaves nowhere to go: "OSC 7" is
            // hard to search for, and most of what comes back overrides
            // `prompt`, which breaks Starship and the other generators people
            // actually run. The hook that does not is named here, and the
            // command that prints it (Q50): a toast does not wrap, and the
            // hook itself is wider than any window.
            let said = no_osc7(&self.term_shell, &kura_command());
            self.error(said);
            return;
        };
        if cwd == self.tabs[self.active].cwd {
            return;
        }
        self.cd(cwd, true);
    }

    /// Read what the shell has said, and keep it in the directory the pane is
    /// showing. Called once a frame, and cheap when there is nothing to do.
    pub(super) fn pump_terminal(&mut self) {
        let cwd = self.tabs[self.active].cwd.clone();
        let Some(term) = &mut self.term else { return };
        let (fg, bg) = (self.cfg.theme.fg, self.cfg.theme.bg_alt);
        term.set_colors([fg.r(), fg.g(), fg.b()], [bg.r(), bg.g(), bg.b()]);
        for text in term.drain() {
            // A program asked for the clipboard; only this thread can oblige.
            let _ = exec::set_clipboard(&text);
        }
        if term.exited {
            self.term = None;
            self.term_pending = None;
            self.term_focus = false;
            self.max_term = false;
            self.toast("The shell exited");
            return;
        }
        // A shell that marks its prompt (OSC 133) is ready when it says so.
        // Otherwise, quiet for 800 ms after its first output: 300 was enough
        // on ARM64 (#249), but the x64 machine's pwsh printed its banner and
        // then said nothing for over 300 ms while its profile loaded, so the
        // path went in before the prompt, 3 runs of 3 (#250).
        let ready =
            |at: &Instant| term.prompt_seen() || term.quiet_for(Duration::from_millis(800)) || at.elapsed() > Duration::from_secs(5);
        if self.term_pending.as_ref().is_some_and(|(_, at)| ready(at)) {
            if let Some((bytes, _)) = self.term_pending.take() {
                term.send(bytes);
            }
        }
        term.follow(&cwd);
    }

    /// How the pane's keymap spells handing the keys back to the list.
    pub(super) fn term_leave_key(&self) -> Option<String> {
        let b = self.cfg.keymap.term.iter().find(|b| matches!(b.run.as_slice(), [Act::Close | Act::Escape(_)]))?;
        Some(crate::config::keys::display_seq(&b.on))
    }
}

/// What `<A-Up>` says when the shell has never reported its directory. The
/// shell is named, because the hook goes in *that* shell's profile: Windows
/// PowerShell 5.1 and PowerShell 7 read different ones, and someone running
/// 5.1 was sent to the same README line again and again (#101). Where there
/// is a hook for it, the command that adds it is the rest of the message
/// (Q50); 5.1 is told it cannot have one rather than handed one that fails.
pub(super) fn no_osc7(shell: &str, kura: &str) -> String {
    let who = if shell.is_empty() { "The shell".to_owned() } else { format!("`{shell}`") };
    let name = shell.split(' ').next().unwrap_or("").to_ascii_lowercase();
    let what = match name.trim_end_matches(".exe") {
        "powershell" => {
            return format!(
                "{who} has not said where it is (no OSC 7), and cannot: the hook needs PowerShell 7 \
                 (winget install Microsoft.PowerShell)"
            )
        }
        "bash" => format!("{kura} shell-hook bash >> ~/.bashrc"),
        "zsh" => format!("{kura} shell-hook zsh >> ~/.zshrc"),
        "" | "pwsh" => format!("{kura} shell-hook | Add-Content $PROFILE"),
        _ => return format!("{who} has not said where it is (no OSC 7). `kura shell-hook` has hooks for {}", crate::shellhook::SHELLS),
    };
    format!("{who} has not said where it is (no OSC 7). In that shell: {what}, then <C-S-t> and <C-t>")
}

/// How the pane's shell can run this kura: by name when that is what the
/// `PATH` finds, by its full path when it is not (a zip unpacked anywhere).
pub(super) fn kura_command() -> String {
    let Ok(me) = std::env::current_exe() else { return "kura".into() };
    let name = if cfg!(windows) { "kura.exe" } else { "kura" };
    let found = std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| same_file(&d.join(name), &me)));
    if found {
        "kura".into()
    } else if cfg!(windows) {
        format!("& '{}'", me.display())
    } else {
        format!("'{}'", me.display())
    }
}

pub(super) fn same_file(a: &std::path::Path, b: &std::path::Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// The console's default title is the shell's full path (`C:\...\pwsh.exe`); name the program by its file stem instead.
pub(super) fn program_label(title: &str) -> &str {
    let t = title.trim();
    if !t.contains(['/', '\\']) || t.contains(' ') && !t.to_ascii_lowercase().ends_with(".exe") {
        return t;
    }
    let name = t.rsplit(['/', '\\']).next().unwrap_or(t);
    let stem = name.strip_suffix(".exe").or_else(|| name.strip_suffix(".EXE")).unwrap_or(name);
    if stem.is_empty() {
        t
    } else {
        stem
    }
}
