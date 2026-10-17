//! Open a bug report with the machine's own answers already filled in.
//!
//! The generic part -- the pre-filled URL, the OS line, the context text, the
//! home-folder scrubbing and the shared issue form -- is `ito-bugreport`. What
//! stays here is what is kura's: its name and repository, the template name,
//! and gathering what kura knew (the last error, how it was drawing, the config
//! files it read) for the context.

#[cfg(test)]
use ito_bugreport::Field;
use ito_bugreport::{Context, Spec};

/// Where the form lives. A fork should point this at its own tracker; the
/// program cannot know it has been forked, and silently filing reports against
/// somebody else's repository would be worse than not opening a browser.
const REPO: &str = "https://github.com/uchmk/kura";

/// The template's filename, from `.github/ISSUE_TEMPLATE/`.
const TEMPLATE: &str = "bug_report.yml";

/// Folder names that start a config file's short name in the report.
const ANCHORS: &[&str] = &["yazi", "kura"];

fn spec() -> Spec<'static> {
    Spec { app: "kura", version: env!("CARGO_PKG_VERSION"), repo: REPO, template: TEMPLATE }
}

/// The URL that opens a pre-filled report: the version and OS lines, and
/// any `extra` fields as `(id, text)`, ids from the template.
pub fn url(extra: &[(&str, String)]) -> String {
    ito_bugreport::url(&spec(), extra)
}

/// What `--version` prints, and for the same reason: with more than one Windows
/// binary in circulation, which one is running is the first thing to settle.
pub fn version_line() -> String {
    ito_bugreport::version_line("kura", env!("CARGO_PKG_VERSION"))
}

/// The OS line, as the form's own PowerShell snippet would print it.
pub fn os_line() -> String {
    ito_bugreport::os_line()
}

/// `text` with the home folder written as `~` (Q64: a report never carries the
/// user's name).
pub(crate) fn without_home(text: &str, home: Option<&std::path::Path>) -> String {
    ito_bugreport::without_home(text, home)
}

/// What the program knew when `<F12>` was pressed, beyond the version and
/// the OS (Q64): the last error it raised, how it was drawing, and which
/// config files it read -- by name, `yazi\keymap.toml`, never by path, since
/// a path carries the user's name onto a public tracker.
pub fn context(last_error: Option<&str>, loaded: &[std::path::PathBuf]) -> String {
    let info = crate::runinfo::load();
    let home = ito_common::home_dir();
    ito_bugreport::context(&Context {
        last_error,
        home: home.as_deref(),
        // `RunInfo::rendering` is [backend, device, adapter]; the context takes
        // [adapter, backend, device].
        rendering: info.as_ref().filter(|i| !i.adapter.is_empty()).map(|i| [i.adapter.as_str(), i.backend.as_str(), i.device.as_str()]),
        scale: info.as_ref().map(|i| i.ppp),
        config: loaded,
        anchors: ANCHORS,
        rows: &[],
    })
}

/// kura's own fields on the shared issue form.
#[cfg(test)]
const FIELDS: &[Field] = &[
    Field {
        id: "files",
        label: "Files or folders involved / 対象のファイルやフォルダ",
        description: "中身そのものではなく、性質を書いてください。長さ・件数・名前の癖で通る経路が変わります。4000 行のソース、500 件のフォルダ、名前に空白や絵文字を含むもの、UNC パス、OneDrive 配下、など。",
        required: false,
    },
    Field {
        id: "config",
        label: "Config / 設定",
        description: "yazi の設定（`yazi.toml` / `keymap.toml` / `theme.toml`）や kura の `config.toml` を使っている場合、関係しそうな部分を貼ってください。既定のままなら「既定」と。",
        required: false,
    },
];

/// The whole `.github/ISSUE_TEMPLATE/bug_report.yml`, made from ito's shared form.
#[cfg(test)]
pub fn form() -> String {
    ito_bugreport::template("kura", FIELDS)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The form on disk is the one ito generates (plus kura's fields). To
    /// rewrite it after a change: `cargo test write_the_form -- --ignored`.
    const FORM: &str = include_str!("../.github/ISSUE_TEMPLATE/bug_report.yml");

    #[test]
    #[ignore = "rewrites .github/ISSUE_TEMPLATE/bug_report.yml"]
    fn write_the_form() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/ISSUE_TEMPLATE").join(TEMPLATE);
        std::fs::write(path, form()).unwrap();
    }

    #[test]
    fn the_form_on_disk_is_the_generated_one() {
        assert_eq!(FORM.replace("\r\n", "\n"), form(), "run `cargo test write_the_form -- --ignored`");
    }

    /// Q64: a config file is named by its folder and name, never its path, and
    /// the home folder never reaches the report.
    #[test]
    fn the_context_names_files_without_their_path() {
        let sep = std::path::MAIN_SEPARATOR;
        let p = |s: &str| std::path::PathBuf::from(s.replace('/', &sep.to_string()));
        let c = context(Some("Copy: a.txt: denied"), &[p("/home/someone/.config/kura/config.toml")]);
        assert!(c.starts_with("Last error: Copy: a.txt: denied\n"), "{c}");
        assert!(c.ends_with(&format!("Config: kura{sep}config.toml")), "{c}");
        assert!(!c.contains("someone"), "no home directory: {c}");
    }

    /// Row 26.2 of TESTING.md, in part: every heading on the form is in
    /// English and Japanese, `What happened / 何が起きたか`, the form's own
    /// title included (Q63).
    #[test]
    fn every_heading_on_the_form_is_in_both_languages() {
        let headings: Vec<&str> =
            FORM.lines().filter_map(|l| l.trim_start().strip_prefix("label: ").or_else(|| l.strip_prefix("name: "))).collect();
        assert!(headings.len() >= 8, "the form's title and its fields: {headings:?}");
        for h in headings {
            let h = h.trim_matches('"');
            let (en, ja) = h.split_once(" / ").unwrap_or_else(|| panic!("{h:?} has no ` / `"));
            assert!(en.is_ascii() && !en.trim().is_empty(), "English first: {h:?}");
            assert!(!ja.is_ascii(), "Japanese second: {h:?}");
        }
    }

    /// Row 26.2 of TESTING.md, in part: each field the link fills in is one
    /// the form has. GitHub drops a parameter it does not know without a word,
    /// so a field renamed in the form would simply come up empty.
    #[test]
    fn the_fields_the_link_fills_are_the_forms_own() {
        let ids = ito_bugreport::field_ids(FORM);
        let u = url(&[("keys", "j".into()), ("context", "Config: defaults only".into())]);
        let (head, query) = u.split_once('?').unwrap();
        assert_eq!(head, format!("{REPO}/issues/new"));
        assert!(query.contains(&format!("template={TEMPLATE}&")), "{u}");
        for (name, _) in query.split('&').filter_map(|p| p.split_once('=')).filter(|(n, _)| *n != "template") {
            assert!(ids.iter().any(|i| i == name), "{name:?} is not a field of the form: {ids:?}");
        }
        for id in ["version", "os", "keys", "context"] {
            assert!(query.contains(&format!("&{id}=")), "{id} is filled: {u}");
        }
    }

    #[test]
    fn the_version_line_names_the_architecture() {
        let v = version_line();
        assert!(v.contains(env!("CARGO_PKG_VERSION")), "{v}");
        assert!(v.contains(std::env::consts::ARCH), "{v}");
    }
}
