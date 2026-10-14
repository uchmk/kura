//! Telling of a long job's end while the window is behind another, the ways
//! `[notify]` picks (uchmk's common spec, Notifications): the system's own
//! notification with kura's icon, a sound, and a webhook. And `[notify]` as
//! the settings' Notifications page (`ito_prefs::notify_page`) shows it.

use crate::config::{NotifyCfg, NOTIFY_EVENTS};

/// Who the notifications are from.
const APP: ito_notify::toast::App = ito_notify::toast::App { id: "uchmk.kura", name: "kura" };

/// kura's icon at `side` pixels, as the window's: the notification shows it.
fn icon(side: u32) -> Vec<u8> {
    crate::app_icon(crate::ICON_SVG, side).map(|i| i.rgba).unwrap_or_default()
}

/// Tell of a job that ended as `event` (one of [`NOTIFY_EVENTS`]) in every
/// way `n` turns on for it.
pub fn tell(n: &NotifyCfg, event: &str, title: &str, body: &str) {
    let on = |list: &[String]| list.iter().any(|e| e == event);
    let sound = if event == "failed" { &n.sound_failed } else { &n.sound_done };
    send(n, on(&n.system), on(&n.sound).then_some(sound.as_str()), on(&n.webhook_states), event == "failed", title, body);
}

/// "Send a test": a finished job's notice, through all three ways (the
/// webhook when it has an address), as `p` on the page sets them now.
pub fn test(p: &ito_prefs::Notify, title: &str, body: &str) {
    let n = NotifyCfg {
        focus_mode: p.focus,
        webhook: p.webhook_url.trim().to_owned(),
        webhook_format: p.webhook_format.clone(),
        ..NotifyCfg::default()
    };
    send(&n, true, p.sounds.first().map(String::as_str), true, false, title, body);
}

fn send(n: &NotifyCfg, system: bool, sound: Option<&str>, webhook: bool, failed: bool, title: &str, body: &str) {
    if system {
        ito_notify::toast::show_with_icon(APP, icon, title, body);
    }
    if let Some(name) = sound {
        if !(n.focus_mode && ito_notify::sound::quiet_time()) {
            ito_notify::sound::play(name);
        }
    }
    if webhook && ito_notify::is_http(&n.webhook) {
        let level = if failed { ito_notify::webhook::Level::Error } else { ito_notify::webhook::Level::Done };
        ito_notify::webhook::send(n.webhook.clone(), n.webhook_format.clone(), level, title.to_owned(), body.to_owned(), String::new());
    }
}

impl NotifyCfg {
    /// As the settings' Notifications page shows it: a column per event of
    /// [`NOTIFY_EVENTS`], in that order.
    pub fn page(&self) -> ito_prefs::Notify {
        let col = |list: &[String]| NOTIFY_EVENTS.iter().map(|e| list.iter().any(|x| x == e)).collect();
        ito_prefs::Notify {
            system: col(&self.system),
            sound: col(&self.sound),
            webhook: col(&self.webhook_states),
            sounds: vec![self.sound_done.clone(), self.sound_failed.clone()],
            focus: self.focus_mode,
            webhook_url: self.webhook.clone(),
            webhook_format: self.webhook_format.clone(),
        }
    }

    /// The `[notify]` keys to write, as TOML, so that the file reads as the
    /// page's `p`: only those that changed.
    pub fn edits(&self, p: &ito_prefs::Notify) -> Vec<(&'static str, String)> {
        let list = |col: &[bool]| {
            let on: Vec<String> = NOTIFY_EVENTS.iter().zip(col).filter(|(_, on)| **on).map(|(e, _)| format!("\"{e}\"")).collect();
            format!("[{}]", on.join(", "))
        };
        let quote = |s: &str| toml::Value::String(s.to_owned()).to_string();
        let was = self.page();
        let mut out = Vec::new();
        for (key, a, b) in
            [("system", &was.system, &p.system), ("sound", &was.sound, &p.sound), ("webhook_states", &was.webhook, &p.webhook)]
        {
            if a != b {
                out.push((key, list(b)));
            }
        }
        for (i, key) in ["sound_done", "sound_failed"].into_iter().enumerate() {
            if let Some(now) = p.sounds.get(i).filter(|now| Some(*now) != was.sounds.get(i)) {
                out.push((key, quote(now)));
            }
        }
        if p.focus != was.focus {
            out.push(("focus_mode", p.focus.to_string()));
        }
        if p.webhook_url != was.webhook_url {
            out.push(("webhook", quote(&p.webhook_url)));
        }
        if p.webhook_format != was.webhook_format {
            out.push(("webhook_format", quote(&p.webhook_format)));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The page shows the file's lists as columns, and writes back only what
    /// was changed, as the file spells it.
    #[test]
    fn the_page_writes_only_what_changed() {
        let was = NotifyCfg::default();
        let mut p = was.page();
        assert_eq!(p.system, [true, true]);
        assert_eq!(p.sound, [false, false]);
        assert!(was.edits(&p).is_empty());
        p.system[1] = false;
        p.sound[1] = true;
        p.sounds[1] = "low".into();
        p.focus = false;
        p.webhook_url = "http://127.0.0.1:8765/hook".into();
        assert_eq!(
            was.edits(&p),
            [
                ("system", "[\"done\"]".to_owned()),
                ("sound", "[\"failed\"]".to_owned()),
                ("sound_failed", "\"low\"".to_owned()),
                ("focus_mode", "false".to_owned()),
                ("webhook", "\"http://127.0.0.1:8765/hook\"".to_owned()),
            ]
        );
    }

    /// kura's icon is drawn for the notification at the side asked for.
    #[test]
    fn the_icon_fills_its_side() {
        assert_eq!(icon(32).len(), 32 * 32 * 4);
    }
}
