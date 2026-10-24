//! The colours of a syntax-coloured preview, taken from the app's own theme.
//!
//! The grammar only says what kind of token a stretch of text is (see
//! `ito-syntax`); which colour a kind gets is the theme's business, so a
//! light window and a dark one both read well. yazi's `[mgr] syntect_theme`
//! is still read from `theme.toml` but no longer picks anything (v0.140.0).

use ito_syntax::Tok;
use ito_theme::Colors;

use crate::config::theme::Theme;

type Rgb = [u8; 3];

/// Colours for token kinds and for the Markdown page, as plain bytes so the
/// preview worker can be handed a copy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hues {
    pub keyword: Rgb,
    pub string: Rgb,
    pub comment: Rgb,
    pub number: Rgb,
    pub constant: Rgb,
    pub ty: Rgb,
    pub func: Rgb,
    pub heading: Rgb,
    pub link: Rgb,
    /// Inline code and the body of a fence whose language is not known.
    pub code: Rgb,
    pub marker: Rgb,
    /// Note, Tip, Important, Warning, Caution.
    pub alerts: [Rgb; 5],
}

fn rgb(c: egui::Color32) -> Rgb {
    [c.r(), c.g(), c.b()]
}

/// `a` darkened toward black, for a yellow that has to be read as text.
fn darker(c: egui::Color32) -> egui::Color32 {
    let k = |v: u8| (f32::from(v) * 0.7).round() as u8;
    egui::Color32::from_rgb(k(c.r()), k(c.g()), k(c.b()))
}

impl Hues {
    /// From a set of theme colours (`light` says what the text sits on).
    pub fn from_colors(c: &Colors) -> Self {
        // A light ground's yellow is a mark's; as text it is darkened.
        let warm = if c.light { darker(c.wait) } else { c.wait };
        Self {
            keyword: rgb(c.magenta),
            string: rgb(c.done),
            comment: rgb(c.dim),
            number: rgb(warm),
            constant: rgb(warm),
            ty: rgb(c.run),
            func: rgb(c.blue),
            heading: rgb(c.blue),
            link: rgb(c.run),
            code: rgb(c.done),
            marker: rgb(c.magenta),
            alerts: [rgb(c.blue), rgb(c.done), rgb(c.magenta), rgb(warm), rgb(c.err)],
        }
    }

    /// From the window's theme. When a `theme.toml` ground turns the window
    /// the other way from the common theme, the built-in pair of that kind is
    /// used instead (#396), the way the highlighter's was.
    pub fn new(theme: &Theme) -> Self {
        let light = theme.light();
        if theme.common.light == light {
            return Self::from_colors(&theme.common);
        }
        let builtin = ito_theme::builtin();
        let pick = builtin.iter().find(|t| t.colors.light == light).unwrap_or(&builtin[0]);
        Self::from_colors(&pick.colors)
    }

    pub fn tok(&self, tok: Tok) -> Rgb {
        match tok {
            Tok::Keyword => self.keyword,
            Tok::Str => self.string,
            Tok::Comment => self.comment,
            Tok::Number => self.number,
            Tok::Const => self.constant,
            Tok::Type => self.ty,
            Tok::Func => self.func,
        }
    }
}

impl Default for Hues {
    /// `tsumugi Dark`, what no `common.toml` gives.
    fn default() -> Self {
        Self::from_colors(&ito_theme::builtin()[0].colors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_light_and_a_dark_window_get_their_own_hues() {
        let builtin = ito_theme::builtin();
        let dark = Hues::from_colors(&builtin[0].colors);
        let light = Hues::from_colors(&builtin[1].colors);
        assert_ne!(dark, light);
        assert_eq!(dark, Hues::default());
        assert_eq!(dark.keyword, rgb(builtin[0].colors.magenta));
        // Yellow is darkened on a light ground.
        assert!(light.number[0] < rgb(builtin[1].colors.wait)[0]);
    }

    #[test]
    fn a_ground_the_file_turns_over_brings_the_other_pair() {
        let named = |n: &str| ito_theme::builtin().into_iter().find(|t| t.name == n).unwrap().colors;
        let t = Theme::layered(
            &named("tsumugi Dark"),
            vec![toml::from_str("[app]\noverall = { bg = \"#ffffff\", fg = \"#222222\" }\n").unwrap()],
        );
        assert!(t.light() && !t.common.light);
        assert_eq!(Hues::new(&t), Hues::from_colors(&named("tsumugi Light")));
        assert_eq!(Hues::new(&Theme::default()), Hues::default());
    }

    #[test]
    fn every_kind_has_a_colour_apart_from_the_text() {
        let h = Hues::default();
        for tok in [Tok::Keyword, Tok::Str, Tok::Comment, Tok::Number, Tok::Const, Tok::Type, Tok::Func] {
            assert_ne!(h.tok(tok), rgb(ito_theme::builtin()[0].colors.fg), "{tok:?}");
        }
    }
}
