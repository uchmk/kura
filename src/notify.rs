//! Telling of a long job's end while the window is behind another: the code
//! is ito's `[notify]` (`ito_prefs::NotifyCfg`); this is kura's name and icon.

use ito_prefs::{NotifyCfg, DONE_FAILED};

/// Who the notifications are from.
const APP: ito_notify::toast::App = ito_notify::toast::App { id: "uchmk.kura", name: "kura" };

/// kura's icon at `side` pixels, as the window's: the notification shows it.
fn icon(side: u32) -> Vec<u8> {
    crate::app_icon(crate::ICON_SVG, side).map(|i| i.rgba).unwrap_or_default()
}

/// Tell of a job that ended as `event` (`done` or `failed`) in every way `n`
/// turns on for it.
pub fn tell(n: &NotifyCfg, event: &str, title: &str, body: &str) {
    n.tell(&DONE_FAILED, APP, icon, event, title, body);
}

/// "Send a test", as `p` on the settings' page sets things now.
pub fn test(p: &ito_prefs::Notify, what: ito_prefs::Test, title: &str, body: &str) {
    ito_prefs::test_notify(&DONE_FAILED, p, what, APP, icon, title, body);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// kura's icon is drawn for the notification at the side asked for.
    #[test]
    fn the_icon_fills_its_side() {
        assert_eq!(icon(32).len(), 32 * 32 * 4);
    }

    /// A config with no `[notify]` tells of both events through the system,
    /// and keeps quiet in focus mode.
    #[test]
    fn the_default_tells_both_events_through_the_system() {
        let p = NotifyCfg::new(&DONE_FAILED).page(&DONE_FAILED);
        assert_eq!(p.system, [true, true]);
        assert_eq!(p.sound, [false, false]);
        assert!(p.focus);
    }
}
