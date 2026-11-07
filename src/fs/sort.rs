use std::cmp::Ordering;

use serde::Deserialize;

use super::entry::Entry;
use crate::util;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortBy {
    None,
    #[serde(alias = "mtime")]
    Mtime,
    #[serde(alias = "btime")]
    Btime,
    #[serde(alias = "atime")]
    Atime,
    Extension,
    Alphabetical,
    Natural,
    Size,
    Random,
    Type,
    Permissions,
    Usage,
    Owner,
    Dimensions,
}

impl SortBy {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "none" => Self::None,
            "mtime" | "modified" => Self::Mtime,
            "btime" | "created" => Self::Btime,
            "atime" | "accessed" => Self::Atime,
            "extension" | "ext" => Self::Extension,
            "alphabetical" | "alpha" => Self::Alphabetical,
            "natural" => Self::Natural,
            "size" => Self::Size,
            "random" => Self::Random,
            "type" => Self::Type,
            "permissions" => Self::Permissions,
            "usage" => Self::Usage,
            "owner" => Self::Owner,
            "dimensions" => Self::Dimensions,
            _ => return None,
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Mtime => "mtime",
            Self::Btime => "btime",
            Self::Atime => "atime",
            Self::Extension => "extension",
            Self::Alphabetical => "alphabetical",
            Self::Natural => "natural",
            Self::Size => "size",
            Self::Random => "random",
            Self::Type => "type",
            Self::Permissions => "permissions",
            Self::Usage => "usage",
            Self::Owner => "owner",
            Self::Dimensions => "dimensions",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SortSpec {
    pub by: SortBy,
    pub reverse: bool,
    pub dir_first: bool,
    pub sensitive: bool,
}

impl Default for SortSpec {
    fn default() -> Self {
        Self { by: SortBy::Natural, reverse: false, dir_first: true, sensitive: false }
    }
}

impl SortSpec {
    pub fn apply(&self, entries: &mut [Entry]) {
        let by = self.by;
        let sensitive = self.sensitive;
        let rev = self.reverse;
        let dir_first = self.dir_first;

        if by == SortBy::Random {
            let mut seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0x9E3779B97F4A7C15)
                | 1;
            // Fisher-Yates with a xorshift source; no rand dependency needed.
            for i in (1..entries.len()).rev() {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                let j = (seed % (i as u64 + 1)) as usize;
                entries.swap(i, j);
            }
            if dir_first {
                entries.sort_by_key(dir_rank);
            }
            return;
        }

        entries.sort_by(|a, b| {
            // Missing lazy values stay last even when descending or directories first.
            let missing = |e: &Entry| match by {
                SortBy::Usage => e.usage.is_none(),
                SortBy::Owner => e.details.as_ref().is_none_or(|d| d.owner.is_empty()),
                SortBy::Dimensions => e.details.as_ref().is_none_or(|d| d.dimensions.is_none()),
                _ => false,
            };
            match missing(a).cmp(&missing(b)) {
                Ordering::Equal => {}
                o => return o,
            }
            if dir_first {
                match dir_rank(a).cmp(&dir_rank(b)) {
                    Ordering::Equal => {}
                    o => return o,
                }
            }
            let ord = match by {
                SortBy::Type => {
                    let label = |e: &Entry| e.sort_type.clone().unwrap_or_else(|| super::details::detail_type(e, "en"));
                    util::alpha_cmp(&label(a), &label(b), sensitive)
                }
                SortBy::Permissions => super::details::permissions(a).cmp(&super::details::permissions(b)),
                SortBy::Usage => a.usage.cmp(&b.usage),
                SortBy::Owner => {
                    let owner = |e: &Entry| e.details.as_ref().map(|d| d.owner.clone()).unwrap_or_default();
                    util::alpha_cmp(&owner(a), &owner(b), sensitive)
                }
                SortBy::Dimensions => {
                    let pixels = |e: &Entry| e.details.as_ref().and_then(|d| d.dimensions).map(|(w, h)| u64::from(w) * u64::from(h));
                    pixels(a).cmp(&pixels(b))
                }
                SortBy::None => Ordering::Equal,
                SortBy::Mtime => cmp_time(a.modified, b.modified),
                SortBy::Btime => cmp_time(a.created, b.created),
                SortBy::Atime => cmp_time(a.accessed, b.accessed),
                // A measured total when there is one (the usage view), so a
                // folder sorts by what is in it rather than by its own entry.
                SortBy::Size => a.usage.unwrap_or(a.len).cmp(&b.usage.unwrap_or(b.len)),
                SortBy::Extension => match (a.ext.as_deref(), b.ext.as_deref()) {
                    (Some(x), Some(y)) => util::alpha_cmp(x, y, sensitive),
                    (None, Some(_)) => Ordering::Less,
                    (Some(_), None) => Ordering::Greater,
                    (None, None) => Ordering::Equal,
                },
                SortBy::Alphabetical => util::alpha_cmp(&a.name, &b.name, sensitive),
                SortBy::Natural | SortBy::Random => util::natural_cmp(&a.name, &b.name, sensitive),
            };
            let ord = if rev { ord.reverse() } else { ord };
            // Stable tie-break so redraws never shuffle equal keys.
            match ord {
                Ordering::Equal => util::natural_cmp(&a.name, &b.name, sensitive),
                o => o,
            }
        });
    }
}

