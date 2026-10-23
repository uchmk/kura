use super::*;
use crate::config::keymap;

/// The spotter and quick look divide the keys the same way.
///
/// `<F3>` leaves the `[mgr]` layer live, so there `j` moves the list and
/// `<A-j>` scrolls what is on show. The spotter had the plain keys on its
/// own rows and `h`/`l` on the files, so the same fingers did different
/// things depending on which was open — and the panel is one screen of
/// facts about one file, so walking the files is the common move.
#[test]
fn the_plain_keys_walk_the_files_and_the_alt_keys_the_panel() {
    let (km, warnings) = keymap::Keymap::load(&[]);
    assert!(warnings.is_empty(), "{warnings:?}");

    let named = |b: &crate::config::keymap::Binding| crate::config::keys::render_seq(&b.on);
    let run = |key: &str| km.spot.iter().find(|b| named(b) == key).unwrap_or_else(|| panic!("`{key}` is not bound in [spot]")).run.clone();
    for key in ["j", "<Down>"] {
        assert_eq!(run(key), vec![Act::Swipe(1)], "`{key}` goes to the next file");
    }
    for key in ["k", "<Up>"] {
        assert_eq!(run(key), vec![Act::Swipe(-1)], "`{key}` goes to the previous file");
    }
    for key in ["l", "<Right>"] {
        assert_eq!(run(key), vec![Act::Enter], "`{key}` goes into the directory");
    }
    for key in ["h", "<Left>"] {
        assert_eq!(run(key), vec![Act::Leave], "`{key}` goes up to the parent");
    }
    for key in ["<A-j>", "<A-Down>"] {
        assert_eq!(run(key), vec![Act::Arrow(Step::Rel(1))], "`{key}` moves down the panel");
    }
    for key in ["<A-k>", "<A-Up>"] {
        assert_eq!(run(key), vec![Act::Arrow(Step::Rel(-1))], "`{key}` moves up the panel");
    }

    // The same split the mgr layer has, which is the point of the change.
    let mgr = |key: &str| km.mgr.iter().find(|b| named(b) == key).map(|b| b.run.clone());
    assert_eq!(mgr("j"), Some(vec![Act::Arrow(Step::Rel(1))]), "`j` moves the list under F3");
    assert!(matches!(mgr("<A-j>").as_deref(), Some([Act::Seek(_)])), "and <A-j> scrolls what is on show",);
    // The horizontal pair is the list's, unchanged, in both.
    assert_eq!(mgr("h"), Some(vec![Act::Leave]));
    assert_eq!(mgr("l"), Some(vec![Act::Enter]));
}
