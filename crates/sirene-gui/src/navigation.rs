//! Moving around the drawing: fitting on open, zooming to the selection, panning with the
//! arrow keys, and selecting by keyboard (GUISHE-8).

use gpui::{Bounds, Context, Pixels, Point, Size, Window, point, px};
use sirene_core::scene::{Owner, Pt, Scene};

use crate::drawing::Item;
use crate::viewer::{MAX_ZOOM, MIN_ZOOM, Viewer};
use crate::{Pan, SelectAll, SelectNext, SelectPrevious, ZoomToSelection};

/// Room left around the selection by ZoomToSelection, in screen pixels.
const MARGIN: f32 = 40.0;
/// ZoomToSelection never zooms past this.
pub const SELECTION_ZOOM: f32 = 2.0;
/// How far an arrow key pans, and with shift.
pub const PAN_STEP: f32 = 40.0;
pub const PAN_FAR: f32 = 200.0;

impl Viewer {
    /// The pan that puts scene point `c` in the middle of `view` at `zoom`.
    fn pan_to(&mut self, c: Pt, zoom: f32, view: Bounds<Pixels>, window: &Window) -> Point<Pixels> {
        let scene = self.scene(window);
        let free = |side: Pixels, len: f32| (side - px(len * zoom)).max(px(0.0));
        let free = point(
            free(view.size.width, scene.width),
            free(view.size.height, scene.height),
        );
        point(
            view.size.width / 2.0 - free.x / 2.0 - px(c.x * zoom),
            view.size.height / 2.0 - free.y / 2.0 - px(c.y * zoom),
        )
    }

    /// The scene box around everything drawn for `items`.
    fn scene_bounds(&mut self, items: &[Item], window: &Window) -> Option<(Pt, Pt)> {
        let scene = self.scene(window);
        items
            .iter()
            .filter_map(|i| self.owner(i))
            .filter_map(|o| scene.bounds(o))
            .reduce(|(a, b), (c, d)| {
                (
                    Pt::new(a.x.min(c.x), a.y.min(c.y)),
                    Pt::new(b.x.max(d.x), b.y.max(d.y)),
                )
            })
    }

    /// Zooms `scene` to fit a drawing area of `view` when it is larger, the first time the area
    /// is known after a document is shown; smaller drawings stay at 100%. Returns the zoom and
    /// pan to draw with.
    pub(crate) fn fit_on_open(
        &mut self,
        scene: &Scene,
        view: Size<Pixels>,
        cx: &mut Context<Self>,
    ) -> (f32, Point<Pixels>) {
        if self.fit_pending && view.width > px(0.0) && view.height > px(0.0) {
            self.fit_pending = false;
            let fit =
                (f32::from(view.width) / scene.width).min(f32::from(view.height) / scene.height);
            if fit < 1.0 {
                log::debug!("fitting the drawing at {:.0}%", fit * 100.0);
                self.pan = Point::default();
                self.set_zoom(fit, cx);
            }
        }
        (self.zoom, self.pan)
    }

    /// Zooms and pans so the selection fills the drawing area, up to 200%.
    pub(crate) fn zoom_to_selection(
        &mut self,
        _: &ZoomToSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let items = self.selection.clone();
        let Some((lo, hi)) = self.scene_bounds(&items, window) else {
            return;
        };
        let view = self.viewport.get();
        let room = |side: Pixels| (f32::from(side) - 2.0 * MARGIN).max(1.0);
        let zoom = (room(view.size.width) / (hi.x - lo.x).max(1.0))
            .min(room(view.size.height) / (hi.y - lo.y).max(1.0))
            .clamp(MIN_ZOOM, SELECTION_ZOOM.min(MAX_ZOOM));
        let mid = Pt::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0);
        self.pan = self.pan_to(mid, zoom, view, window);
        self.set_zoom(zoom, cx);
    }

    /// Pans so `item` is in the middle of the drawing area, when any of it is outside.
    pub fn reveal(&mut self, item: &Item, window: &Window, cx: &mut Context<Self>) {
        let Some((lo, hi)) = self.item_bounds(item, window) else {
            return;
        };
        let view = self.viewport.get();
        let (vlo, vhi) = (
            view.origin,
            view.origin + point(view.size.width, view.size.height),
        );
        if lo.x >= vlo.x && lo.y >= vlo.y && hi.x <= vhi.x && hi.y <= vhi.y {
            return;
        }
        let Some((lo, hi)) = self.scene_bounds(std::slice::from_ref(item), window) else {
            return;
        };
        let mid = Pt::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0);
        self.pan = self.pan_to(mid, self.zoom, view, window);
        cx.notify();
    }

    /// Moves the drawing, leaving the keys to the label field while it is open.
    pub(crate) fn pan_action(&mut self, pan: &Pan, _: &mut Window, cx: &mut Context<Self>) {
        if self.label.is_some() {
            cx.propagate();
            return;
        }
        self.pan += point(px(pan.x), px(pan.y));
        cx.notify();
    }

    /// Selects every node, then every edge.
    pub(crate) fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        if self.label.is_some() {
            cx.propagate();
            return;
        }
        let chart = &self.doc.chart;
        let items = (chart.nodes.iter().map(|n| Item::Node(n.id.clone())))
            .chain((0..chart.edges.len()).map(Item::Edge))
            .collect();
        self.select(items, cx);
    }

    /// Selects the node `step` after the last selected one, in source order, wrapping around,
    /// and brings it into view.
    fn select_step(&mut self, step: isize, window: &mut Window, cx: &mut Context<Self>) {
        if self.label.is_some() {
            cx.propagate();
            return;
        }
        let n = self.doc.chart.nodes.len();
        if n == 0 {
            return;
        }
        let current = self
            .selection
            .iter()
            .rev()
            .find_map(|i| match self.owner(i) {
                Some(Owner::Node(k)) => Some(k),
                _ => None,
            });
        let next = match current {
            Some(k) => (k as isize + step).rem_euclid(n as isize) as usize,
            None if step > 0 => 0,
            None => n - 1,
        };
        let item = Item::Node(self.doc.chart.nodes[next].id.clone());
        self.select(vec![item.clone()], cx);
        self.reveal(&item, window, cx);
    }

    pub(crate) fn select_next(
        &mut self,
        _: &SelectNext,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_step(1, window, cx);
    }

    pub(crate) fn select_previous(
        &mut self,
        _: &SelectPrevious,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_step(-1, window, cx);
    }
}