fn dir_rank(e: &Entry) -> u8 {
    if e.is_dir_like() {
        0
    } else {
        1
    }
}

fn cmp_time(a: Option<std::time::SystemTime>, b: Option<std::time::SystemTime>) -> Ordering {
    match (a, b) {
        (Some(x), Some(y)) => x.cmp(&y),
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

#[cfg(test)]
mod detail_sort_tests {
    use super::*;
    use crate::fs::details::Details;
    fn entry(name: &str, value: u64) -> Entry {
        Entry {
            name: name.into(),
            usage: Some(value),
            details: Some(Details { owner: value.to_string(), dimensions: Some((value as u32, 2)) }),
            ..Default::default()
        }
    }
    fn names(entries: &[Entry]) -> Vec<&str> {
        entries.iter().map(|e| e.name.as_str()).collect()
    }
    #[test]
    fn lazy_sorts_keep_missing_last_and_ties_by_name_in_both_directions() {
        for by in [SortBy::Usage, SortBy::Owner, SortBy::Dimensions] {
            for reverse in [false, true] {
                let mut entries = vec![Entry { name: "missing".into(), ..Default::default() }, entry("b", 1), entry("a", 1), entry("z", 2)];
                SortSpec { by, reverse, dir_first: false, sensitive: false }.apply(&mut entries);
                assert_eq!(
                    names(&entries),
                    if reverse { vec!["z", "a", "b", "missing"] } else { vec!["a", "b", "z", "missing"] },
                    "{by:?}, {reverse}"
                );
            }
        }
    }
    #[test]
    fn permissions_and_types_use_displayed_strings_and_name_ties() {
        let a = Entry { name: "z".into(), ext: Some("png".into()), readonly: false, ..Default::default() };
        let b = Entry { name: "b".into(), ext: Some("json".into()), readonly: true, ..Default::default() };
        let mut c = b.clone();
        c.name = "a".into();
        for by in [SortBy::Type, SortBy::Permissions] {
            for reverse in [false, true] {
                let mut entries = vec![a.clone(), b.clone(), c.clone()];
                SortSpec { by, reverse, dir_first: false, sensitive: false }.apply(&mut entries);
                assert_eq!(names(&entries), if reverse { vec!["z", "a", "b"] } else { vec!["a", "b", "z"] });
            }
        }
    }
    #[test]
    fn missing_values_follow_measured_directories_and_files() {
        let mut absent = Entry { name: "missing-folder".into(), kind: super::super::Kind::Dir, ..Default::default() };
        for by in [SortBy::Usage, SortBy::Owner, SortBy::Dimensions] {
            for reverse in [false, true] {
                absent.details = Some(Details::default());
                let mut entries = vec![absent.clone(), entry("file", 1)];
                SortSpec { by, reverse, dir_first: true, sensitive: false }.apply(&mut entries);
                assert_eq!(names(&entries), vec!["file", "missing-folder"]);
            }
        }
    }
    #[test]
    fn all_new_names_parse_in_commands_and_serde() {
        #[derive(Deserialize)]
        struct Config {
            by: SortBy,
        }
        for name in ["type", "permissions", "usage", "owner", "dimensions"] {
            let by = SortBy::parse(name).unwrap();
            assert_eq!(by.label(), name);
            assert_eq!(toml::from_str::<Config>(&format!("by = '{name}'")).unwrap().by, by);
            assert!(
                matches!(crate::config::cmd::parse(&format!("sort {name}")), crate::config::cmd::Act::Sort { by: Some(b), .. } if b == by)
            );
        }
    }
}
