use super::*;
use crate::fs::entry::{Entry, Kind, Linemode};

/// Q32 / 31.9: in `linemode size`, a folder's row asks for its children to
/// be counted -- but a host's rows are shares, and each count would be a
/// trip across the network. The same rows in an ordinary folder are counted.
#[test]
fn a_hosts_shares_are_not_counted() {
    let listing = |at: &Path| {
        let entries: Vec<Entry> = ["Backup", "cache"]
            .iter()
            .map(|n| Entry { path: at.join(n), name: n.to_string(), kind: Kind::Dir, ..Default::default() })
            .collect();
        crate::core::folder::Folder::from_entries(at.to_path_buf(), Arc::new(entries), true)
    };
    let ctx = egui::Context::default();
    let mut a = App::new(Config::load(), std::env::temp_dir(), ctx);
    a.tabs[a.active].linemode = Linemode::Size;
    a.tabs[a.active].page_rows = 10;

    // A `\\host` is a host only on Windows; elsewhere `//x` is a path.
    #[cfg(windows)]
    {
        let host = PathBuf::from(r"\\fileserver");
        a.tabs[a.active].cwd = host.clone();
        a.tabs[a.active].current = listing(&host);
        a.ensure_dir_sizes();
        assert!(a.counted.is_empty(), "nothing asked of the network: {:?}", a.counted);
    }

    let dir = crate::util::test_dir("host-sizes");
    a.tabs[a.active].cwd = dir.clone();
    a.tabs[a.active].current = listing(&dir);
    a.ensure_dir_sizes();
    assert_eq!(a.counted.len(), 2, "an ordinary folder's folders are still counted");
}
