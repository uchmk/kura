//! yazi-compatible keymap loading and chord resolution, by `ito-keymap`
//! (where this file's loader went in ito 0.32.0, for every app to share).
//!
//! Layering follows yazi: `prepend_keymap` entries come first, then either the
//! user's `keymap` (a full replacement) or the built-in defaults, then
//! `append_keymap`. The first exact match wins, which is what makes a
//! prepended single-key binding shadow a default prefix. A key bound twice
//! names each side's file (Q57, #193, #194); a user's key over a default one
//! is an override, not a warning (Q60).

use super::cmd::{self, Act};
#[cfg(test)]
use super::keys::Key;

pub use ito_keymap::resolve;
#[cfg(test)]
pub use ito_keymap::BUILT_IN;

pub type Binding = ito_keymap::Binding<Act>;
pub type Match<'a> = ito_keymap::Match<'a, Act>;

#[derive(Default, Debug)]
pub struct Keymap {
    pub mgr: Vec<Binding>,
    pub input: Vec<Binding>,
    pub confirm: Vec<Binding>,
    pub pick: Vec<Binding>,
    pub help: Vec<Binding>,
    pub tasks: Vec<Binding>,
    pub spot: Vec<Binding>,
    /// The side-by-side compare view's own keys.
    pub diff: Vec<Binding>,
    /// The settings screen's keys (`<C-,>`): the rest go to its controls.
    pub settings: Vec<Binding>,
    /// The few keys the terminal pane keeps for itself; everything else it
    /// hears goes to the shell.
    pub term: Vec<Binding>,
    /// Commands that parsed but aren't implemented, for the help panel.
    pub unsupported: Vec<String>,
    /// A key of the built-in defaults that a user's file binds to something
    /// else: meant, so not a warning, but kept where "what did my `T`
    /// displace?" can be looked up -- the help panel and `kura env` (Q60).
    pub overrides: Vec<String>,
}

pub const DEFAULT_KEYMAP: &str = include_str!("defaults/keymap.toml");

/// kura's layers, with yazi's older names for two of them. yazi's `[cmp]`
/// (and its older `[completion]`) is read and left alone: completion is
/// native here.
const SPEC: ito_keymap::Spec = ito_keymap::Spec {
    layers: &[
        ito_keymap::Layer { name: "mgr", aliases: &["manager"] },
        ito_keymap::Layer::new("input"),
        ito_keymap::Layer::new("confirm"),
        ito_keymap::Layer { name: "pick", aliases: &["select"] },
        ito_keymap::Layer::new("help"),
        ito_keymap::Layer::new("tasks"),
        ito_keymap::Layer::new("spot"),
        ito_keymap::Layer::new("diff"),
        ito_keymap::Layer::new("settings"),
        ito_keymap::Layer::new("term"),
    ],
    ignored: &["cmp", "completion"],
    defaults: DEFAULT_KEYMAP,
};

impl Keymap {
    /// Fold the built-in defaults and each user keymap file in turn, for a
    /// test that has the text and no path to name.
    #[cfg(test)]
    pub fn load(user_tomls: &[&str]) -> (Self, Vec<String>) {
        let named: Vec<(&str, &str)> = user_tomls.iter().map(|t| ("keymap.toml", *t)).collect();
        Self::load_named(&named)
    }

    /// Whether `text` reads as a keymap file at all. A file that does not is
    /// skipped by [`Keymap::load_named`], with a warning first among its own.
    pub fn parses(text: &str) -> bool {
        ito_keymap::parses(text)
    }

