use super::*;
use crate::preview::TocEntry;

fn app() -> App {
    let ctx = egui::Context::default();
    App::new(Config::load(), std::env::temp_dir(), ctx)
}

/// An outline entry near the end of a file must not scroll past it.
///
/// The draw paints from whatever `preview_offset` says and corrects the
/// number after, so an offset beyond the content costs one wrong frame.
/// A tap hides that; a held key turns it into a flicker. `seek` was
/// clamped for this exact reason and the outline's own arrow was missed,
/// which is why it survived: it shows only on the last entry or two, and
/// only in a file whose last heading sits near the end — so it reproduces
/// on one document and not on the next.
#[test]
fn it_stops_where_the_pane_does() {
    let mut a = app();
    let toc = |line: usize| TocEntry { level: 1, label: format!("h{line}"), line };
    a.preview.state = PreviewState::Ready(Payload::Text {
        lines: Vec::new(),
        map: Vec::new(),
        extent: crate::preview::Extent { truncated: false, total: 500, ..Default::default() },
        outline: vec![toc(0), toc(120), toc(480)],
    });
    // The furthest the pane can be scrolled, as the last draw worked out.
    a.preview.max_offset = 300;
    a.preview.outline = Some(0);

    // A middle entry is inside the range and lands on its own line.
    assert!(a.outline_act(&Act::Arrow(Step::Rel(1))));
    assert_eq!(a.tabs[a.active].preview_offset, 120);

    // The last one is past the end, and stops at the end instead.
    assert!(a.outline_act(&Act::Arrow(Step::Rel(1))));
    assert_eq!(a.preview.outline, Some(2), "the cursor still reaches it");
    assert_eq!(a.tabs[a.active].preview_offset, 300, "but the pane is not asked to draw from beyond the content",);

    // Held down at the end: every extra press has to be a no-op.
    for _ in 0..20 {
        a.outline_act(&Act::Arrow(Step::Rel(1)));
    }
    assert_eq!(a.preview.outline, Some(2));
    assert_eq!(a.tabs[a.active].preview_offset, 300, "no frame is drawn past the end");
}
