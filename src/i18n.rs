//! The words that come from tables: the built-in keymap's descriptions in the
//! screen's language (the help panel, the settings screen's Keys page). The
//! table and the language choice are `ito-i18n`'s; a description a user's
//! keymap.toml writes is shown as written.

use std::sync::OnceLock;

use ito_i18n::Table;

/// The languages with a table, English first.
pub const AVAILABLE: &[(&str, &str)] = &[("en", include_str!("lang/en.toml")), ("ja", include_str!("lang/ja.toml"))];

/// One table per language in [`AVAILABLE`], built on first use, so a lookup per
/// help row costs a hash and the language can change while kura runs.
static TABLES: OnceLock<Vec<(&'static str, Table)>> = OnceLock::new();

fn tables() -> &'static [(&'static str, Table)] {
    TABLES.get_or_init(|| AVAILABLE.iter().map(|(code, _)| (*code, Table::new(AVAILABLE, code).0)).collect())
}

/// The language to use for `common`'s `language` (`auto` follows the OS's).
pub fn resolve(common: Option<&str>, os: Option<&str>) -> String {
    let codes: Vec<&str> = AVAILABLE.iter().map(|(c, _)| *c).collect();
    ito_i18n::resolve(None, common, os, &codes)
}

/// `desc` in `lang`'s words when its table has them, else as written.
pub fn tr_desc(lang: &str, desc: &str) -> String {
    tables().iter().find(|(code, _)| *code == lang).map_or(desc, |(_, t)| t.tr_desc(desc)).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tables_agree_with_english() {
        let bad = ito_i18n::check::translations(AVAILABLE);
        assert!(bad.is_empty(), "{bad:#?}");
    }

    /// Every description of the built-in keymap has its Japanese: a new
    /// key with no translation is shown in English on a Japanese screen.
    #[test]
    fn every_built_in_description_is_in_japanese() {
        let (km, _) = crate::config::Keymap::load(&[]);
        let layers = km.layers();
        let descs: Vec<&str> = layers.iter().flat_map(|(_, b)| b.iter()).map(|b| b.desc.as_str()).filter(|d| !d.is_empty()).collect();
        let missing = ito_i18n::check::key_descriptions(AVAILABLE, &descs);
        assert!(missing.is_empty(), "{missing:#?}");
    }

    #[test]
    fn a_description_reads_in_japanese_and_a_users_own_as_written() {
        assert_eq!(tr_desc("ja", "Move cursor up"), "カーソルを上へ");
        assert_eq!(tr_desc("en", "Move cursor up"), "Move cursor up");
        assert_eq!(tr_desc("ja", "my own words"), "my own words");
    }

    #[test]
    fn auto_follows_the_os_and_an_unknown_language_is_english() {
        assert_eq!(resolve(Some("auto"), Some("ja-JP")), "ja");
        assert_eq!(resolve(None, Some("en-US")), "en");
        assert_eq!(resolve(Some("ja"), Some("en-US")), "ja");
        assert_eq!(resolve(Some("xx"), Some("ja-JP")), "en");
    }
}
