//! Portable paint definitions and deterministic brush geometry, shared by every renderer.
use crate::{Drawing, Geometry, Point, Segment, color, coordinate, identity, ink_path};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Paint {
    Color(String),
    Gradient(Gradient),
}
impl From<String> for Paint {
    fn from(value: String) -> Self {
        Self::Color(value)
    }
}
impl From<&str> for Paint {
    fn from(value: &str) -> Self {
        Self::Color(value.into())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColorStop {
    pub offset: f64,
    pub color: String,
    #[serde(default = "crate::one")]
    pub opacity: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Gradient {
    Linear {
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        stops: Vec<ColorStop>,
    },
    Radial {
        cx: f64,
        cy: f64,
        radius: f64,
        stops: Vec<ColorStop>,
    },
}
impl Gradient {
    pub fn stops(&self) -> &[ColorStop] {
        match self {
            Self::Linear { stops, .. } | Self::Radial { stops, .. } => stops,
        }
    }
}
impl Paint {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Color(value) if color(value) => Ok(()),
            Self::Gradient(gradient) => {
                let valid = match gradient {
                    Gradient::Linear { x0, y0, x1, y1, .. } => {
                        [*x0, *y0, *x1, *y1].iter().all(|v| coordinate(*v))
                            && (*x1 - *x0).hypot(*y1 - *y0) > 0.000001
                    }
                    Gradient::Radial { cx, cy, radius, .. } => {
                        coordinate(*cx)
                            && coordinate(*cy)
                            && radius.is_finite()
                            && *radius > 0.0
                            && *radius <= 1_000_000.0
                    }
                };
                let stops = gradient.stops();
                if !valid
                    || !(2..=16).contains(&stops.len())
                    || stops.iter().any(|s| {
                        !s.offset.is_finite()
                            || !(0.0..=1.0).contains(&s.offset)
                            || !color(&s.color)
                            || !s.opacity.is_finite()
                            || !(0.0..=1.0).contains(&s.opacity)
                    })
                    || stops.windows(2).any(|s| s[0].offset > s[1].offset)
                {
                    Err("Invalid gradient: use finite coordinates and 2–16 ordered stops, with offsets/opacity from 0 to 1 and #RRGGBB colors.".into())
                } else {
                    Ok(())
                }
            }
            _ => Err("Paint must be #RRGGBB or a linear/radial gradient.".into()),
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Brush {
    #[default]
    Marker,
    Soft {
        softness: f64,
    },
    Textured {
        grain: f64,
        seed: u32,
    },
}
impl Brush {
    pub fn validate(&self, geometry: &Geometry, width: f64) -> Result<(), String> {
        match self {
            Self::Marker => Ok(()),
            Self::Soft { softness } if softness.is_finite() && (0.0..=1.0).contains(softness) && matches!(geometry,Geometry::Stroke{..}) => Ok(()),
            Self::Textured { grain, .. } if grain.is_finite() && (0.05..=1.0).contains(grain) && matches!(geometry,Geometry::Stroke{..}) => {
                if let Geometry::Stroke { points } = geometry {
                    let steps=texture_steps(points,width);
                    if steps>4000.0 { return Err("Textured stroke is too long. Use a wider brush or split it into shorter strokes (maximum 4000 brush samples).".into()); }
                }
                Ok(())
            }
            _ => Err("Soft/textured brushes require stroke geometry; softness is 0–1 and grain is 0.05–1.".into()),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct Style {
    pub stroke: Paint,
    pub fill: Option<Paint>,
    pub width: f64,
    pub opacity: f64,
    pub brush: Brush,
}
impl Default for Style {
    fn default() -> Self {
        Self {
            stroke: "#243b35".into(),
            fill: None,
            width: 8.0,
            opacity: 1.0,
            brush: Brush::Marker,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Clip {
    pub segments: Vec<Segment>,
    #[serde(default = "identity")]
    pub transform: [f64; 6],
}
#[derive(Clone, Debug, PartialEq)]
pub struct InkLayer {
    pub path: String,
    pub opacity: f64,
}
fn texture_steps(points: &[Point], width: f64) -> f64 {
    1.0 + points
        .windows(2)
        .map(|p| ((p[1].x - p[0].x).hypot(p[1].y - p[0].y) / (width * 0.12).max(1.0)).ceil())
        .sum::<f64>()
}
fn random(seed: &mut u32) -> f64 {
    *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
    f64::from(*seed) / 4294967296.0
}
fn stipple(path: &mut String, point: &Point, width: f64, grain: f64, seed: &mut u32) {
    let radius = width * (0.2 + point.pressure * 0.8) / 2.0;
    for _ in 0..8 {
        let angle = random(seed) * std::f64::consts::TAU;
        let distance = random(seed).sqrt() * radius * 0.92;
        let dot = width * (0.012 + grain * 0.045) * (0.5 + random(seed) * 0.5);
        let x = point.x + angle.cos() * distance;
        let y = point.y + angle.sin() * distance;
        path.push_str(&format!(
            "M {} {y} a {dot} {dot} 0 1 0 {} 0 a {dot} {dot} 0 1 0 {} 0 Z ",
            x + dot,
            -dot * 2.0,
            dot * 2.0
        ));
    }
}
pub fn ink_layers(drawing: &Drawing) -> Result<Vec<InkLayer>, String> {
    drawing.validate()?;
    let Geometry::Stroke { points } = &drawing.geometry else {
        return Err("Ink layers require stroke geometry.".into());
    };
    match &drawing.style.brush {
        Brush::Marker => Ok(vec![InkLayer {
            path: ink_path(points, drawing.style.width),
            opacity: 1.0,
        }]),
        Brush::Soft { softness } => {
            let mut layers = Vec::new();
            let mut previous = 0.0;
            for index in 1..=16 {
                let t = f64::from(index) / 16.0;
                let target = 0.98 * t * t;
                layers.push(InkLayer {
                    path: ink_path(
                        points,
                        drawing.style.width * (1.0 - softness * (t - 0.0625)).max(0.02),
                    ),
                    opacity: (target - previous) / (1.0 - previous),
                });
                previous = target;
            }
            Ok(layers)
        }
        Brush::Textured { grain, seed } => {
            let mut seed = *seed;
            let mut path = String::new();
            stipple(
                &mut path,
                &points[0],
                drawing.style.width,
                *grain,
                &mut seed,
            );
            for pair in points.windows(2) {
                let a = &pair[0];
                let b = &pair[1];
                let count = ((b.x - a.x).hypot(b.y - a.y) / (drawing.style.width * 0.12).max(1.0))
                    .ceil() as u32;
                for index in 1..=count {
                    let t = f64::from(index) / f64::from(count);
                    let point = Point {
                        x: a.x + (b.x - a.x) * t,
                        y: a.y + (b.y - a.y) * t,
                        pressure: a.pressure + (b.pressure - a.pressure) * t,
                    };
                    stipple(&mut path, &point, drawing.style.width, *grain, &mut seed);
                }
            }
            Ok(vec![InkLayer { path, opacity: 1.0 }])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn stroke(brush: Brush) -> Drawing {
        Drawing {
            geometry: Geometry::Stroke {
                points: vec![
                    Point {
                        x: 10.0,
                        y: 20.0,
                        pressure: 1.0,
                    },
                    Point {
                        x: 100.0,
                        y: 20.0,
                        pressure: 0.5,
                    },
                ],
            },
            clip: None,
            style: Style {
                brush,
                ..Style::default()
            },
            transform: identity(),
        }
    }
    #[test]
    fn gradients_reject_invalid_ranges_and_accept_transparent_stops() {
        let mut gradient = Gradient::Linear {
            x0: 0.0,
            y0: 0.0,
            x1: 100.0,
            y1: 0.0,
            stops: vec![
                ColorStop {
                    offset: 0.0,
                    color: "#ff0000".into(),
                    opacity: 1.0,
                },
                ColorStop {
                    offset: 1.0,
                    color: "#0000ff".into(),
                    opacity: 0.0,
                },
            ],
        };
        assert!(Paint::Gradient(gradient.clone()).validate().is_ok());
        if let Gradient::Linear { stops, .. } = &mut gradient {
            stops[1].offset = -1.0;
        }
        assert!(Paint::Gradient(gradient).validate().is_err());
        assert!(Paint::from("url(external)").validate().is_err());
    }
    #[test]
    fn soft_layers_are_nested_and_accumulate_to_the_declared_center_coverage() {
        let layers = ink_layers(&stroke(Brush::Soft { softness: 1.0 })).unwrap();
        assert_eq!(layers.len(), 16);
        let coverage = 1.0
            - layers
                .iter()
                .map(|layer| 1.0 - layer.opacity)
                .product::<f64>();
        assert!((coverage - 0.98).abs() < 0.000001);
        assert_ne!(layers[0].path, layers[15].path);
    }
    #[test]
    fn texture_is_seeded_and_rejects_excessive_work() {
        let mut drawing = stroke(Brush::Textured {
            grain: 0.5,
            seed: 42,
        });
        assert_eq!(ink_layers(&drawing).unwrap(), ink_layers(&drawing).unwrap());
        let original = ink_layers(&drawing).unwrap();
        drawing.style.brush = Brush::Textured {
            grain: 0.5,
            seed: 43,
        };
        assert_ne!(original, ink_layers(&drawing).unwrap());
        if let Geometry::Stroke { points } = &mut drawing.geometry {
            points[1].x = 1_000_000.0;
        }
        assert!(drawing.validate().is_err());
    }
    #[test]
    fn old_solid_drawings_round_trip_and_clips_are_validated() {
        let mut drawing:Drawing=serde_json::from_str(r##"{"geometry":{"kind":"stroke","points":[{"x":2,"y":3}]},"style":{"stroke":"#ff0000"}}"##).unwrap();
        assert_eq!(drawing.style.brush, Brush::Marker);
        drawing.clip = Some(Clip {
            segments: vec![Segment::Z],
            transform: identity(),
        });
        assert!(drawing.validate().is_err());
    }
}
