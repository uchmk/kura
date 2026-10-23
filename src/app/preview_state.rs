//! What the preview pane shows and how a look at a file is asked for.

use super::*;

pub enum PreviewState {
    Empty,
    Loading,
    Dir(Folder),
    Ready(Payload),
}

#[derive(Clone)]
pub struct CachedPreview {
    pub payload: Payload,
    /// The uploaded image, so a cache hit can draw without re-uploading.
    pub texture: Option<egui::TextureHandle>,
}

pub struct PreviewSlot {
    pub key: Option<preview::Key>,
    pub state: PreviewState,
    pub texture: Option<egui::TextureHandle>,
    pub request_id: u64,
    pub pending_since: Option<Instant>,
    /// A request is out and its answer has not come back. Mostly that shows
    /// as `Loading`, but a re-layout or the next page of an external preview
    /// keeps the old one up meanwhile, and `settled()` has to see past it
    /// (#166: `<A-j><Shot:p2>` pictured page 1 again).
    pub in_flight: bool,
    /// What is on screen is the uncoloured first screen the worker sent ahead
    /// of the whole one (`<State:>` says `preview: early`).
    pub early: bool,
    /// A body search has already scrolled to its first match on the early
    /// payload, so the whole one that follows keeps the reader where they are.
    pub placed: bool,
    pub cache: Lru<preview::Key, CachedPreview>,
    /// Size of the preview pane in pixels, used when decoding images.
    pub box_size: (u32, u32),
    /// Where the preview was drawn in the last frame, for `<Shot:name@preview>`.
    pub rect: Option<egui::Rect>,
    /// The largest `preview_offset` the pane can usefully show, as the last
    /// draw worked it out. `seek` stops here, so a scroll past the end never
    /// becomes a frame drawn from beyond the content.
    pub max_offset: usize,
    /// Text columns across the pane, which rendered Markdown wraps to.
    pub cols: u16,
    /// Which picture of the file a configured previewer is showing: a PDF's
    /// page, a video's second. Belongs to the file, so it resets on the way to
    /// the next one — landing on page nine of a two-page document would be a
    /// puzzle with no visible cause.
    pub n: i64,
    /// The last `n` that produced a picture, so that stepping past the end can
    /// step back to it. There is no page count to clamp against; the end is
    /// only ever found by reaching it.
    pub n_ok: Option<i64>,
    /// The outline entry under the cursor while the keys drive the preview's
    /// outline rather than the file list.
    pub outline: Option<usize>,
    /// A file whose outline should take the keys as soon as its preview lands
    /// (`enter` was pressed before it had loaded).
    pub outline_wanted: Option<PathBuf>,
    /// How an image is scaled. `None` is fit-to-pane, where every image starts.
    pub zoom: Option<f32>,
    /// How far a zoomed image has been dragged from centered, in points.
    pub pan: egui::Vec2,
    /// The scale *fit* comes to for the image on screen. Written by the pane
    /// each frame, since only it knows how big it is; read when zooming away
    /// from fit so the first step does not jump.
    pub fit: f32,
}

/// The scale that fits an image of `w` × `h` into `avail`, never magnifying: a
/// small image sits at its own size rather than being blown up unasked.
pub fn image_fit(avail: egui::Vec2, w: f32, h: f32) -> f32 {
    if w <= 0.0 || h <= 0.0 {
        return 1.0;
    }
    (avail.x / w).min(avail.y / h).min(1.0)
}

/// The smallest and largest an image may be scaled to.
pub(super) const ZOOM_MIN: f32 = 0.05;
pub(super) const ZOOM_MAX: f32 = 32.0;

/// Zoom about `pointer`, so whatever is under the cursor stays under it.
/// Without this the image slides out from under the eye as it grows.
pub fn zoom_at(zoom: f32, pan: egui::Vec2, center: egui::Pos2, pointer: egui::Pos2, factor: f32) -> (f32, egui::Vec2) {
    let next = (zoom * factor).clamp(ZOOM_MIN, ZOOM_MAX);
    if zoom <= 0.0 {
        return (next, pan);
    }
    // Where the pointer is relative to the image's own centre.
    let d = pointer - center - pan;
    (next, pan + d * (1.0 - next / zoom))
}

/// Keep an image from being dragged off the pane. Smaller than the pane, it has
/// nowhere to go and stays centred.
pub fn clamp_pan(pan: egui::Vec2, shown: egui::Vec2, avail: egui::Vec2) -> egui::Vec2 {
    let slack = ((shown - avail) * 0.5).max(egui::Vec2::ZERO);
    egui::Vec2::new(pan.x.clamp(-slack.x, slack.x), pan.y.clamp(-slack.y, slack.y))
}

/// The box the preview worker should decode an image into for the zoom in
/// force. The worker renders to the pane's size, so magnifying past that would
/// show a blurred copy; asking for a bigger decode is what the
/// `box_size` in [`preview::Key`] is for, and the cache keeps both.
///
/// Stepping in powers of two means dragging the zoom around costs a handful of
/// decodes rather than one per frame, and the cap keeps a huge photo from
/// asking for a gigabyte of texture.
///
/// What matters is the zoom against `fit`, not the zoom on its own: a photo that
/// fits at 5% is already asking for twice the pane's detail at 10%, while a
/// small icon at 2x is asking for nothing that exists.
pub fn zoom_box(pane: (u32, u32), zoom: Option<f32>, fit: f32) -> (u32, u32) {
    const CAP: u32 = crate::preview::MAX_DECODE;
    let want = match zoom {
        Some(z) if fit > 0.0 => (z / fit).max(1.0),
        _ => 1.0,
    };
    let step = want.log2().ceil().exp2();
    (((pane.0 as f32 * step) as u32).min(CAP), ((pane.1 as f32 * step) as u32).min(CAP))
}

impl Default for PreviewSlot {
    fn default() -> Self {
        Self {
            key: None,
            state: PreviewState::Empty,
            texture: None,
            request_id: 0,
            pending_since: None,
            in_flight: false,
            early: false,
            placed: false,
            cache: Lru::new(24),
            box_size: (900, 900),
            rect: None,
            max_offset: 0,
            cols: 80,
            n: 0,
            n_ok: None,
            outline: None,
            outline_wanted: None,
            zoom: None,
            pan: egui::Vec2::ZERO,
            fit: 1.0,
        }
    }
}