    /// The same, with each file's path to name in a parse error: there are two
    /// config directories, and `keymap.toml: TOML parse error` did not say which
    /// one's (#131), where the other three files' errors always did.
    pub fn load_named(user_tomls: &[(&str, &str)]) -> (Self, Vec<String>) {
        let mut command = |s: &str, warnings: &mut Vec<String>| {
            let a = cmd::parse(s);
            if let Act::Unsupported(_) = a {
                warnings.push(format!("unsupported command `{s}`"));
            }
            Some(a)
        };
        let mut loaded = ito_keymap::load(&SPEC, user_tomls, &mut command);
        let mut take = |name| loaded.take(name);
        let mut km = Keymap {
            mgr: take("mgr"),
            input: take("input"),
            confirm: take("confirm"),
            pick: take("pick"),
            help: take("help"),
            tasks: take("tasks"),
            spot: take("spot"),
            diff: take("diff"),
            settings: take("settings"),
            term: take("term"),
            unsupported: Vec::new(),
            overrides: loaded.overrides,
        };
        km.unsupported = loaded.warnings.clone();
        (km, loaded.warnings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #131: a broken keymap.toml is named by its path, like the other three
    /// files, and the warning does not end in a blank line.
    #[test]
    fn a_broken_keymap_names_its_path() {
        let (_, warnings) = Keymap::load_named(&[("/cfg/kura/keymap.toml", "[[mgr.keymap]\n")]);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].starts_with("/cfg/kura/keymap.toml: TOML parse error"), "{}", warnings[0]);
        assert!(!warnings[0].ends_with('\n'), "{:?}", warnings[0]);
    }

    /// #164 / #258: one `[[mgr.keymap]]` entry replaced every key in the list,
    /// `q` included, and nothing said so. A short replacement is named, with
    /// the section that would have kept the defaults.
    #[test]
    fn a_short_keymap_that_replaces_the_defaults_is_named() {
        let user = "[[mgr.keymap]]\non = \"<F9>\"\nrun = \"config_reload\"\n";
        let (km, warnings) = Keymap::load_named(&[("/cfg/keymap.toml", user)]);
        assert_eq!(km.mgr.len(), 1, "the defaults are replaced, as in yazi");
        let said: Vec<&String> = warnings.iter().filter(|w| w.contains("prepend_keymap")).collect();
        assert_eq!(said.len(), 1, "{warnings:?}");
        assert!(said[0].starts_with("/cfg/keymap.toml: `[[mgr.keymap]]` replaces all "), "{}", said[0]);
        assert!(said[0].ends_with(" with 1 -- did you mean `[[mgr.prepend_keymap]]`?"), "{}", said[0]);
    }

    /// A copy of the whole keymap replaces it on purpose and is not warned
    /// about, and neither is a `prepend_keymap`, which keeps the defaults.
    #[test]
    fn a_full_replacement_or_a_prepend_is_not_named() {
        let (_, warnings) = Keymap::load(&[DEFAULT_KEYMAP]);
        assert!(!warnings.iter().any(|w| w.contains("did you mean")), "{warnings:?}");
        let (_, warnings) = Keymap::load(&["[[mgr.prepend_keymap]]\non = \"<F9>\"\nrun = \"config_reload\"\n"]);
        assert!(!warnings.iter().any(|w| w.contains("did you mean")), "{warnings:?}");
    }

