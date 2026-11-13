//! Browser drawing uses the canonical paths produced by board-core.
use std::cell::RefCell;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use webapi::{
    CanvasRenderingContext2D, HTMLCanvasElement, canvas_contexts, document, path2_d, tags,
};
use whiteboard_core::{
    Brush, Drawing, Element, Geometry, Gradient, Paint, Point, Segment, Snapshot, ink_layers,
    inverse, mask_data, path_data, segment_data,
};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Viewport {
    pub cx: f64,
    pub cy: f64,
    pub zoom: f64,
}
impl Default for Viewport {
    fn default() -> Self {
        Self {
            cx: 2048.0,
            cy: 1152.0,
            zoom: 1.0,
        }
    }
}
impl Viewport {
    pub fn validate(&self) -> Result<(), String> {
        if !self.cx.is_finite()
            || !self.cy.is_finite()
            || self.cx.abs() > 1_000_000.0
            || self.cy.abs() > 1_000_000.0
            || !self.zoom.is_finite()
            || !(0.2..=8.0).contains(&self.zoom)
        {
            Err("Invalid viewport. Zoom is 0.2–8; coordinates must be finite.".into())
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Region {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
#[derive(Clone, Copy)]
pub struct Mapping {
    pub scale: f64,
    pub x: f64,
    pub y: f64,
}
pub fn mapping(width: f64, height: f64, view: Viewport) -> Mapping {
    let scale = (width / 4096.0).min(height / 2304.0) * view.zoom;
    Mapping {
        scale,
        x: width / 2.0 - view.cx * scale,
        y: height / 2.0 - view.cy * scale,
    }
}
pub fn context(
    canvas: &'static HTMLCanvasElement,
) -> Result<&'static CanvasRenderingContext2D, String> {
    canvas
        .get_context(canvas_contexts::TwoD, None)
        .ok_or_else(|| "Canvas rendering is unavailable in this browser.".to_string())
}
pub fn new_canvas(width: f64, height: f64) -> &'static HTMLCanvasElement {
    let canvas = document.create_element(tags::Canvas);
    canvas.set_width(width.round().max(1.0));
    canvas.set_height(height.round().max(1.0));
    canvas
}
fn apply_mapping(ctx: &CanvasRenderingContext2D, map: Mapping) {
    ctx.reset_transform();
    ctx.transform(map.scale, 0.0, 0.0, map.scale, map.x, map.y);
}
fn transform(ctx: &CanvasRenderingContext2D, matrix: [f64; 6]) {
    let [a, b, c, d, e, f] = matrix;
    ctx.transform(a, b, c, d, e, f);
}

#[derive(Default)]
pub struct Renderer {
    paths: RefCell<HashMap<String, &'static webapi::Path2D>>,
    ink: RefCell<HashMap<String, Vec<(&'static webapi::Path2D, f64)>>>,
}
impl Renderer {
    fn path(&self, drawing: &Drawing) -> Result<&'static webapi::Path2D, String> {
        let key = serde_json::to_string(drawing).map_err(|e| e.to_string())?;
        let found = self.paths.borrow().get(&key).copied();
        if let Some(found) = found {
            return Ok(found);
        }
        let data = path_data(drawing)?;
        let path = path2_d::new(Some(&data));
        let mut cache = self.paths.borrow_mut();
        if cache.len() >= 500 {
            cache.clear();
        }
        cache.insert(key, path);
        Ok(path)
    }
    fn ink(&self, drawing: &Drawing) -> Result<Vec<(&'static webapi::Path2D, f64)>, String> {
        let key = serde_json::to_string(drawing).map_err(|e| e.to_string())?;
        if let Some(paths) = self.ink.borrow().get(&key) {
            return Ok(paths.clone());
        }
        let paths = ink_layers(drawing)?
            .into_iter()
            .map(|layer| (path2_d::new(Some(&layer.path)), layer.opacity))
            .collect::<Vec<_>>();
        let mut cache = self.ink.borrow_mut();
        if cache.len() >= 500 {
            cache.clear();
        }
        cache.insert(key, paths.clone());
        Ok(paths)
    }
    fn geometry(&self, ctx: &CanvasRenderingContext2D, element: &Element) -> Result<(), String> {
        let drawing = &element.drawing;
        let style = &drawing.style;
        transform(ctx, drawing.transform);
        ctx.set_line_cap("round");
        ctx.set_line_join("round");
        ctx.set_line_width(style.width);
        if let Some(clip) = &drawing.clip {
            transform(ctx, clip.transform);
            ctx.clip_with_path(
                path2_d::new(Some(&segment_data(&clip.segments))),
                Some("nonzero"),
            );
            transform(ctx, inverse(clip.transform));
        }
        set_paint(ctx, &style.stroke, true);
        set_paint(ctx, style.fill.as_ref().unwrap_or(&style.stroke), false);
        match &drawing.geometry {
            Geometry::Stroke { .. } => {
                set_paint(ctx, &style.stroke, false);
                for (path, opacity) in self.ink(drawing)? {
                    ctx.set_global_alpha(opacity);
                    ctx.fill_with_path(path, Some("nonzero"));
                }
                ctx.set_global_alpha(1.0);
            }
            Geometry::Path { .. } => {
                let path = self.path(drawing)?;
                if style.fill.is_some() {
                    ctx.fill_with_path(path, Some("nonzero"));
                }
                ctx.stroke(Some(path));
            }
            Geometry::Text { x, y, text, size } => {
                ctx.set_font(&format!("{size}px 'DM Sans', sans-serif"));
                ctx.set_text_baseline("alphabetic");
                set_paint(ctx, &style.stroke, false);
                for (index, line) in text.split('\n').enumerate() {
                    ctx.fill_text(line, *x, *y + index as f64 * size * 1.3, None);
                }
            }
        }
        for mask in &element.masks {
            let data = mask_data(mask)?;
            ctx.save();
            ctx.set_global_composite_operation("destination-out");
            ctx.set_global_alpha(1.0);
            transform(ctx, mask.transform);
            ctx.set_fill_style("#000000");
            ctx.fill_with_path(path2_d::new(Some(&data)), Some("nonzero"));
            ctx.restore();
        }
        Ok(())
    }
    pub fn paint(
        &self,
        canvas: &'static HTMLCanvasElement,
        snapshot: &Snapshot,
        map: Mapping,
        grid: bool,
    ) -> Result<(), String> {
        let ctx = context(canvas)?;
        let width = canvas.width();
        let height = canvas.height();
        ctx.reset_transform();
        ctx.set_global_alpha(1.0);
        ctx.set_global_composite_operation("source-over");
        ctx.clear_rect(0.0, 0.0, width, height);
        ctx.set_fill_style("#ffffff");
        ctx.fill_rect(0.0, 0.0, width, height);
        if grid {
            let spacing = 96.0 * map.scale;
            if spacing >= 12.0 {
                ctx.set_fill_style("#dfe5df");
                let mut x = map.x.rem_euclid(spacing);
                while x < width {
                    let mut y = map.y.rem_euclid(spacing);
                    while y < height {
                        ctx.fill_rect(x, y, 1.0, 1.0);
                        y += spacing;
                    }
                    x += spacing;
                }
            }
        }
        let mut layer = None;
        for element in &snapshot.elements {
            let isolated = !element.masks.is_empty()
                || element.drawing.style.opacity < 1.0
                || !matches!(element.drawing.style.brush, Brush::Marker);
            let destination = if isolated {
                let surface = *layer.get_or_insert_with(|| new_canvas(width, height));
                surface.set_width(width);
                surface.set_height(height);
                context(surface)?
            } else {
                ctx
            };
            destination.save();
            apply_mapping(destination, map);
            self.geometry(destination, element)?;
            destination.restore();
            if isolated && let Some(surface) = layer {
                ctx.save();
                ctx.reset_transform();
                ctx.set_global_alpha(element.drawing.style.opacity);
                ctx.draw_image(surface, 0.0, 0.0, None, None);
                ctx.restore();
            }
        }
        Ok(())
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureInfo {
    pub sequence: u32,
    pub bounds: Region,
    pub pixel_width: f64,
    pub pixel_height: f64,
    pub world_units_per_pixel: f64,
}
pub struct Capture {
    pub info: CaptureInfo,
    pub data_url: String,
}
pub async fn capture(
    snapshot: &Snapshot,
    bounds: Option<Region>,
    max_size: Option<u32>,
) -> Result<Capture, String> {
    let _ = js::settle(document.fonts().ready()).await;
    let region = bounds.unwrap_or(Region {
        x: 0.0,
        y: 0.0,
        width: snapshot.width,
        height: snapshot.height,
    });
    let max = max_size.unwrap_or(1536);
    if ![region.x, region.y, region.width, region.height]
        .iter()
        .all(|v| v.is_finite())
        || region.x.abs() > 1_000_000.0
        || region.y.abs() > 1_000_000.0
        || region.width <= 0.0
        || region.height <= 0.0
        || region.width > 2_000_000.0
        || region.height > 2_000_000.0
        || !(64..=2048).contains(&max)
    {
        return Err("Capture requires finite bounds and a size from 64 to 2048 pixels.".into());
    }
    let scale = (f64::from(max) / region.width).min(f64::from(max) / region.height);
    let canvas = new_canvas(region.width * scale, region.height * scale);
    Renderer::default().paint(
        canvas,
        snapshot,
        Mapping {
            scale,
            x: -region.x * scale,
            y: -region.y * scale,
        },
        false,
    )?;
    let data_url = canvas
        .to_data_url(Some("image/png"), None)
        .map_err(|_| "Could not capture the canvas.".to_string())?;
    if data_url.len() > 6 * 1024 * 1024 {
        return Err("Capture exceeds supported size. Request a smaller image.".into());
    }
    Ok(Capture {
        info: CaptureInfo {
            sequence: snapshot.sequence,
            bounds: region,
            pixel_width: canvas.width(),
            pixel_height: canvas.height(),
            world_units_per_pixel: 1.0 / scale,
        },
        data_url,
    })
}
pub fn bounds(element: &Element) -> Region {
    let points = match &element.drawing.geometry {
        Geometry::Stroke { points } => points.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>(),
        Geometry::Path { segments } => segments
            .iter()
            .flat_map(|segment| match segment {
                Segment::M { x, y } | Segment::L { x, y } => vec![(*x, *y)],
                Segment::Q { cx, cy, x, y } => vec![(*cx, *cy), (*x, *y)],
                Segment::C {
                    cx1,
                    cy1,
                    cx2,
                    cy2,
                    x,
                    y,
                } => vec![(*cx1, *cy1), (*cx2, *cy2), (*x, *y)],
                Segment::Z => Vec::new(),
            })
            .collect(),
        Geometry::Text { x, y, text, size } => vec![
            (*x, *y - size),
            (
                *x + text
                    .split('\n')
                    .map(|line| line.chars().count())
                    .max()
                    .unwrap_or(0) as f64
                    * size
                    * 0.7,
                *y + text.lines().count().saturating_sub(1) as f64 * size * 1.3,
            ),
        ],
    };
    let [a, b, c, d, e, f] = element.drawing.transform;
    let transformed = points
        .iter()
        .map(|(x, y)| (a * x + c * y + e, b * x + d * y + f))
        .collect::<Vec<_>>();
    let min_x = transformed
        .iter()
        .map(|p| p.0)
        .fold(f64::INFINITY, f64::min);
    let max_x = transformed
        .iter()
        .map(|p| p.0)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = transformed
        .iter()
        .map(|p| p.1)
        .fold(f64::INFINITY, f64::min);
    let max_y = transformed
        .iter()
        .map(|p| p.1)
        .fold(f64::NEG_INFINITY, f64::max);
    let pad = element.drawing.style.width;
    Region {
        x: min_x - pad,
        y: min_y - pad,
        width: max_x - min_x + pad * 2.0,
        height: max_y - min_y + pad * 2.0,
    }
}
pub fn hit(snapshot: &Snapshot, point: &Point, tolerance: f64) -> Option<Element> {
    snapshot
        .elements
        .iter()
        .rev()
        .find(|element| {
            let b = bounds(element);
            point.x >= b.x - tolerance
                && point.x <= b.x + b.width + tolerance
                && point.y >= b.y - tolerance
                && point.y <= b.y + b.height + tolerance
        })
        .cloned()
}
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn matrix_text(matrix: [f64; 6]) -> String {
    matrix
        .iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}
fn stop_color(stop: &whiteboard_core::ColorStop) -> String {
    let r = u8::from_str_radix(&stop.color[1..3], 16).expect("validated red");
    let g = u8::from_str_radix(&stop.color[3..5], 16).expect("validated green");
    let b = u8::from_str_radix(&stop.color[5..7], 16).expect("validated blue");
    format!("rgba({r},{g},{b},{})", stop.opacity)
}
fn set_paint(ctx: &CanvasRenderingContext2D, paint: &Paint, stroke: bool) {
    match paint {
        Paint::Color(color) => {
            if stroke {
                ctx.set_stroke_style(color);
            } else {
                ctx.set_fill_style(color);
            }
        }
        Paint::Gradient(paint) => {
            let gradient = match paint {
                Gradient::Linear { x0, y0, x1, y1, .. } => {
                    ctx.create_linear_gradient(*x0, *y0, *x1, *y1)
                }
                Gradient::Radial { cx, cy, radius, .. } => {
                    ctx.create_radial_gradient(*cx, *cy, 0.0, *cx, *cy, *radius)
                }
            };
            for stop in paint.stops() {
                gradient.add_color_stop(stop.offset, &stop_color(stop));
            }
            if stroke {
                ctx.set_stroke_style(gradient);
            } else {
                ctx.set_fill_style(gradient);
            }
        }
    }
}
fn svg_paint(paint: &Paint, id: &str, definitions: &mut String) -> String {
    match paint {
        Paint::Color(color) => color.clone(),
        Paint::Gradient(gradient) => {
            let (tag, attributes) = match gradient {
                Gradient::Linear { x0, y0, x1, y1, .. } => (
                    "linearGradient",
                    format!("x1=\"{x0}\" y1=\"{y0}\" x2=\"{x1}\" y2=\"{y1}\""),
                ),
                Gradient::Radial { cx, cy, radius, .. } => (
                    "radialGradient",
                    format!("cx=\"{cx}\" cy=\"{cy}\" r=\"{radius}\""),
                ),
            };
            definitions.push_str(&format!(
                "<defs><{tag} id=\"{id}\" gradientUnits=\"userSpaceOnUse\" {attributes}>"
            ));
            for stop in gradient.stops() {
                definitions.push_str(&format!(
                    "<stop offset=\"{}\" stop-color=\"{}\" stop-opacity=\"{}\"/>",
                    stop.offset, stop.color, stop.opacity
                ));
            }
            definitions.push_str(&format!("</{tag}></defs>"));
            format!("url(#{id})")
        }
    }
}
pub fn svg(snapshot: &Snapshot) -> Result<String, String> {
    let mut body = String::new();
    for element in &snapshot.elements {
        let drawing = &element.drawing;
        let style = &drawing.style;
        let stroke_paint = svg_paint(&style.stroke, &format!("stroke-{}", element.id), &mut body);
        let fill_paint = style
            .fill
            .as_ref()
            .map(|paint| svg_paint(paint, &format!("fill-{}", element.id), &mut body))
            .unwrap_or_else(|| "none".into());
        let clip_id = format!("clip-{}", element.id);
        if let Some(clip) = &drawing.clip {
            body.push_str(&format!("<defs><clipPath id=\"{clip_id}\" clipPathUnits=\"userSpaceOnUse\"><path d=\"{}\" transform=\"matrix({})\"/></clipPath></defs>",segment_data(&clip.segments),matrix_text(clip.transform)));
        }
        let mask_id = format!("mask-{}", element.id);
        if !element.masks.is_empty() {
            body.push_str(&format!("<defs><mask id=\"{mask_id}\" maskUnits=\"userSpaceOnUse\" x=\"-1000000\" y=\"-1000000\" width=\"2000000\" height=\"2000000\"><rect x=\"-1000000\" y=\"-1000000\" width=\"2000000\" height=\"2000000\" fill=\"white\"/>"));
            for mask in &element.masks {
                body.push_str(&format!(
                    "<path d=\"{}\" transform=\"matrix({})\" fill=\"black\"/>",
                    mask_data(mask)?,
                    matrix_text(mask.transform)
                ));
            }
            body.push_str("</mask></defs>");
        }
        let mask_attribute = if element.masks.is_empty() {
            String::new()
        } else {
            format!(" mask=\"url(#{mask_id})\"")
        };
        body.push_str(&format!(
            "<g transform=\"matrix({})\" opacity=\"{}\"{mask_attribute}>",
            matrix_text(drawing.transform),
            style.opacity
        ));
        if drawing.clip.is_some() {
            body.push_str(&format!("<g clip-path=\"url(#{clip_id})\">"));
        }
        match &drawing.geometry {
            Geometry::Stroke { .. } => {
                for layer in ink_layers(drawing)? {
                    body.push_str(&format!(
                        "<path d=\"{}\" fill=\"{stroke_paint}\" opacity=\"{}\"/>",
                        layer.path, layer.opacity
                    ));
                }
            }
            Geometry::Text { x, y, text, size } => {
                for (index, line) in text.split('\n').enumerate() {
                    body.push_str(&format!("<text x=\"{x}\" y=\"{}\" font-size=\"{size}\" font-family=\"DM Sans,sans-serif\" fill=\"{}\">{}</text>",y+index as f64*size*1.3,stroke_paint,escape(line)));
                }
            }
            Geometry::Path { .. } => {
                let stroke = &stroke_paint;
                let fill = &fill_paint;
                body.push_str(&format!("<path d=\"{}\" fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\" fill-rule=\"nonzero\"/>",path_data(drawing)?,style.width));
            }
        }
        if drawing.clip.is_some() {
            body.push_str("</g>");
        }
        body.push_str("</g>");
    }
    Ok(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" width=\"{}\" height=\"{}\"><rect width=\"100%\" height=\"100%\" fill=\"white\"/>{body}</svg>",
        snapshot.width, snapshot.height, snapshot.width, snapshot.height
    ))
}
