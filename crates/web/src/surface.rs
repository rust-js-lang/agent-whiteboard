//! Transient pointer drafts, camera and replay never modify the saved document.
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::panic::catch_unwind;
use std::rc::Rc;

use react::{Dispatch, SetStateAction};
use serde::Serialize;
use webapi::{HTMLCanvasElement, PointerEvent, document, window};
use whiteboard_core::{
    Brush as InkBrush, Clip, ColorStop, Drawing, Element, Geometry, Gradient, Journal, Operation,
    Paint, Point, Segment, Snapshot, Style, Target, replay_snapshot,
};

use crate::render::{self, Mapping, Renderer, Viewport};

#[derive(Clone, Copy, PartialEq)]
pub enum Tool {
    Select,
    Pen,
    Eraser,
    Line,
    Rectangle,
    Ellipse,
    Text,
    Hand,
}
impl Tool {
    pub fn name(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Pen => "Pen",
            Self::Eraser => "Eraser",
            Self::Line => "Line",
            Self::Rectangle => "Rectangle",
            Self::Ellipse => "Ellipse",
            Self::Text => "Text",
            Self::Hand => "Hand",
        }
    }
    pub fn class(self) -> &'static str {
        match self {
            Self::Select => "select",
            Self::Pen => "pen",
            Self::Eraser => "eraser",
            Self::Line => "line",
            Self::Rectangle => "rectangle",
            Self::Ellipse => "ellipse",
            Self::Text => "text",
            Self::Hand => "hand",
        }
    }
}
#[derive(Clone, Copy, PartialEq)]
pub enum PaintMode {
    Solid,
    Linear,
    Radial,
}
#[derive(Clone)]
pub struct BrushSettings {
    pub tool: Tool,
    pub color: String,
    pub secondary: String,
    pub width: f64,
    pub filled: bool,
    pub opacity: f64,
    pub ink: InkBrush,
    pub paint: PaintMode,
    pub clip: Option<Clip>,
}
struct Pointer {
    id: i32,
    brush: BrushSettings,
    start: Point,
    screen: (f64, f64),
    view: Viewport,
    points: Vec<Point>,
    element: Option<Element>,
}
#[derive(Serialize)]
pub struct PlaybackResult {
    pub cancelled: bool,
}
struct Replay {
    journal: Journal,
    started: Option<f64>,
    duration: f64,
    last_step: Option<u32>,
    snapshot: Snapshot,
    next: Snapshot,
    resolve: &'static dyn Fn(PlaybackResult),
}
struct Background {
    sequence: u32,
    map: Mapping,
    width: f64,
    height: f64,
    hidden: Option<String>,
    canvas: &'static HTMLCanvasElement,
}
pub struct Surface {
    pub canvas: &'static HTMLCanvasElement,
    renderer: Renderer,
    background: RefCell<Option<Background>>,
    live: RefCell<Snapshot>,
    view: Cell<Viewport>,
    brush: RefCell<BrushSettings>,
    pointer: RefCell<Option<Pointer>>,
    draft: RefCell<Option<Element>>,
    selected: RefCell<Option<String>>,
    replay: RefCell<Option<Replay>>,
    // Cached presentation frame; never an authoritative document.
    shown: RefCell<Option<Snapshot>>,
    frame: Cell<Option<f64>>,
    set_view: &'static Dispatch<SetStateAction<Viewport>>,
    set_playback: &'static Dispatch<SetStateAction<Option<f64>>>,
    set_problem: &'static Dispatch<SetStateAction<Option<String>>>,
}
impl Surface {
    pub fn new(
        canvas: &'static HTMLCanvasElement,
        view: Viewport,
        set_view: &'static Dispatch<SetStateAction<Viewport>>,
        set_playback: &'static Dispatch<SetStateAction<Option<f64>>>,
        set_problem: &'static Dispatch<SetStateAction<Option<String>>>,
    ) -> Result<Rc<Self>, String> {
        render::context(canvas)?;
        let surface = Rc::new(Self {
            canvas,
            renderer: Renderer::default(),
            background: RefCell::new(None),
            live: RefCell::new(Journal::default().snapshot("initial")),
            view: Cell::new(view),
            brush: RefCell::new(BrushSettings {
                tool: Tool::Pen,
                color: "#243b35".into(),
                width: 12.0,
                filled: false,
                secondary: "#ffffff".into(),
                opacity: 1.0,
                ink: InkBrush::Marker,
                paint: PaintMode::Solid,
                clip: None,
            }),
            pointer: RefCell::new(None),
            draft: RefCell::new(None),
            selected: RefCell::new(None),
            replay: RefCell::new(None),
            shown: RefCell::new(None),
            frame: Cell::new(None),
            set_view,
            set_playback,
            set_problem,
        });
        surface.resize();
        Ok(surface)
    }
    pub fn configure(&self, settings: BrushSettings) {
        *self.brush.borrow_mut() = settings;
    }
    pub fn selected_clip(&self) -> Result<Clip, String> {
        let element = self
            .selected()
            .ok_or("Select a path shape before clipping the brush.")?;
        match element.drawing.geometry {
            Geometry::Path { segments } => Ok(Clip {
                segments,
                transform: element.drawing.transform,
            }),
            _ => Err(
                "Clipping needs a path shape, such as a rectangle, ellipse or Bezier outline."
                    .into(),
            ),
        }
    }
    pub fn text_drawing(&self, point: Point, text: String, size: f64) -> Drawing {
        self.element(
            &self.brush.borrow(),
            Geometry::Text {
                x: point.x,
                y: point.y,
                text,
                size,
            },
        )
        .drawing
    }
    pub fn update(&self, snapshot: Snapshot) {
        let changed = snapshot.sequence != self.live.borrow().sequence;
        if changed {
            self.stop_replay();
            if self
                .selected
                .borrow()
                .as_ref()
                .is_some_and(|id| !snapshot.elements.iter().any(|element| &element.id == id))
            {
                self.selected.borrow_mut().take();
            }
        }
        *self.live.borrow_mut() = snapshot;
        self.paint();
    }
    pub fn set_view(&self, view: Viewport) -> Result<(), String> {
        view.validate()?;
        self.view.set(view);
        self.set_view.set(view);
        self.paint();
        Ok(())
    }
    pub fn viewport(&self) -> Viewport {
        self.view.get()
    }
    fn map(&self) -> Mapping {
        render::mapping(
            self.canvas.client_width().max(1.0),
            self.canvas.client_height().max(1.0),
            self.view.get(),
        )
    }
    pub fn world(&self, x: f64, y: f64) -> Point {
        let bounds = self.canvas.get_bounding_client_rect();
        let map = self.map();
        Point {
            x: (x - bounds.left() - map.x) / map.scale,
            y: (y - bounds.top() - map.y) / map.scale,
            pressure: 1.0,
        }
    }
    pub fn resize(&self) {
        let ratio = window.device_pixel_ratio().min(2.0);
        self.canvas
            .set_width((self.canvas.client_width().max(1.0) * ratio).round());
        self.canvas
            .set_height((self.canvas.client_height().max(1.0) * ratio).round());
        self.paint();
    }
    pub fn fonts_changed(&self) {
        self.background.borrow_mut().take();
        self.paint();
    }
    pub fn paint(&self) {
        let shown = self.shown.borrow().clone();
        let replaying = shown.is_some();
        let result = if let Some(snapshot) = shown {
            self.paint_snapshot(&snapshot, replaying)
        } else {
            self.paint_snapshot(&self.live.borrow(), false)
        };
        if let Err(error) = result {
            self.set_problem.set(Some(error));
        }
    }
    fn paint_snapshot(&self, snapshot: &Snapshot, replaying: bool) -> Result<(), String> {
        if replaying {
            *self.shown.borrow_mut() = Some(snapshot.clone());
        }
        let map = self.map();
        let ratio = window.device_pixel_ratio().min(2.0);
        let physical = Mapping {
            scale: map.scale * ratio,
            x: map.x * ratio,
            y: map.y * ratio,
        };
        let draft = self.draft.borrow();
        let draft_index = draft.as_ref().and_then(|draft| {
            snapshot
                .elements
                .iter()
                .position(|element| element.id == draft.id)
        });
        if replaying {
            self.renderer
                .paint(self.canvas, &snapshot.elements, physical, true)?;
        } else {
            let hidden = draft_index.map(|index| snapshot.elements[index].id.clone());
            let mut cached = self.background.borrow_mut();
            let valid = cached.as_ref().is_some_and(|background| {
                background.sequence == snapshot.sequence
                    && background.map == physical
                    && background.width == self.canvas.width()
                    && background.height == self.canvas.height()
                    && background.hidden == hidden
            });
            if !valid {
                let canvas = cached
                    .as_ref()
                    .map(|background| background.canvas)
                    .unwrap_or_else(|| {
                        render::new_canvas(self.canvas.width(), self.canvas.height())
                    });
                if canvas.width() != self.canvas.width() {
                    canvas.set_width(self.canvas.width());
                }
                if canvas.height() != self.canvas.height() {
                    canvas.set_height(self.canvas.height());
                }
                self.renderer.paint(
                    canvas,
                    draft_index
                        .map(|index| &snapshot.elements[..index])
                        .unwrap_or(&snapshot.elements),
                    physical,
                    true,
                )?;
                *cached = Some(Background {
                    sequence: snapshot.sequence,
                    map: physical,
                    width: self.canvas.width(),
                    height: self.canvas.height(),
                    hidden,
                    canvas,
                });
            }
            let ctx = render::context(self.canvas)?;
            ctx.reset_transform();
            ctx.set_global_alpha(1.0);
            ctx.set_global_composite_operation("source-over");
            ctx.draw_image(
                cached.as_ref().expect("painted background").canvas,
                0.0,
                0.0,
                None,
                None,
            );
            if let Some(draft) = draft.as_ref() {
                self.renderer.paint_element(self.canvas, draft, physical)?;
            }
            // An existing mark keeps its layer order during a drag. Cache only
            // the finished prefix, then draw the moving mark and its foreground.
            if let Some(index) = draft_index {
                for element in &snapshot.elements[index + 1..] {
                    self.renderer
                        .paint_element(self.canvas, element, physical)?;
                }
            }
        }
        if !replaying
            && let Some(selected) = self.selected.borrow().as_ref()
            && let Some(element) = draft
                .as_ref()
                .filter(|element| &element.id == selected)
                .or_else(|| {
                    snapshot
                        .elements
                        .iter()
                        .find(|element| &element.id == selected)
                })
        {
            let bounds = render::bounds(element);
            let ctx = render::context(self.canvas)?;
            ctx.save();
            ctx.reset_transform();
            ctx.transform(
                physical.scale,
                0.0,
                0.0,
                physical.scale,
                physical.x,
                physical.y,
            );
            ctx.set_stroke_style("#3d8065");
            ctx.set_line_width(1.3 / map.scale);
            ctx.set_line_dash(&[5.0 / map.scale, 4.0 / map.scale]);
            ctx.stroke_rect(bounds.x, bounds.y, bounds.width, bounds.height);
            ctx.restore();
        }
        self.canvas.set_attribute(
            "data-drafting",
            (!replaying && self.draft.borrow().is_some()).to_string(),
        );
        self.canvas
            .set_attribute("data-sequence", snapshot.sequence.to_string());
        self.canvas.set_attribute(
            "data-live-sequence",
            self.live.borrow().sequence.to_string(),
        );
        self.canvas.set_attribute(
            "data-element-count",
            (snapshot.elements.len()
                + usize::from(
                    !replaying
                        && draft.as_ref().is_some_and(|draft| {
                            !snapshot
                                .elements
                                .iter()
                                .any(|element| element.id == draft.id)
                        }),
                ))
            .to_string(),
        );
        self.canvas
            .set_attribute("data-playback", if replaying { "replay" } else { "live" });
        Ok(())
    }
    pub fn clear_draft(&self) {
        self.draft.borrow_mut().take();
        self.paint();
    }
    pub fn cancel_pointer(&self) {
        if self.pointer.borrow_mut().take().is_some() {
            self.clear_draft();
        }
    }
    pub fn selected(&self) -> Option<Element> {
        let id = self.selected.borrow().clone();
        self.live
            .borrow()
            .elements
            .iter()
            .find(|e| Some(&e.id) == id.as_ref())
            .cloned()
    }
    pub fn deselect(&self) {
        self.selected.borrow_mut().take();
        self.paint();
    }
    fn element(&self, brush: &BrushSettings, geometry: Geometry) -> Element {
        let ink = if matches!(geometry, Geometry::Stroke { .. }) {
            brush.ink.clone()
        } else {
            InkBrush::Marker
        };
        let mut element = Element {
            id: "draft".into(),
            version: 0,
            order: u32::MAX,
            author: "draft".into(),
            drawing: Drawing {
                clip: brush.clip.clone(),
                geometry,
                style: Style {
                    stroke: brush.color.clone().into(),
                    fill: brush.filled.then(|| brush.color.clone().into()),
                    width: brush.width,
                    opacity: brush.opacity,
                    brush: ink,
                },
                transform: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            },
            masks: Vec::new(),
        };
        if brush.paint != PaintMode::Solid {
            let bounds = render::bounds(&element);
            let stops = vec![
                ColorStop {
                    offset: 0.0,
                    color: brush.color.clone(),
                    opacity: 1.0,
                },
                ColorStop {
                    offset: 1.0,
                    color: brush.secondary.clone(),
                    opacity: 1.0,
                },
            ];
            let gradient = match brush.paint {
                PaintMode::Linear => Gradient::Linear {
                    x0: bounds.x,
                    y0: bounds.y,
                    x1: bounds.x + bounds.width,
                    y1: bounds.y + bounds.height,
                    stops,
                },
                _ => Gradient::Radial {
                    cx: bounds.x + bounds.width * 0.35,
                    cy: bounds.y + bounds.height * 0.3,
                    radius: bounds.width.max(bounds.height) * 0.7,
                    stops,
                },
            };
            let paint = Paint::Gradient(gradient);
            element.drawing.style.stroke = paint.clone();
            element.drawing.style.fill = brush.filled.then_some(paint);
        }
        element
    }
    pub fn down(&self, id: i32, x: f64, y: f64, pressure: f64) -> Option<Point> {
        if self.replay.borrow().is_some() {
            return None;
        }
        let point = self.world(x, y);
        let brush = self.brush.borrow().clone();
        if brush.tool == Tool::Text {
            return Some(point);
        }
        let element = if brush.tool == Tool::Select {
            render::hit(&self.live.borrow(), &point, 8.0 / self.map().scale)
        } else {
            None
        };
        *self.selected.borrow_mut() = element.as_ref().map(|element| element.id.clone());
        let points = vec![Point {
            pressure,
            ..point.clone()
        }];
        if brush.tool == Tool::Pen {
            *self.draft.borrow_mut() = Some(self.element(
                &brush,
                Geometry::Stroke {
                    points: points.clone(),
                },
            ));
        }
        *self.pointer.borrow_mut() = Some(Pointer {
            id,
            brush,
            start: point,
            screen: (x, y),
            view: self.view.get(),
            points,
            element,
        });
        self.paint();
        None
    }
    pub fn moved(&self, id: i32, x: f64, y: f64, native: Option<&PointerEvent>) {
        let current = self.pointer.borrow_mut().take();
        let Some(mut pointer) = current else {
            return;
        };
        if pointer.id != id {
            *self.pointer.borrow_mut() = Some(pointer);
            return;
        }
        let point = self.world(x, y);
        match pointer.brush.tool {
            Tool::Hand => {
                let scale = render::mapping(
                    self.canvas.client_width(),
                    self.canvas.client_height(),
                    pointer.view,
                )
                .scale;
                let _ = self.set_view(Viewport {
                    cx: pointer.view.cx - (x - pointer.screen.0) / scale,
                    cy: pointer.view.cy - (y - pointer.screen.1) / scale,
                    ..pointer.view
                });
            }
            Tool::Select => {
                if let Some(element) = &pointer.element {
                    let mut draft = element.clone();
                    draft.drawing.transform[4] += point.x - pointer.start.x;
                    draft.drawing.transform[5] += point.y - pointer.start.y;
                    *self.draft.borrow_mut() = Some(draft);
                }
            }
            Tool::Pen | Tool::Eraser => {
                let samples = native
                    .map(|event| catch_unwind(|| event.get_coalesced_events()).unwrap_or_default())
                    .unwrap_or_default();
                if samples.is_empty() {
                    push_point(&mut pointer.points, point);
                } else {
                    for sample in samples {
                        let mut point = self.world(sample.client_x(), sample.client_y());
                        point.pressure = if sample.pointer_type() == "pen" {
                            sample.pressure()
                        } else {
                            1.0
                        };
                        push_point(&mut pointer.points, point);
                    }
                }
                if pointer.brush.tool == Tool::Pen {
                    *self.draft.borrow_mut() = Some(self.element(
                        &pointer.brush,
                        Geometry::Stroke {
                            points: pointer.points.clone(),
                        },
                    ));
                }
            }
            Tool::Line | Tool::Rectangle | Tool::Ellipse => {
                *self.draft.borrow_mut() = Some(self.element(
                    &pointer.brush,
                    Geometry::Path {
                        segments: shape(pointer.brush.tool, &pointer.start, &point),
                    },
                ));
            }
            Tool::Text => {}
        }
        *self.pointer.borrow_mut() = Some(pointer);
        self.paint();
    }
    pub fn up(&self, id: i32, x: f64, y: f64, native: &PointerEvent) -> Option<Vec<Operation>> {
        self.moved(id, x, y, Some(native));
        let pointer = self.pointer.borrow_mut().take()?;
        if pointer.id != id {
            *self.pointer.borrow_mut() = Some(pointer);
            return None;
        }
        let point = self.world(x, y);
        match pointer.brush.tool {
            Tool::Hand | Tool::Text => None,
            Tool::Select => pointer.element.and_then(|element| {
                if point.x == pointer.start.x && point.y == pointer.start.y {
                    self.clear_draft();
                    return None;
                }
                let mut drawing = element.drawing;
                drawing.transform[4] += point.x - pointer.start.x;
                drawing.transform[5] += point.y - pointer.start.y;
                Some(vec![Operation::Replace {
                    id: element.id,
                    expected_version: element.version,
                    drawing,
                }])
            }),
            Tool::Eraser => {
                let targets = self
                    .live
                    .borrow()
                    .elements
                    .iter()
                    .map(|element| Target {
                        id: element.id.clone(),
                        expected_version: element.version,
                    })
                    .collect::<Vec<_>>();
                if targets.is_empty() {
                    None
                } else {
                    Some(vec![Operation::Erase {
                        targets,
                        points: pointer.points,
                        radius: pointer.brush.width * 2.0,
                    }])
                }
            }
            _ => {
                let geometry = if pointer.brush.tool == Tool::Pen {
                    Geometry::Stroke {
                        points: pointer.points,
                    }
                } else {
                    Geometry::Path {
                        segments: shape(pointer.brush.tool, &pointer.start, &point),
                    }
                };
                Some(vec![Operation::Create {
                    id: window.crypto().random_uuid(),
                    drawing: self.element(&pointer.brush, geometry).drawing,
                }])
            }
        }
    }
    pub fn zoom_at(&self, x: f64, y: f64, delta: f64) {
        let bounds = self.canvas.get_bounding_client_rect();
        let map = self.map();
        let world = self.world(x, y);
        let previous = self.view.get();
        let zoom = (previous.zoom * (-delta * 0.0015).exp()).clamp(0.2, 8.0);
        let scale = map.scale / previous.zoom * zoom;
        let _ = self.set_view(Viewport {
            zoom,
            cx: world.x - (x - bounds.left() - self.canvas.client_width() / 2.0) / scale,
            cy: world.y - (y - bounds.top() - self.canvas.client_height() / 2.0) / scale,
        });
    }
    pub fn stop_replay(&self) {
        if let Some(frame) = self.frame.take() {
            window.cancel_animation_frame(frame);
        }
        let running = self.replay.borrow_mut().take();
        self.shown.borrow_mut().take();
        if let Some(running) = running {
            (running.resolve)(PlaybackResult { cancelled: true });
            self.shown.borrow_mut().take();
            self.set_playback.set(None);
            self.paint();
        }
    }
    pub async fn start_replay(
        self: &Rc<Self>,
        journal: Journal,
        duration: Option<u32>,
    ) -> Result<PlaybackResult, String> {
        self.stop_replay();
        if journal.sequence == 0 {
            return Err("Make a mark before replaying.".into());
        }
        let duration = duration.unwrap_or((journal.sequence * 650).clamp(2400, 18000));
        if !(500..=30000).contains(&duration) {
            return Err("Replay duration must be from 500 to 30000 ms.".into());
        }
        if window
            .match_media("(prefers-reduced-motion: reduce)")
            .matches()
        {
            return Ok(PlaybackResult { cancelled: false });
        }
        self.draft.borrow_mut().take();
        self.selected.borrow_mut().take();
        let snapshot = replay_snapshot(&journal, 0)?;
        let waiting = js::promise::with_resolvers();
        *self.replay.borrow_mut() = Some(Replay {
            journal,
            started: None,
            duration: f64::from(duration),
            last_step: None,
            snapshot: snapshot.clone(),
            next: snapshot.clone(),
            resolve: waiting.resolve,
        });
        self.set_playback.set(Some(0.0));
        self.paint_snapshot(&snapshot, true)?;
        self.schedule();
        Ok(waiting.promise.await)
    }
    fn schedule(self: &Rc<Self>) {
        let surface = Rc::clone(self);
        self.frame.set(Some(
            window.request_animation_frame(Box::new(move |time| surface.tick(time))),
        ));
    }
    fn tick(self: &Rc<Self>, time: f64) {
        self.frame.set(None);
        let active = self.replay.borrow_mut().take();
        let Some(mut replay) = active else {
            return;
        };
        if document.hidden() {
            (replay.resolve)(PlaybackResult { cancelled: true });
            self.shown.borrow_mut().take();
            self.set_playback.set(None);
            self.paint();
            return;
        }
        let start = *replay.started.get_or_insert(time);
        let progress = ((time - start) / replay.duration).clamp(0.0, 1.0);
        let position = progress * f64::from(replay.journal.sequence);
        let step = (position.floor() as u32).min(replay.journal.sequence);
        if replay.last_step != Some(step) {
            replay.snapshot =
                replay_snapshot(&replay.journal, step).expect("validated replay history");
            replay.next = if step < replay.journal.sequence {
                replay_snapshot(&replay.journal, step + 1).expect("validated replay history")
            } else {
                replay.snapshot.clone()
            };
            replay.last_step = Some(step);
        }
        let mut visible = replay.snapshot.clone();
        if step < replay.journal.sequence {
            let fraction = position - f64::from(step);
            let created = replay.journal.events[step as usize]
                .changes
                .iter()
                .filter(|change| change.before.is_none() && change.after.is_some())
                .map(|change| change.id.clone())
                .collect::<HashSet<_>>();
            for element in replay
                .next
                .elements
                .iter()
                .filter(|element| created.contains(&element.id))
            {
                let mut element = element.clone();
                match &mut element.drawing.geometry {
                    Geometry::Stroke { points } => {
                        let len = (points.len() as f64 * fraction).ceil() as usize;
                        points.truncate(len.max(1));
                    }
                    Geometry::Path { segments } => {
                        let len = (segments.len() as f64 * fraction).ceil() as usize;
                        segments.truncate(len.max(1));
                    }
                    Geometry::Text { .. } => element.drawing.style.opacity *= fraction,
                };
                visible.elements.push(element);
            }
            visible.elements.sort_by_key(|element| element.order);
        }
        if let Err(error) = self.paint_snapshot(&visible, true) {
            self.set_problem.set(Some(error));
            (replay.resolve)(PlaybackResult { cancelled: true });
            self.shown.borrow_mut().take();
            self.set_playback.set(None);
            self.paint();
            return;
        }
        if progress >= 1.0 {
            self.shown.borrow_mut().take();
            self.set_playback.set(None);
            self.paint();
            (replay.resolve)(PlaybackResult { cancelled: false });
        } else {
            self.set_playback.set(Some(progress));
            *self.replay.borrow_mut() = Some(replay);
            self.schedule();
        }
    }
    pub async fn wait_painted(self: &Rc<Self>, sequence: u32) -> bool {
        if document.hidden() {
            return false;
        }
        let waiting = js::promise::with_resolvers();
        let surface = Rc::clone(self);
        window.request_animation_frame(Box::new(move |_| {
            (waiting.resolve)(
                !document.hidden()
                    && surface.live.borrow().sequence >= sequence
                    && surface.replay.borrow().is_none(),
            );
        }));
        waiting.promise.await
    }
}
fn push_point(points: &mut Vec<Point>, point: Point) {
    if points
        .last()
        .is_none_or(|last| (point.x - last.x).hypot(point.y - last.y) > 0.3)
    {
        points.push(point);
    }
}
fn shape(tool: Tool, start: &Point, end: &Point) -> Vec<Segment> {
    if tool == Tool::Line {
        return vec![
            Segment::M {
                x: start.x,
                y: start.y,
            },
            Segment::L { x: end.x, y: end.y },
        ];
    }
    let x = start.x.min(end.x);
    let y = start.y.min(end.y);
    let w = (end.x - start.x).abs();
    let h = (end.y - start.y).abs();
    if tool == Tool::Rectangle {
        return vec![
            Segment::M { x, y },
            Segment::L { x: x + w, y },
            Segment::L { x: x + w, y: y + h },
            Segment::L { x, y: y + h },
            Segment::Z,
        ];
    }
    let k = 0.5522847498;
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;
    let rx = w / 2.0;
    let ry = h / 2.0;
    vec![
        Segment::M { x: cx + rx, y: cy },
        Segment::C {
            cx1: cx + rx,
            cy1: cy + k * ry,
            cx2: cx + k * rx,
            cy2: cy + ry,
            x: cx,
            y: cy + ry,
        },
        Segment::C {
            cx1: cx - k * rx,
            cy1: cy + ry,
            cx2: cx - rx,
            cy2: cy + k * ry,
            x: cx - rx,
            y: cy,
        },
        Segment::C {
            cx1: cx - rx,
            cy1: cy - k * ry,
            cx2: cx - k * rx,
            cy2: cy - ry,
            x: cx,
            y: cy - ry,
        },
        Segment::C {
            cx1: cx + k * rx,
            cy1: cy - ry,
            cx2: cx + rx,
            cy2: cy - k * ry,
            x: cx + rx,
            y: cy,
        },
        Segment::Z,
    ]
}