    /// A Windows path in double quotes is the usual broken keymap; the warning
    /// says how to write it.
    #[test]
    fn a_backslash_in_double_quotes_gets_the_single_quote_hint() {
        let user = "[[mgr.prepend_keymap]]\non = \"e\"\nrun = \"shell 'C:\\dev\\nvim.exe'\"\n";
        let (_, warnings) = Keymap::load(&[user]);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].ends_with("write a Windows path in 'single quotes')"), "{}", warnings[0]);
    }

    #[test]
    fn default_keymap_loads() {
        let (km, warnings) = Keymap::load(&[]);
        assert!(km.mgr.len() > 50, "got {} bindings", km.mgr.len());
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn prepend_shadows_default_prefix() {
        let user = r#"
[[mgr.prepend_keymap]]
on = "m"
run = "plugin bookmarks save"
"#;
        let (km, _) = Keymap::load(&[user]);
        let m = Key::parse("m").unwrap();
        match resolve(&km.mgr, &[m]) {
            Match::Exact(b) => assert_eq!(b.run, vec![Act::BookmarkSave]),
            _ => panic!("prepended single key should win over the default prefix"),
        }
    }

    #[test]
    fn multi_key_chords_pend() {
        let (km, _) = Keymap::load(&[]);
        let g = Key::parse("g").unwrap();
        assert!(matches!(resolve(&km.mgr, &[g]), Match::Pending(_)));
    }

    fn bound(section: &[Binding], key: &str) -> Vec<Act> {
        let k = Key::parse(key).unwrap_or_else(|| panic!("`{key}` is not key notation"));
        match resolve(section, &[k]) {
            Match::Exact(b) => b.run.clone(),
            _ => panic!("`{key}` is not bound to anything on its own"),
        }
    }

    /// The easy key must not be the destructive one. `<C-t>` is pressed all day
    /// to go to and fro; ending the shell — losing the scrollback and whatever
    /// is running — belongs on the harder chord.
    #[test]
    fn the_terminals_easy_key_keeps_the_shell_alive() {
        let (km, _) = Keymap::load(&[]);

        assert_eq!(bound(&km.term, "<C-t>"), vec![Act::Close]);
        assert_eq!(bound(&km.term, "<C-S-t>"), vec![Act::Terminal(Some(false))]);
    }

    /// Without these the help and the palette cannot be reached at all while
    /// the terminal holds the keys, and the shell has no use for either.
    #[test]
    fn the_terminal_layer_lets_the_help_and_the_palette_through() {
        let (km, _) = Keymap::load(&[]);

        assert_eq!(bound(&km.term, "<F1>"), vec![Act::Help]);
        assert_eq!(bound(&km.term, "<C-S-p>"), vec![Act::Palette]);
    }

    /// Reloading from inside the pane: before, `<C-F5>` went to the shell as
    /// `\e[15;5~` and trying a config change meant leaving the pane each time.
    #[test]
    fn the_terminal_layer_reloads_the_config() {
        let (km, _) = Keymap::load(&[]);

        assert_eq!(bound(&km.term, "<C-F5>"), bound(&km.mgr, "<C-F5>"));
        assert_eq!(bound(&km.term, "<C-F5>"), vec![Act::ConfigReload]);
    }

    /// The one place the defaults knowingly leave yazi's: `<C-r>` is redo, as
    /// it is nearly everywhere, and inverting the selection moves over one
    /// modifier. Worth a test, because the obvious "fix" is to put it back.
    #[test]
    fn ctrl_r_redoes_and_inverting_the_selection_moved() {
        let (km, _) = Keymap::load(&[]);

        assert_eq!(bound(&km.mgr, "<C-r>"), vec![Act::Redo]);
        assert_eq!(bound(&km.mgr, "U"), vec![Act::Redo]);
        assert_eq!(bound(&km.mgr, "<C-S-r>"), vec![Act::ToggleAll { state: None }]);
    }

    /// Jumping is `'`, the rest hangs off `b`. Both references agree on this
    /// split — vim jumps to a mark with `'`, and bookmarks.yazi jumps with `'`
    /// and deletes with `b`+`d` — and the earlier arrangement, where `b` and
    /// `'` were the same command, read in the key list as one of them being
    /// something else.
    #[test]
    fn bookmarks_jump_with_the_vim_key_and_are_managed_under_b() {
        let (km, _) = Keymap::load(&[]);
        let chord = |keys: &[&str]| {
            let ks: Vec<Key> = keys.iter().map(|k| Key::parse(k).expect("key notation")).collect();
            match resolve(&km.mgr, &ks) {
                Match::Exact(b) => b.run.clone(),
                _ => panic!("`{keys:?}` is not bound"),
            }
        };

        assert_eq!(bound(&km.mgr, "'"), vec![Act::BookmarkJump]);
        assert_eq!(chord(&["b", "b"]), vec![Act::BookmarkList]);
        assert_eq!(chord(&["b", "s"]), vec![Act::BookmarkSave]);
        assert_eq!(chord(&["b", "d"]), vec![Act::BookmarkDelete]);
        assert_eq!(chord(&["b", "D"]), vec![Act::BookmarkDeleteAll]);

        // `b` alone must stay a prefix: bound to a command of its own it would
        // shadow every chord above, which is how it used to behave.
        assert!(matches!(resolve(&km.mgr, &[Key::parse("b").unwrap()]), Match::Pending(_)), "`b` must lead somewhere, not do something",);

        // The keys these grew out of still work.
        assert_eq!(bound(&km.mgr, "B"), vec![Act::BookmarkSave]);
        assert_eq!(bound(&km.mgr, "<A-b>"), vec![Act::BookmarkDelete]);
        assert_eq!(bound(&km.mgr, "<A-B>"), vec![Act::BookmarkDeleteAll]);
    }

    /// No key may mean two things in one layer, and no single key may sit in
    /// front of a chord that starts with it: `resolve` takes the first exact
    /// match, so the chord would never be reached.
    #[test]
    fn no_layer_binds_a_key_twice_or_swallows_its_own_chords() {
        let (km, _) = Keymap::load(&[]);
        let layers: [(&str, &[Binding]); 10] = [
            ("mgr", &km.mgr),
            ("input", &km.input),
            ("confirm", &km.confirm),
            ("pick", &km.pick),
            ("help", &km.help),
            ("tasks", &km.tasks),
            ("spot", &km.spot),
            ("term", &km.term),
            ("diff", &km.diff),
            ("settings", &km.settings),
        ];
        for (name, bindings) in layers {
            for (i, b) in bindings.iter().enumerate() {
                for (j, other) in bindings.iter().enumerate().skip(i + 1) {
                    assert_ne!(b.on, other.on, "[{name}] binds {:?} twice", b.on);
                    assert!(
                        !(b.on.len() == 1 && other.on.len() > 1 && other.on[0] == b.on[0]),
                        "[{name}] #{i} {:?} comes before #{j} {:?} and swallows it",
                        b.on,
                        other.on,
                    );
                }
            }
        }
    }

    /// The defaults being clean is only half of it: the arrangement that
    /// actually bites is a user file's, and it bites in silence. One
    /// `prepend_keymap` line binding `m` — which is what the bookmark plugins
    /// for yazi do, and what a config copied from one carries over — sits in
    /// front of every line-mode chord and ends the sequence at `m`. Pressing
    /// `m`+`s` then saves a bookmark under `s`, and nothing anywhere says the
    /// line-mode keys are gone.
    #[test]
    fn a_user_key_that_buries_a_whole_prefix_is_reported() {
        let user = r#"
            [[mgr.prepend_keymap]]
            on = "m"
            run = "plugin bookmarks save"
        "#;
        let (_, warnings) = Keymap::load(&[user]);

        let about_m: Vec<&String> = warnings.iter().filter(|w| w.contains("`m`")).collect();
        assert_eq!(about_m.len(), 1, "one line per prefix, not one per key lost: {warnings:?}");
        let w = about_m[0];
        assert!(w.contains("plugin bookmarks save"), "must name what took the key: {w}");
        assert!(w.contains("`ms`"), "must name a key that was lost: {w}");
        assert!(w.contains('6'), "must count them: {w}");
    }

    /// The same key twice is the other way a line goes missing, and it reads
    /// as the key being unbound rather than as the file disagreeing with
    /// itself.
    #[test]
    fn a_key_bound_twice_is_reported() {
        let user = r#"
            [[mgr.prepend_keymap]]
            on = "<F5>"
            run = "quit"
            [[mgr.prepend_keymap]]
            on = "<F5>"
            run = "close"
        "#;
        let (_, warnings) = Keymap::load(&[user]);

        let w = warnings.iter().find(|w| w.contains("more than once"));
        let w = w.unwrap_or_else(|| panic!("no duplicate reported: {warnings:?}"));
        assert!(w.contains("quit"), "must name the one that wins: {w}");
    }

    /// Q57: the warning names the file of each side, the built-in defaults
    /// included (#193 found the other side was neither of the two files the
    /// Config section listed), and the run that lost (#194).
    /// `<S-t>` is accepted but said to be unpressable, with what to write.
    #[test]
    fn a_shifted_printable_key_is_warned_about() {
        let (_, w) = Keymap::load(&["[[mgr.prepend_keymap]]\non = \"<S-t>\"\nrun = \"quit\"\n[[mgr.prepend_keymap]]\non = \"<S-Enter>\"\nrun = \"quit\"\n[[mgr.prepend_keymap]]\non = \"<C-S-t>\"\nrun = \"quit\"\n"]);
        let shift: Vec<&String> = w.iter().filter(|w| w.contains("can never be pressed")).collect();
        assert_eq!(shift.len(), 1, "only the bare `<S-t>`: {w:?}");
        assert!(shift[0].contains("write `T`"), "{}", shift[0]);
    }

    #[test]
    fn a_duplicate_names_each_sides_file() {
        let yazi = "[[mgr.prepend_keymap]]\non = \"Q\"\nrun = \"quit\"\n";
        let kura = "[[mgr.prepend_keymap]]\non = \"Q\"\nrun = \"hidden toggle\"\n";
        let (_, warnings) = Keymap::load_named(&[("/cfg/yazi/keymap.toml", yazi), ("/cfg/kura/keymap.toml", kura)]);
        let w = warnings.iter().find(|w| w.contains("`Q`")).unwrap_or_else(|| panic!("{warnings:?}"));
        assert_eq!(
            w,
            "[mgr] `Q` is bound more than once; only `hidden toggle` (/cfg/kura/keymap.toml) runs, \
             not `quit` (/cfg/yazi/keymap.toml)",
        );

        // A default bound again in a user's own file wins over it on purpose:
        // not a warning, an override (Q60).
        let one = "[[mgr.prepend_keymap]]\non = \"T\"\nrun = \"quit\"\n";
        let (km, warnings) = Keymap::load_named(&[("/cfg/kura/keymap.toml", one)]);
        assert!(!warnings.iter().any(|w| w.contains("`T`")), "{warnings:?}");
        let o = km.overrides.iter().find(|o| o.contains("`T`")).unwrap_or_else(|| panic!("{:?}", km.overrides));
        assert!(o.starts_with("[mgr] `T`: `quit` (/cfg/kura/keymap.toml) instead of the default `"), "{o}");
    }

    /// Q60: the same command on both sides loses nothing and says nothing;
    /// a user's binding that a default shuts out is still a warning, since
    /// the user's line never runs.
    #[test]
    fn a_rebinding_to_the_same_command_is_silent() {
        let same = "[[mgr.prepend_keymap]]\non = \"T\"\nrun = \"plugin toggle-pane max-preview\"\n";
        let (km, warnings) = Keymap::load_named(&[("/cfg/kura/keymap.toml", same)]);
        assert!(!warnings.iter().any(|w| w.contains("`T`")), "{warnings:?}");
        assert!(!km.overrides.iter().any(|o| o.contains("`T`")), "{:?}", km.overrides);

        let shut_out = "[[mgr.append_keymap]]\non = \"T\"\nrun = \"quit\"\n";
        let (km, warnings) = Keymap::load_named(&[("/cfg/kura/keymap.toml", shut_out)]);
        let w = warnings.iter().find(|w| w.contains("`T`")).unwrap_or_else(|| panic!("{warnings:?}"));
        assert!(w.contains(&format!("({BUILT_IN}) runs, not `quit` (/cfg/kura/keymap.toml)")), "{w}");
        assert!(km.overrides.is_empty(), "{:?}", km.overrides);
    }
}
