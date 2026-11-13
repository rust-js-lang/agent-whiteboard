//! Shared document authority. Browser and server execute these same transitions.
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const MAX_ELEMENTS: usize = 10_000;
const MAX_EVENTS: usize = 10_000;
const MAX_POINTS: usize = 20_000;
const MAX_JSON: usize = 32 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Point {
    pub x: f64,
    pub y: f64,
    #[serde(default = "one")]
    pub pressure: f64,
}
fn one() -> f64 {
    1.0
}
fn identity() -> [f64; 6] {
    [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]
}
fn default_width() -> f64 {
    8.0
}
fn default_color() -> String {
    "#243b35".into()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", deny_unknown_fields)]
pub enum Segment {
    M {
        x: f64,
        y: f64,
    },
    L {
        x: f64,
        y: f64,
    },
    Q {
        cx: f64,
        cy: f64,
        x: f64,
        y: f64,
    },
    C {
        cx1: f64,
        cy1: f64,
        cx2: f64,
        cy2: f64,
        x: f64,
        y: f64,
    },
    Z,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Geometry {
    Stroke {
        points: Vec<Point>,
    },
    Path {
        segments: Vec<Segment>,
    },
    Text {
        x: f64,
        y: f64,
        text: String,
        size: f64,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct Style {
    pub stroke: String,
    pub fill: Option<String>,
    pub width: f64,
    pub opacity: f64,
}
impl Default for Style {
    fn default() -> Self {
        Self {
            stroke: default_color(),
            fill: None,
            width: default_width(),
            opacity: 1.0,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Drawing {
    pub geometry: Geometry,
    #[serde(default)]
    pub style: Style,
    #[serde(default = "identity")]
    pub transform: [f64; 6],
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Mask {
    pub points: Vec<Point>,
    pub radius: f64,
    pub transform: [f64; 6],
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Element {
    pub id: String,
    pub version: u32,
    pub order: u32,
    pub author: String,
    pub drawing: Drawing,
    pub masks: Vec<Mask>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Target {
    pub id: String,
    pub expected_version: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "op",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Operation {
    Create {
        id: String,
        drawing: Drawing,
    },
    Replace {
        id: String,
        expected_version: u32,
        drawing: Drawing,
    },
    Delete {
        id: String,
        expected_version: u32,
    },
    Erase {
        targets: Vec<Target>,
        points: Vec<Point>,
        radius: f64,
    },
    Clear {
        expected_sequence: u32,
    },
    Undo,
    Redo,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Command {
    pub command_id: String,
    pub operations: Vec<Operation>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Change {
    pub id: String,
    pub before: Option<Element>,
    pub after: Option<Element>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Event {
    pub sequence: u32,
    pub actor: String,
    pub kind: String,
    pub changes: Vec<Change>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct History {
    actor: String,
    active: bool,
    redoable: bool,
    last_undo: u32,
    changes: Vec<Change>,
    undo_changes: Vec<Change>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Receipt {
    pub command_id: String,
    pub sequence: u32,
    pub affected: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SavedReceipt {
    actor: String,
    command: Command,
    receipt: Receipt,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Journal {
    pub version: u32,
    pub sequence: u32,
    pub elements: Vec<Element>,
    pub events: Vec<Event>,
    history: Vec<History>,
    receipts: Vec<SavedReceipt>,
}
impl Default for Journal {
    fn default() -> Self {
        Self {
            version: 1,
            sequence: 0,
            elements: Vec::new(),
            events: Vec::new(),
            history: Vec::new(),
            receipts: Vec::new(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub version: u32,
    pub sequence: u32,
    pub width: f64,
    pub height: f64,
    pub elements: Vec<Element>,
    pub can_undo: bool,
    pub can_redo: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Acceptance {
    pub journal: Journal,
    pub snapshot: Snapshot,
    pub receipt: Receipt,
    pub accepted: bool,
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_:.".contains(&c))
}
fn coordinate(value: f64) -> bool {
    value.is_finite() && value.abs() <= 1_000_000.0
}
fn color(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].bytes().all(|c| c.is_ascii_hexdigit())
}
fn points_valid(points: &[Point]) -> bool {
    !points.is_empty()
        && points.len() <= MAX_POINTS
        && points.iter().all(|p| {
            coordinate(p.x)
                && coordinate(p.y)
                && p.pressure.is_finite()
                && p.pressure >= 0.0
                && p.pressure <= 1.0
        })
}
fn matrix_valid(m: &[f64; 6]) -> bool {
    m.iter().all(|v| coordinate(*v)) && (m[0] * m[3] - m[1] * m[2]).abs() > 0.000001
}
fn inverse(m: [f64; 6]) -> [f64; 6] {
    let determinant = m[0] * m[3] - m[1] * m[2];
    [
        m[3] / determinant,
        -m[1] / determinant,
        -m[2] / determinant,
        m[0] / determinant,
        (m[2] * m[5] - m[3] * m[4]) / determinant,
        (m[1] * m[4] - m[0] * m[5]) / determinant,
    ]
}
impl Drawing {
    pub fn validate(&self) -> Result<(), String> {
        let style = &self.style;
        if !color(&style.stroke)
            || style.fill.as_ref().is_some_and(|c| !color(c))
            || !style.width.is_finite()
            || style.width <= 0.0
            || style.width > 500.0
            || !style.opacity.is_finite()
            || !(0.0..=1.0).contains(&style.opacity)
            || !matrix_valid(&self.transform)
        {
            return Err("Invalid style or transform. Colors use #RRGGBB; width is 0–500; opacity is 0–1; transforms must be invertible.".into());
        }
        match &self.geometry {
            Geometry::Stroke { points } if points_valid(points) => Ok(()),
            Geometry::Path { segments } if !segments.is_empty() && segments.len() <= MAX_POINTS && matches!(segments.first(), Some(Segment::M { .. })) => {
                for segment in segments {
                    let values = match segment {
                        Segment::M { x, y } | Segment::L { x, y } => vec![*x, *y],
                        Segment::Q { cx, cy, x, y } => vec![*cx, *cy, *x, *y],
                        Segment::C { cx1, cy1, cx2, cy2, x, y } => vec![*cx1, *cy1, *cx2, *cy2, *x, *y],
                        Segment::Z => Vec::new(),
                    };
                    if !values.iter().all(|v| coordinate(*v)) { return Err("Path coordinates must be finite and within the supported board range.".into()); }
                }
                Ok(())
            }
            Geometry::Text { x, y, text, size } if coordinate(*x) && coordinate(*y) && !text.is_empty() && text.len() <= 8000 && size.is_finite() && *size >= 4.0 && *size <= 500.0 => Ok(()),
            _ => Err("Invalid geometry. Supply stroke points, a path beginning with M, or nonempty text.".into()),
        }
    }
    fn point_count(&self) -> usize {
        match &self.geometry {
            Geometry::Stroke { points } => points.len(),
            Geometry::Path { segments } => segments.len(),
            Geometry::Text { .. } => 0,
        }
    }
}
fn ordered(elements: &mut [Element]) {
    elements.sort_by(|a, b| a.order.cmp(&b.order).then(a.id.cmp(&b.id)));
}
fn put(elements: &mut Vec<Element>, id: &str, value: Option<Element>) {
    elements.retain(|e| e.id != id);
    if let Some(value) = value {
        elements.push(value);
    }
    ordered(elements);
}
fn target(elements: &[Element], id: &str, version: u32) -> Result<Element, String> {
    let element = elements
        .iter()
        .find(|e| e.id == id)
        .ok_or("Conflict: element no longer exists.")?;
    if element.version != version {
        return Err(format!(
            "Conflict: {} is version {}, not {}. Read current state before retrying.",
            id, element.version, version
        ));
    }
    Ok(element.clone())
}
fn inverse_changes(
    elements: &mut Vec<Element>,
    changes: &[Change],
    sequence: u32,
) -> Result<Vec<Change>, String> {
    // Check every precondition first: history commands are atomic too.
    for change in changes {
        let current = elements.iter().find(|e| e.id == change.id);
        if current != change.after.as_ref() {
            return Err(format!(
                "Conflict: {} changed since that action. Undo will not overwrite another edit.",
                change.id
            ));
        }
    }
    let mut applied = Vec::new();
    for change in changes.iter().rev() {
        let restored = change.before.clone().map(|mut element| {
            element.version = sequence;
            element
        });
        applied.push(Change {
            id: change.id.clone(),
            before: change.after.clone(),
            after: restored.clone(),
        });
        put(elements, &change.id, restored);
    }
    Ok(applied)
}
fn rebase_expected(expected: &mut [Change], source: &[Change], applied: &[Change]) {
    for change in expected {
        if let Some(original) = source.iter().find(|original| original.id == change.id)
            && change.after == original.before
            && let Some(actual) = applied.iter().find(|actual| actual.id == change.id)
        {
            change.after = actual.after.clone();
        }
    }
}
impl Journal {
    pub fn snapshot(&self, actor: &str) -> Snapshot {
        Snapshot {
            version: self.version,
            sequence: self.sequence,
            width: 4096.0,
            height: 2304.0,
            elements: self.elements.clone(),
            can_undo: self.history.iter().any(|h| h.actor == actor && h.active),
            can_redo: self
                .history
                .iter()
                .any(|h| h.actor == actor && !h.active && h.redoable),
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err(
                "This board uses an unsupported format version. Saved data was preserved.".into(),
            );
        }
        if self.events.len() > MAX_EVENTS
            || self.sequence as usize != self.events.len()
            || self.receipts.len() != self.events.len()
            || self.elements.len() > MAX_ELEMENTS
        {
            return Err(
                "Saved board history is inconsistent or exceeds limits. Saved data was preserved."
                    .into(),
            );
        }
        let mut ids = HashSet::new();
        for element in &self.elements {
            if !valid_id(&element.id) || !ids.insert(&element.id) || element.version > self.sequence
            {
                return Err("Saved element identities are invalid.".into());
            }
            element.drawing.validate()?;
            for mask in &element.masks {
                if !points_valid(&mask.points)
                    || !mask.radius.is_finite()
                    || mask.radius <= 0.0
                    || mask.radius > 500.0
                    || !matrix_valid(&mask.transform)
                {
                    return Err("Saved eraser mask is invalid.".into());
                }
            }
        }
        let mut restored = Vec::new();
        for (index, event) in self.events.iter().enumerate() {
            if event.sequence as usize != index + 1 {
                return Err("Saved event sequence has a gap.".into());
            }
            for change in &event.changes {
                if restored.iter().find(|e: &&Element| e.id == change.id) != change.before.as_ref()
                {
                    return Err("Saved event history is inconsistent.".into());
                }
                put(&mut restored, &change.id, change.after.clone());
            }
        }
        if restored != self.elements {
            return Err("Saved board differs from its history. Saved data was preserved.".into());
        }
        Ok(())
    }
    pub fn accept(&self, command: Command, actor: &str) -> Result<Acceptance, String> {
        self.validate()?;
        if !valid_id(actor)
            || !valid_id(&command.command_id)
            || command.operations.is_empty()
            || command.operations.len() > 256
        {
            return Err("Supply a valid commandId and 1–256 operations.".into());
        }
        if let Some(saved) = self
            .receipts
            .iter()
            .find(|r| r.actor == actor && r.command.command_id == command.command_id)
        {
            if saved.command != command {
                return Err(
                    "Command ID collision: reuse an ID only for an identical retry.".into(),
                );
            }
            return Ok(Acceptance {
                journal: self.clone(),
                snapshot: self.snapshot(actor),
                receipt: saved.receipt.clone(),
                accepted: false,
            });
        }
        if self.events.len() >= MAX_EVENTS {
            return Err("History is full. Export this board before starting a new one.".into());
        }
        let mut next = self.clone();
        let sequence = self.sequence + 1;
        let history_action = command
            .operations
            .iter()
            .any(|op| matches!(op, Operation::Undo | Operation::Redo));
        if history_action && command.operations.len() != 1 {
            return Err("Undo and redo must be submitted alone.".into());
        }
        let mut changes = Vec::new();
        let mut kind = "draw".to_string();
        if history_action {
            let redo = matches!(command.operations[0], Operation::Redo);
            let index = if redo {
                next.history
                    .iter()
                    .enumerate()
                    .filter(|(_, h)| h.actor == actor && !h.active && h.redoable)
                    .max_by_key(|(_, h)| h.last_undo)
                    .map(|(index, _)| index)
            } else {
                next.history
                    .iter()
                    .rposition(|h| h.actor == actor && h.active)
            }
            .ok_or(if redo {
                "Nothing to redo."
            } else {
                "Nothing to undo."
            })?;
            let entry = &next.history[index];
            let source = if redo {
                &entry.undo_changes
            } else {
                &entry.changes
            };
            let source = source.clone();
            changes = inverse_changes(&mut next.elements, &source, sequence)?;
            // An author's own compensating edit must not invalidate their next undo/redo.
            // Other actors retain strict version guards and still receive conflicts.
            for history in next
                .history
                .iter_mut()
                .filter(|history| history.actor == actor)
            {
                rebase_expected(&mut history.changes, &source, &changes);
                rebase_expected(&mut history.undo_changes, &source, &changes);
            }
            if redo {
                next.history[index].changes = changes.clone();
                next.history[index].active = true;
                next.history[index].redoable = false;
                kind = "redo".into();
            } else {
                next.history[index].undo_changes = changes.clone();
                next.history[index].active = false;
                next.history[index].redoable = true;
                next.history[index].last_undo = sequence;
                kind = "undo".into();
            }
        } else {
            let mut point_count = 0;
            for (index, operation) in command.operations.iter().enumerate() {
                match operation {
                    Operation::Create { id, drawing } => {
                        drawing.validate()?;
                        point_count += drawing.point_count();
                        if !valid_id(id) || next.elements.iter().any(|e| e.id == *id) {
                            return Err("Element IDs must be valid and unique.".into());
                        }
                        let element = Element {
                            id: id.clone(),
                            version: sequence,
                            order: sequence * 256 + index as u32,
                            author: actor.into(),
                            drawing: drawing.clone(),
                            masks: Vec::new(),
                        };
                        changes.push(Change {
                            id: id.clone(),
                            before: None,
                            after: Some(element.clone()),
                        });
                        put(&mut next.elements, id, Some(element));
                    }
                    Operation::Replace {
                        id,
                        expected_version,
                        drawing,
                    } => {
                        drawing.validate()?;
                        point_count += drawing.point_count();
                        let before = target(&next.elements, id, *expected_version)?;
                        let mut after = before.clone();
                        after.drawing = drawing.clone();
                        after.version = sequence;
                        changes.push(Change {
                            id: id.clone(),
                            before: Some(before),
                            after: Some(after.clone()),
                        });
                        put(&mut next.elements, id, Some(after));
                    }
                    Operation::Delete {
                        id,
                        expected_version,
                    } => {
                        let before = target(&next.elements, id, *expected_version)?;
                        changes.push(Change {
                            id: id.clone(),
                            before: Some(before),
                            after: None,
                        });
                        put(&mut next.elements, id, None);
                    }
                    Operation::Erase {
                        targets,
                        points,
                        radius,
                    } => {
                        if targets.is_empty()
                            || targets.len() > 256
                            || !points_valid(points)
                            || !radius.is_finite()
                            || *radius <= 0.0
                            || *radius > 500.0
                        {
                            return Err(
                                "Erase requires targets, points and a radius from 0 to 500.".into(),
                            );
                        }
                        point_count += points.len();
                        for reference in targets {
                            let before =
                                target(&next.elements, &reference.id, reference.expected_version)?;
                            let mut after = before.clone();
                            after.masks.push(Mask {
                                points: points.clone(),
                                radius: *radius,
                                transform: inverse(before.drawing.transform),
                            });
                            after.version = sequence;
                            changes.push(Change {
                                id: reference.id.clone(),
                                before: Some(before),
                                after: Some(after.clone()),
                            });
                            put(&mut next.elements, &reference.id, Some(after));
                        }
                        kind = "erase".into();
                    }
                    Operation::Clear { expected_sequence } => {
                        if *expected_sequence != self.sequence {
                            return Err(
                                "Conflict: board changed before clear. Read current state first."
                                    .into(),
                            );
                        }
                        for element in next.elements.clone() {
                            changes.push(Change {
                                id: element.id.clone(),
                                before: Some(element),
                                after: None,
                            });
                        }
                        next.elements.clear();
                        kind = "clear".into();
                    }
                    Operation::Undo | Operation::Redo => unreachable!(),
                }
                if point_count > MAX_POINTS || next.elements.len() > MAX_ELEMENTS {
                    return Err("This batch exceeds board geometry limits.".into());
                }
            }
            let mut folded: Vec<Change> = Vec::new();
            for change in changes {
                if let Some(previous) = folded.iter_mut().find(|previous| previous.id == change.id)
                {
                    previous.after = change.after;
                } else {
                    folded.push(change);
                }
            }
            changes = folded
                .into_iter()
                .filter(|change| change.before != change.after)
                .collect();
            if changes.is_empty() {
                return Err("This command changes nothing.".into());
            }
            for history in &mut next.history {
                if history.actor == actor && !history.active {
                    history.redoable = false;
                }
            }
            next.history.push(History {
                actor: actor.into(),
                active: true,
                redoable: false,
                last_undo: 0,
                changes: changes.clone(),
                undo_changes: Vec::new(),
            });
        }
        next.sequence = sequence;
        next.events.push(Event {
            sequence,
            actor: actor.into(),
            kind,
            changes: changes.clone(),
        });
        let receipt = Receipt {
            command_id: command.command_id.clone(),
            sequence,
            affected: changes.iter().map(|c| c.id.clone()).collect(),
        };
        next.receipts.push(SavedReceipt {
            actor: actor.into(),
            command,
            receipt: receipt.clone(),
        });
        next.validate()?;
        Ok(Acceptance {
            snapshot: next.snapshot(actor),
            journal: next,
            receipt,
            accepted: true,
        })
    }
}

fn decode(record: &str) -> Result<Journal, String> {
    if record.len() > MAX_JSON {
        return Err("Saved board exceeds the supported size.".into());
    }
    let journal: Journal = serde_json::from_str(record)
        .map_err(|e| format!("Cannot read saved board: {e}. Saved data was preserved."))?;
    journal.validate()?;
    Ok(journal)
}
pub fn initial_record() -> String {
    serde_json::to_string(&Journal::default()).expect("empty journal is JSON")
}
pub fn inspect_json(record: &str, actor: &str) -> Result<String, String> {
    serde_json::to_string(&decode(record)?.snapshot(actor)).map_err(|e| e.to_string())
}
pub fn apply_json(record: &str, command: &str, actor: &str) -> Result<String, String> {
    if command.len() > 1024 * 1024 {
        return Err("Command exceeds 1 MiB.".into());
    }
    let command: Command =
        serde_json::from_str(command).map_err(|e| format!("Invalid command: {e}"))?;
    let accepted = decode(record)?.accept(command, actor)?;
    serde_json::to_string(&accepted).map_err(|e| e.to_string())
}
pub fn replay_snapshot(journal: &Journal, sequence: u32) -> Result<Snapshot, String> {
    if sequence > journal.sequence {
        return Err("Replay sequence exceeds accepted history.".into());
    }
    let mut elements = Vec::new();
    for event in journal.events.iter().take(sequence as usize) {
        for change in &event.changes {
            put(&mut elements, &change.id, change.after.clone());
        }
    }
    let snapshot = Snapshot {
        version: 1,
        sequence,
        width: 4096.0,
        height: 2304.0,
        elements,
        can_undo: false,
        can_redo: false,
    };
    Ok(snapshot)
}

pub fn replay_json(record: &str, sequence: u32, _actor: &str) -> Result<String, String> {
    serde_json::to_string(&replay_snapshot(&decode(record)?, sequence)?).map_err(|e| e.to_string())
}

/// A single nonzero-filled outline: overlapping subpaths do not darken marker ink.
fn ink_path(points: &[Point], width: f64) -> String {
    let mut path = String::new();
    for point in points {
        let radius = width * (0.2 + point.pressure * 0.8) / 2.0;
        path.push_str(&format!(
            "M {} {} a {} {} 0 1 0 {} 0 a {} {} 0 1 0 {} 0 Z ",
            point.x + radius,
            point.y,
            radius,
            radius,
            -2.0 * radius,
            radius,
            radius,
            2.0 * radius
        ));
    }
    for pair in points.windows(2) {
        let a = &pair[0];
        let b = &pair[1];
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let length = dx.hypot(dy);
        if length > 0.000001 {
            let ra = width * (0.2 + a.pressure * 0.8) / 2.0;
            let rb = width * (0.2 + b.pressure * 0.8) / 2.0;
            let nx = -dy / length;
            let ny = dx / length;
            path.push_str(&format!(
                "M {} {} L {} {} L {} {} L {} {} Z ",
                a.x + nx * ra,
                a.y + ny * ra,
                b.x + nx * rb,
                b.y + ny * rb,
                b.x - nx * rb,
                b.y - ny * rb,
                a.x - nx * ra,
                a.y - ny * ra
            ));
        }
    }
    path
}
pub fn path_data(drawing: &Drawing) -> Result<String, String> {
    drawing.validate()?;
    let path = match &drawing.geometry {
        Geometry::Stroke { points } => ink_path(points, drawing.style.width),
        Geometry::Path { segments } => segments
            .iter()
            .map(|s| match s {
                Segment::M { x, y } => format!("M {x} {y}"),
                Segment::L { x, y } => format!("L {x} {y}"),
                Segment::Q { cx, cy, x, y } => format!("Q {cx} {cy} {x} {y}"),
                Segment::C {
                    cx1,
                    cy1,
                    cx2,
                    cy2,
                    x,
                    y,
                } => format!("C {cx1} {cy1} {cx2} {cy2} {x} {y}"),
                Segment::Z => "Z".into(),
            })
            .collect::<Vec<_>>()
            .join(" "),
        Geometry::Text { .. } => String::new(),
    };
    Ok(path)
}
pub fn mask_data(mask: &Mask) -> Result<String, String> {
    if !points_valid(&mask.points)
        || !mask.radius.is_finite()
        || mask.radius <= 0.0
        || mask.radius > 500.0
    {
        return Err("Invalid mask.".into());
    }
    Ok(ink_path(&mask.points, mask.radius * 2.0))
}

pub fn path_json(text: &str) -> Result<String, String> {
    path_data(&serde_json::from_str(text).map_err(|e| e.to_string())?)
}
pub fn mask_json(text: &str) -> Result<String, String> {
    mask_data(&serde_json::from_str(text).map_err(|e| e.to_string())?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn drawing() -> Drawing {
        Drawing {
            geometry: Geometry::Stroke {
                points: vec![
                    Point {
                        x: 10.0,
                        y: 20.0,
                        pressure: 1.0,
                    },
                    Point {
                        x: 50.0,
                        y: 60.0,
                        pressure: 1.0,
                    },
                ],
            },
            style: Style::default(),
            transform: identity(),
        }
    }
    fn command(id: &str, operations: Vec<Operation>) -> Command {
        Command {
            command_id: id.into(),
            operations,
        }
    }
    fn create(id: &str) -> Operation {
        Operation::Create {
            id: id.into(),
            drawing: drawing(),
        }
    }
    #[test]
    fn create_retry_and_collision_are_atomic() {
        let first = Journal::default()
            .accept(command("one", vec![create("ink")]), "alice")
            .unwrap();
        let retry = first
            .journal
            .accept(command("one", vec![create("ink")]), "alice")
            .unwrap();
        assert!(!retry.accepted);
        assert_eq!(retry.journal.sequence, 1);
        assert_eq!(retry.snapshot.elements.len(), 1);
        assert!(
            first
                .journal
                .accept(command("one", vec![create("other")]), "alice")
                .is_err()
        );
        assert!(
            first
                .journal
                .accept(command("bad", vec![create("new"), create("ink")]), "alice")
                .is_err()
        );
        assert_eq!(first.journal.elements.len(), 1);
    }
    #[test]
    fn undo_redo_and_new_work_preserve_other_artists() {
        let one = Journal::default()
            .accept(command("one", vec![create("a")]), "alice")
            .unwrap()
            .journal;
        let two = one
            .accept(command("two", vec![create("b")]), "bob")
            .unwrap()
            .journal;
        let undo = two
            .accept(command("undo", vec![Operation::Undo]), "alice")
            .unwrap()
            .journal;
        assert_eq!(undo.elements[0].id, "b");
        let redo = undo
            .accept(command("redo", vec![Operation::Redo]), "alice")
            .unwrap()
            .journal;
        assert_eq!(redo.elements.len(), 2);
        let again = redo
            .accept(command("again", vec![Operation::Undo]), "alice")
            .unwrap()
            .journal;
        assert_eq!(again.elements[0].id, "b");
        let newer = again
            .accept(command("newer", vec![create("c")]), "alice")
            .unwrap()
            .journal;
        assert!(!newer.snapshot("alice").can_redo);
    }
    #[test]
    fn conflicting_edit_does_not_get_overwritten_by_undo() {
        let one = Journal::default()
            .accept(command("one", vec![create("a")]), "alice")
            .unwrap()
            .journal;
        let edit = Operation::Replace {
            id: "a".into(),
            expected_version: 1,
            drawing: drawing(),
        };
        let two = one
            .accept(command("two", vec![edit]), "bob")
            .unwrap()
            .journal;
        assert!(
            two.accept(command("undo", vec![Operation::Undo]), "alice")
                .is_err()
        );
        assert_eq!(two.sequence, 2);
    }
    #[test]
    fn eraser_masks_undo_without_losing_geometry() {
        let one = Journal::default()
            .accept(command("one", vec![create("a")]), "alice")
            .unwrap()
            .journal;
        let erased = one
            .accept(
                command(
                    "erase",
                    vec![Operation::Erase {
                        targets: vec![Target {
                            id: "a".into(),
                            expected_version: 1,
                        }],
                        points: vec![Point {
                            x: 20.0,
                            y: 30.0,
                            pressure: 1.0,
                        }],
                        radius: 10.0,
                    }],
                ),
                "alice",
            )
            .unwrap()
            .journal;
        assert_eq!(erased.elements[0].masks.len(), 1);
        let undone = erased
            .accept(command("undo", vec![Operation::Undo]), "alice")
            .unwrap()
            .journal;
        assert!(undone.elements[0].masks.is_empty());
        assert_eq!(undone.elements[0].drawing, drawing());
    }
    #[test]
    fn replay_starts_blank_and_finishes_exactly_even_with_history_actions() {
        let one = Journal::default()
            .accept(command("one", vec![create("a")]), "alice")
            .unwrap()
            .journal;
        let two = one
            .accept(command("undo", vec![Operation::Undo]), "alice")
            .unwrap()
            .journal;
        let text = serde_json::to_string(&two).unwrap();
        let blank: Snapshot =
            serde_json::from_str(&replay_json(&text, 0, "alice").unwrap()).unwrap();
        let first: Snapshot =
            serde_json::from_str(&replay_json(&text, 1, "alice").unwrap()).unwrap();
        let last: Snapshot =
            serde_json::from_str(&replay_json(&text, 2, "alice").unwrap()).unwrap();
        assert!(blank.elements.is_empty());
        assert_eq!(first.elements.len(), 1);
        assert_eq!(last.elements, two.elements);
        assert_eq!(decode(&text).unwrap(), two);
    }
    #[test]
    fn invalid_inputs_and_future_data_are_rejected() {
        let mut invalid = drawing();
        invalid.style.width = f64::NAN;
        assert!(invalid.validate().is_err());
        assert!(apply_json(&initial_record(), r#"{"commandId":"x","operations":[{"op":"create","id":"x","drawing":{"geometry":{"kind":"stroke","points":[]}}}]}"#, "alice").is_err());
        assert!(inspect_json(r#"{"version":2,"sequence":0,"elements":[],"events":[],"history":[],"receipts":[]}"#, "alice").is_err());
        let corrupt = Journal {
            sequence: 10,
            ..Journal::default()
        };
        assert!(corrupt.validate().is_err());
    }
    #[test]
    fn paths_preserve_cubic_geometry_and_pressure_outlines() {
        let draw = Drawing {
            geometry: Geometry::Path {
                segments: vec![
                    Segment::M { x: 0.0, y: 1.0 },
                    Segment::C {
                        cx1: 2.0,
                        cy1: 3.0,
                        cx2: 4.0,
                        cy2: 5.0,
                        x: 6.0,
                        y: 7.0,
                    },
                ],
            },
            style: Style::default(),
            transform: identity(),
        };
        assert_eq!(
            path_json(&serde_json::to_string(&draw).unwrap()).unwrap(),
            "M 0 1 C 2 3 4 5 6 7"
        );
        assert!(
            path_json(&serde_json::to_string(&drawing()).unwrap())
                .unwrap()
                .contains("a 4 4")
        );
    }
    #[test]
    fn redo_follows_the_order_of_undo_not_creation_order() {
        let a = Journal::default()
            .accept(command("a", vec![create("a")]), "alice")
            .unwrap()
            .journal;
        let b = a
            .accept(command("b", vec![create("b")]), "alice")
            .unwrap()
            .journal;
        let undo_b = b
            .accept(command("ub", vec![Operation::Undo]), "alice")
            .unwrap()
            .journal;
        let undo_a = undo_b
            .accept(command("ua", vec![Operation::Undo]), "alice")
            .unwrap()
            .journal;
        let redo_a = undo_a
            .accept(command("ra", vec![Operation::Redo]), "alice")
            .unwrap()
            .journal;
        assert_eq!(redo_a.elements.len(), 1);
        assert_eq!(redo_a.elements[0].id, "a");
        let redo_b = redo_a
            .accept(command("rb", vec![Operation::Redo]), "alice")
            .unwrap()
            .journal;
        assert_eq!(redo_b.elements.len(), 2);
    }
    #[test]
    fn one_batch_can_create_and_edit_the_same_element_then_undo_as_one_action() {
        let mut changed = drawing();
        changed.style.stroke = "#ff0000".into();
        let accepted = Journal::default()
            .accept(
                command(
                    "batch",
                    vec![
                        create("a"),
                        Operation::Replace {
                            id: "a".into(),
                            expected_version: 1,
                            drawing: changed,
                        },
                    ],
                ),
                "alice",
            )
            .unwrap()
            .journal;
        let undo = accepted
            .accept(command("undo", vec![Operation::Undo]), "alice")
            .unwrap()
            .journal;
        assert!(undo.elements.is_empty());
        let redo = undo
            .accept(command("redo", vec![Operation::Redo]), "alice")
            .unwrap()
            .journal;
        assert_eq!(redo.elements[0].drawing.style.stroke, "#ff0000");
    }

    #[test]
    fn an_authors_edit_chain_can_be_undone_and_redone_in_order() {
        let created = Journal::default()
            .accept(command("a", vec![create("a")]), "alice")
            .unwrap()
            .journal;
        let mut drawing = drawing();
        drawing.style.stroke = "#ff0000".into();
        let edited = created
            .accept(
                command(
                    "edit",
                    vec![Operation::Replace {
                        id: "a".into(),
                        expected_version: 1,
                        drawing,
                    }],
                ),
                "alice",
            )
            .unwrap()
            .journal;
        let undo_edit = edited
            .accept(command("ue", vec![Operation::Undo]), "alice")
            .unwrap()
            .journal;
        let undo_create = undo_edit
            .accept(command("uc", vec![Operation::Undo]), "alice")
            .unwrap()
            .journal;
        assert!(undo_create.elements.is_empty());
        let redo_create = undo_create
            .accept(command("rc", vec![Operation::Redo]), "alice")
            .unwrap()
            .journal;
        let redo_edit = redo_create
            .accept(command("re", vec![Operation::Redo]), "alice")
            .unwrap()
            .journal;
        assert_eq!(redo_edit.elements[0].drawing.style.stroke, "#ff0000");
    }
}
