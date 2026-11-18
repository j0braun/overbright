//! Props mode: the move / rotate handles in the viewport, the actions on
//! the selected placements (delete, duplicate, drop to ground, reset to
//! original, upright) and the properties panel of the selection.
//!
//! Every gesture changes the mod and the world live (`tools::map::edit`)
//! and ends in one [`commit`]: a handle drag on release, a typed value on
//! Enter or when the field loses focus, a button at once.

use egui::{Color32, Pos2, Rect, RichText, Stroke, Ui};
use glam::{Mat3, Vec3, Vec4};

use super::state::{EditorState, Panel, commit, project};
use crate::launcher::draw::{Rgb, THEME_A};
use crate::tools::map::edit::{self, PropState};
use crate::world::World;

/// Default snap steps.
pub const SNAP_METRES: f32 = 0.25;
pub const SNAP_DEGREES: f32 = 15.0;
/// How near (screen points) the pointer must be to grab a handle.
const GRAB: f32 = 7.0;
/// The handles' length on screen, roughly (points).
const HANDLE_POINTS: f32 = 80.0;

/// A theme colour for egui.
pub fn colour(c: Rgb) -> Color32 {
    Color32::from_rgb(c.0, c.1, c.2)
}

/// The axis colours: x red, y green, z blue (the usual editor convention).
const AXIS: [Color32; 3] =
    [Color32::from_rgb(0xff, 0x6b, 0x5e), Color32::from_rgb(0x8f, 0xe0, 0x6a), Color32::from_rgb(0x5e, 0x9b, 0xff)];

/// The tool the left button works with in the viewport.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tool {
    /// Click to select only.
    Select,
    /// Axis handles move the selection.
    #[default]
    Move,
    /// A ring turns the selection about z.
    Rotate,
}

impl Tool {
    pub const ALL: [Tool; 3] = [Tool::Select, Tool::Move, Tool::Rotate];

    pub fn name(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Move => "Move",
            Tool::Rotate => "Rotate",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Tool::Select => "Q",
            Tool::Move => "W",
            Tool::Rotate => "E",
        }
    }
}

/// The part of the handles under the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    /// A move axis: 0 x, 1 y, 2 z.
    Axis(usize),
    /// The rotate ring.
    Ring,
}

/// A handle drag in progress.
#[derive(Clone, Debug)]
struct Drag {
    handle: Handle,
    from: Pos2,
    /// The placements as they were when the drag began.
    starts: Vec<(usize, Vec3, Mat3)>,
    pivot: Vec3,
    /// The ring's start angle (degrees) under the pointer.
    angle: f32,
    label: String,
}

/// Props mode: the tool, the snap settings and the properties panel.
#[derive(Clone, Debug)]
pub struct Props {
    pub tool: Tool,
    pub snap: bool,
    pub snap_metres: f32,
    pub snap_degrees: f32,
    drag: Option<Drag>,
    /// The handle under the pointer this frame.
    pub hover: Option<Handle>,
}

impl Default for Props {
    fn default() -> Props {
        Props {
            tool: Tool::Move,
            snap: true,
            snap_metres: SNAP_METRES,
            snap_degrees: SNAP_DEGREES,
            drag: None,
            hover: None,
        }
    }
}

/// The solid name of placement `i`.
pub fn name(world: &World, i: usize) -> String {
    world.placements.get(i).and_then(|p| world.solids.get(p.solid)).map_or_else(|| format!("#{i}"), |s| s.name.clone())
}

/// Whether placement `i` is a knock-over prop (it carries its own
/// collision and is knocked about by cars).
pub fn knock_over(world: &World, i: usize) -> bool {
    world.smackables.iter().any(|p| p.placement == i)
}

/// What an action on the selection is called in the undo list: the one
/// prop's name, else the count.
fn label(s: &EditorState, verb: &str) -> String {
    match s.selection.as_slice() {
        [i] => format!("{verb} {}", name(&s.world, *i)),
        sel => format!("{verb} {} props", sel.len()),
    }
}

/// The pivot of the selection: the mean of the placements' origins.
pub fn pivot(s: &EditorState) -> Option<Vec3> {
    let pts: Vec<Vec3> = s.selection.iter().filter_map(|&i| edit::placement_parts(&s.world, i).map(|p| p.0)).collect();
    (!pts.is_empty()).then(|| pts.iter().sum::<Vec3>() / pts.len() as f32)
}

/// The mouse ray through screen point `at` of the viewport `rect`: the eye
/// and a unit direction.
pub fn screen_ray(s: &EditorState, rect: Rect, at: Pos2) -> (Vec3, Vec3) {
    let x = (at.x - rect.min.x) / rect.width().max(1.0) * 2.0 - 1.0;
    let y = 1.0 - (at.y - rect.min.y) / rect.height().max(1.0) * 2.0;
    let aspect = (rect.width() / rect.height().max(1.0)).max(1e-3);
    let inv = s.camera.view_projection(aspect).inverse();
    let (far, near) = (inv * Vec4::new(x, y, 1.0, 1.0), inv * Vec4::new(x, y, 0.0, 1.0));
    let dir = (far.truncate() / far.w - near.truncate() / near.w).normalize_or(s.camera.forward());
    (s.camera.position, dir)
}

/// Where the ray through `at` meets the ground (1.5 km at most).
pub fn ground_under(s: &EditorState, rect: Rect, at: Pos2) -> Option<Vec3> {
    let (eye, dir) = screen_ray(s, rect, at);
    edit::ray_ground(&s.world, eye, dir, 1500.0)
}

/// The camera's heading (degrees, the game's convention).
pub fn camera_heading(s: &EditorState) -> f32 {
    let f = s.camera.forward();
    f.y.atan2(f.x).to_degrees()
}

/// Distance from `p` to the segment `a`–`b` (screen points).
fn to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let (ab, ap) = (b - a, p - a);
    let t = (ap.dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
    (a + ab * t).distance(p)
}

/// A turn of `degrees` about z through `pivot` applied to a position.
fn turn_about(p: Vec3, pivot: Vec3, degrees: f32) -> Vec3 {
    pivot + Mat3::from_rotation_z(degrees.to_radians()) * (p - pivot)
}

/// Wrap an angle difference into −180…180 degrees.
fn wrap(d: f32) -> f32 {
    (d + 180.0).rem_euclid(360.0) - 180.0
}

// ---- actions on the selection -------------------------------------------

/// Set each selected placement live; the first refusal goes to the status
/// bar. True when anything changed.
fn apply_each(s: &mut EditorState, mut f: impl FnMut(&World, usize) -> Option<(Vec3, Mat3)>) -> bool {
    let sel = s.selection.clone();
    let mut any = false;
    for i in sel {
        let Some((p, r)) = f(&s.world, i) else { continue };
        match edit::set_transform(&mut s.map, &s.folder, &mut s.world, &s.region, i, p, r) {
            Ok(()) => any = true,
            Err(e) => s.status = e.to_string(),
        }
    }
    if any {
        s.world_changed = true;
    }
    any
}

/// Delete the selection (Del).
pub fn delete_selection(s: &mut EditorState) {
    if s.selection.is_empty() {
        return;
    }
    let label = label(s, "Delete");
    for i in s.selection.clone() {
        if let Err(e) = edit::delete(&mut s.map, &s.folder, &mut s.world, &s.region, i) {
            s.status = e.to_string();
        }
    }
    s.world_changed = true;
    s.selection.clear();
    commit(s, &label);
}

/// The placement this mod's add edit `k` became (after a commit).
pub fn added_placement(s: &EditorState, k: usize) -> Option<usize> {
    s.world.map_overlay.added.iter().find(|(_, (f, kk))| *f == s.folder && *kk == k).map(|(i, _)| *i)
}

/// Copy the selection (Ctrl+D): around `at` when given (the ground under
/// the pointer), else one box width to the camera's right. The copies
/// become the selection.
pub fn duplicate_selection(s: &mut EditorState, at: Option<Vec3>) {
    let Some(pivot) = pivot(s) else { return };
    let shift = match at {
        Some(at) => at - pivot,
        None => {
            let r = s
                .selection
                .iter()
                .filter_map(|&i| edit::placement_sphere(&s.world, i))
                .map(|c| c.1)
                .fold(1.0, f32::max);
            (s.camera.right() * r * 1.2).truncate().extend(0.0)
        }
    };
    let label = label(s, "Duplicate");
    let mut edits = Vec::new();
    for i in s.selection.clone() {
        let Some((p, _)) = edit::placement_parts(&s.world, i) else { continue };
        match edit::duplicate(&mut s.map, &s.folder, &mut s.world, &s.region, i, p + shift) {
            Ok(a) => edits.push(a.edit),
            Err(e) => s.status = e.to_string(),
        }
    }
    s.adds_changed = true;
    commit(s, &label);
    s.selection = edits.into_iter().filter_map(|k| added_placement(s, k)).collect();
}

/// Stand each selected placement on the ground under it (End).
pub fn drop_selection(s: &mut EditorState) {
    let label = label(s, "Drop to ground");
    let changed = apply_each(s, |w, i| {
        let (p, r) = edit::placement_parts(w, i)?;
        edit::on_ground(w, i, p, r).map(|g| (g, r))
    });
    if changed {
        commit(s, &label);
    } else if !s.selection.is_empty() {
        s.status = "No ground under the selection".into();
    }
}

/// Put the selection back as the original has it.
pub fn reset_selection(s: &mut EditorState) {
    let label = label(s, "Reset");
    for i in s.selection.clone() {
        if let Err(e) = edit::reset(&mut s.map, &s.folder, &mut s.world, Some(&s.base), i) {
            s.status = e.to_string();
        }
    }
    s.world_changed = true;
    commit(s, &label);
}

/// Stand the selection upright (heading kept).
pub fn upright_selection(s: &mut EditorState) {
    let label = label(s, "Upright");
    if apply_each(s, |w, i| edit::placement_parts(w, i).map(|(p, r)| (p, edit::upright(edit::heading_of(r))))) {
        commit(s, &label);
    }
}

/// Place a new prop of `solid` standing on the ground at `at`, facing
/// `heading`; it becomes the selection.
pub fn place_new(s: &mut EditorState, solid: &str, at: Vec3, heading: f32) -> Option<usize> {
    let rot = edit::upright(heading);
    let added = match edit::add(&mut s.map, &s.folder, Some(&mut s.world), &s.region, solid, at, rot) {
        Ok(a) => a,
        Err(e) => {
            s.status = e.to_string();
            return None;
        }
    };
    if let Some(i) = added.placement
        && let Some(g) = edit::on_ground(&s.world, i, at, rot)
    {
        let _ = edit::set_transform(&mut s.map, &s.folder, &mut s.world, &s.region, i, g, rot);
    }
    s.adds_changed = true;
    commit(s, &format!("Add {solid}"));
    let i = added_placement(s, added.edit);
    s.selection = i.into_iter().collect();
    i
}

impl Props {
    /// Whether a handle drag is in progress (the viewport leaves the left
    /// button alone meanwhile).
    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    fn snapping(&self, ctx: &egui::Context) -> bool {
        self.snap && !ctx.input(|i| i.modifiers.alt)
    }

    /// The handles' world length at `pivot`: about [`HANDLE_POINTS`] on
    /// screen.
    fn handle_length(s: &EditorState, rect: Rect, pivot: Vec3) -> f32 {
        let d = s.camera.position.distance(pivot).max(0.5);
        let px_per_rad = rect.height().max(1.0) / s.camera.fov_y.max(0.1);
        d * HANDLE_POINTS / px_per_rad
    }

    /// The rotate ring's points (screen) around `pivot`.
    fn ring(s: &EditorState, rect: Rect, pivot: Vec3, radius: f32) -> Vec<Option<Pos2>> {
        (0..=48)
            .map(|k| {
                let a = k as f32 / 48.0 * std::f32::consts::TAU;
                project(s, rect, pivot + Vec3::new(a.cos(), a.sin(), 0.0) * radius)
            })
            .collect()
    }

    /// The handle under `p`.
    fn handle_at(&self, s: &EditorState, rect: Rect, pivot: Vec3, p: Pos2) -> Option<Handle> {
        let len = Self::handle_length(s, rect, pivot);
        let c = project(s, rect, pivot)?;
        match self.tool {
            Tool::Select => None,
            Tool::Move => (0..3)
                .filter_map(|k| {
                    let end = project(s, rect, pivot + Vec3::AXES[k] * len)?;
                    let d = to_segment(p, c, end);
                    (d < GRAB && end.distance(c) > 4.0).then_some((k, d))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(k, _)| Handle::Axis(k)),
            Tool::Rotate => {
                let pts = Self::ring(s, rect, pivot, len);
                let near = pts.windows(2).filter_map(|w| Some(to_segment(p, w[0]?, w[1]?))).fold(f32::MAX, f32::min);
                (near < GRAB).then_some(Handle::Ring)
            }
        }
    }

    /// The angle (degrees) about `pivot` at which the ray through `p` meets
    /// the level plane through it.
    fn ring_angle(s: &EditorState, rect: Rect, pivot: Vec3, p: Pos2) -> Option<f32> {
        let (eye, dir) = screen_ray(s, rect, p);
        if dir.z.abs() < 1e-4 {
            return None;
        }
        let t = (pivot.z - eye.z) / dir.z;
        if t <= 0.0 {
            return None;
        }
        let h = eye + dir * t - pivot;
        Some(h.y.atan2(h.x).to_degrees())
    }

    /// The handles of the selection in the viewport `rect`: drawn, and
    /// dragged with the left button (Shift at the start of a move drag
    /// copies the selection first; Alt bypasses snapping). True while the
    /// pointer is on a handle or a drag runs: the viewport must not pick
    /// with that click.
    pub fn handles(&mut self, s: &mut EditorState, ctx: &egui::Context, painter: &egui::Painter, rect: Rect) -> bool {
        self.hover = None;
        let Some(pivot) = (match &self.drag {
            Some(d) if !s.selection.is_empty() => Some(match d.handle {
                Handle::Ring => d.pivot,
                Handle::Axis(_) => pivot(s).unwrap_or(d.pivot),
            }),
            _ => pivot(s),
        }) else {
            self.drag = None;
            return false;
        };
        let (pointer, pressed, down, released) = ctx.input(|i| {
            (i.pointer.hover_pos(), i.pointer.primary_pressed(), i.pointer.primary_down(), i.pointer.primary_released())
        });
        // Continue or end a drag.
        if self.drag.is_some() {
            if let Some(p) = pointer {
                self.drag_to(s, ctx, rect, p);
            }
            if released || !down {
                let d = self.drag.take().unwrap();
                commit(s, &d.label);
            }
            self.draw(s, painter, rect, pivot);
            return true;
        }
        let over = pointer.filter(|p| rect.contains(*p)).and_then(|p| self.handle_at(s, rect, pivot, p));
        self.hover = over;
        if pressed && let (Some(h), Some(p)) = (over, pointer) {
            self.begin(s, ctx, rect, h, p, pivot);
        }
        self.draw(s, painter, rect, pivot);
        over.is_some() || self.drag.is_some()
    }

    fn begin(&mut self, s: &mut EditorState, ctx: &egui::Context, rect: Rect, handle: Handle, from: Pos2, pivot: Vec3) {
        let copy = handle != Handle::Ring && ctx.input(|i| i.modifiers.shift);
        let label = if copy {
            duplicate_in_place(s);
            label(s, "Copy")
        } else {
            label(s, if handle == Handle::Ring { "Rotate" } else { "Move" })
        };
        let starts = s
            .selection
            .iter()
            .filter_map(|&i| edit::placement_parts(&s.world, i).map(|(p, r)| (i, p, r)))
            .collect::<Vec<_>>();
        let angle = Self::ring_angle(s, rect, pivot, from).unwrap_or(0.0);
        self.drag = Some(Drag { handle, from, starts, pivot, angle, label });
    }

    fn drag_to(&mut self, s: &mut EditorState, ctx: &egui::Context, rect: Rect, p: Pos2) {
        let snapping = self.snapping(ctx);
        let (step_m, step_deg) = (self.snap_metres, self.snap_degrees);
        let Some(d) = self.drag.as_ref() else { return };
        let (starts, pivot) = (d.starts.clone(), d.pivot);
        match d.handle {
            Handle::Axis(k) => {
                let len = Self::handle_length(s, rect, pivot);
                let (Some(c), Some(e)) = (project(s, rect, pivot), project(s, rect, pivot + Vec3::AXES[k] * len))
                else {
                    return;
                };
                let axis = e - c;
                let screen_len = axis.length();
                if screen_len < 4.0 {
                    return;
                }
                let mut by = (p - d.from).dot(axis / screen_len) / screen_len * len;
                if snapping {
                    // The pivot lands on the grid along the axis.
                    by = edit::snap(pivot[k] + by, step_m) - pivot[k];
                }
                let delta = Vec3::AXES[k] * by;
                let map: std::collections::HashMap<usize, (Vec3, Mat3)> =
                    starts.iter().map(|&(i, p, r)| (i, (p + delta, r))).collect();
                apply_each(s, |_, i| map.get(&i).copied());
            }
            Handle::Ring => {
                let Some(a) = Self::ring_angle(s, rect, pivot, p) else { return };
                let mut turn = wrap(a - d.angle);
                if snapping {
                    turn = edit::snap(turn, step_deg);
                }
                let map: std::collections::HashMap<usize, (Vec3, Mat3)> =
                    starts.iter().map(|&(i, p, r)| (i, (turn_about(p, pivot, turn), edit::turned(r, turn)))).collect();
                apply_each(s, |_, i| map.get(&i).copied());
            }
        }
    }

    fn draw(&self, s: &EditorState, painter: &egui::Painter, rect: Rect, pivot: Vec3) {
        let Some(c) = project(s, rect, pivot) else { return };
        let len = Self::handle_length(s, rect, pivot);
        let active = self.drag.as_ref().map(|d| d.handle).or(self.hover);
        match self.tool {
            Tool::Select => {}
            Tool::Move => {
                for k in 0..3 {
                    let Some(e) = project(s, rect, pivot + Vec3::AXES[k] * len) else { continue };
                    let hot = active == Some(Handle::Axis(k));
                    let col = if hot { colour(THEME_A.play) } else { AXIS[k] };
                    painter.line_segment([c, e], Stroke::new(if hot { 4.0 } else { 2.5 }, col));
                    // The arrow head.
                    let dir = (e - c).normalized();
                    let side = egui::vec2(-dir.y, dir.x) * 5.0;
                    painter.add(egui::Shape::convex_polygon(
                        vec![e + dir * 9.0, e + side, e - side],
                        col,
                        Stroke::NONE,
                    ));
                }
            }
            Tool::Rotate => {
                let hot = active == Some(Handle::Ring);
                let col = if hot { colour(THEME_A.play) } else { AXIS[2] };
                let pts = Self::ring(s, rect, pivot, len);
                for w in pts.windows(2) {
                    if let (Some(a), Some(b)) = (w[0], w[1]) {
                        painter.line_segment([a, b], Stroke::new(if hot { 3.5 } else { 2.0 }, col));
                    }
                }
                if let Some(d) = &self.drag
                    && let Some(&(i, ..)) = d.starts.first()
                    && let Some((_, r)) = edit::placement_parts(&s.world, i)
                {
                    painter.text(
                        c + egui::vec2(12.0, -12.0),
                        egui::Align2::LEFT_BOTTOM,
                        format!("{:.1}°", edit::heading_of(r)),
                        egui::FontId::proportional(13.0),
                        colour(THEME_A.ink),
                    );
                }
            }
        }
        painter.circle_filled(c, 3.5, colour(THEME_A.play));
    }

    /// The Props keys (not while a text field has the keyboard or the
    /// camera flies): Q / W / E tools, Del delete, End drop to ground,
    /// Ctrl+D duplicate at `cursor` (the ground under the pointer), Esc
    /// clears the selection.
    pub fn keys(&mut self, s: &mut EditorState, ctx: &egui::Context, cursor: Option<Vec3>) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        use egui::{Key, Modifiers};
        let (q, w, e, del, end, dup, esc) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::Q),
                i.consume_key(Modifiers::NONE, Key::W),
                i.consume_key(Modifiers::NONE, Key::E),
                i.consume_key(Modifiers::NONE, Key::Delete),
                i.consume_key(Modifiers::NONE, Key::End),
                i.consume_key(Modifiers::COMMAND, Key::D),
                i.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        for (hit, tool) in [(q, Tool::Select), (w, Tool::Move), (e, Tool::Rotate)] {
            if hit {
                self.tool = tool;
            }
        }
        if del {
            delete_selection(s);
        }
        if end {
            drop_selection(s);
        }
        if dup {
            duplicate_selection(s, cursor);
        }
        if esc && self.drag.is_none() {
            s.selection.clear();
        }
    }

    /// The Props part of the toolbar: tools, snap, drop to ground.
    pub fn toolbar(&mut self, s: &mut EditorState, ui: &mut Ui) {
        for t in Tool::ALL {
            let text = RichText::new(format!("{}  {}", t.name(), t.key()));
            if ui.selectable_label(self.tool == t, text).on_hover_text(format!("{} ({})", t.name(), t.key())).clicked()
            {
                self.tool = t;
            }
        }
        ui.separator();
        ui.toggle_value(&mut self.snap, "Snap").on_hover_text("Snap moves and turns (hold Alt to bypass)");
        ui.add_enabled(
            self.snap,
            egui::DragValue::new(&mut self.snap_metres).range(0.01..=10.0).speed(0.01).suffix(" m"),
        );
        ui.add_enabled(
            self.snap,
            egui::DragValue::new(&mut self.snap_degrees).range(1.0..=90.0).speed(0.5).suffix("°"),
        );
        ui.separator();
        if ui.add_enabled(!s.selection.is_empty(), egui::Button::new("Drop to ground  End")).clicked() {
            drop_selection(s);
        }
    }
}

/// Copies of the selection at the same places; the copies become the
/// selection (the start of a Shift + move drag).
fn duplicate_in_place(s: &mut EditorState) {
    let mut edits = Vec::new();
    for i in s.selection.clone() {
        let Some((p, _)) = edit::placement_parts(&s.world, i) else { continue };
        if let Ok(a) = edit::duplicate(&mut s.map, &s.folder, &mut s.world, &s.region, i, p) {
            edits.push((a.edit, a.placement));
        }
    }
    s.adds_changed = true;
    s.selection = edits.into_iter().filter_map(|(_, i)| i).collect();
}

/// A section heading in the panel.
fn heading(ui: &mut Ui, text: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(text).strong().italics().color(colour(THEME_A.lime)));
}

/// A small outlined tag.
pub fn tag(ui: &mut Ui, text: &str, col: Color32) {
    egui::Frame::new()
        .stroke(Stroke::new(1.0, col))
        .corner_radius(8.0)
        .inner_margin(egui::Margin::symmetric(6, 1))
        .show(ui, |ui| ui.label(RichText::new(text).small().color(col)));
}

/// Did a value field finish its edit this frame (drag released, Enter,
/// focus lost, or a single click step)?
fn finished(r: &egui::Response) -> bool {
    r.drag_stopped() || r.lost_focus() || (r.changed() && !r.dragged() && !r.has_focus())
}

impl Panel for Props {
    /// The properties of the selection.
    fn ui(&mut self, s: &mut EditorState, ui: &mut Ui) {
        let Some(&first) = s.selection.first() else {
            ui.label(RichText::new("Nothing selected").color(colour(THEME_A.dim)));
            ui.label(
                RichText::new("Click a prop in the viewport or the tree, or drag one in from the prop browser.")
                    .small()
                    .color(colour(THEME_A.dim)),
            );
            return;
        };
        if first >= s.world.placements.len() {
            s.selection.clear();
            return;
        }
        let state = edit::prop_state(&s.map, &s.folder, &s.world, &s.region, first);
        let knock = knock_over(&s.world, first);
        ui.horizontal(|ui| {
            ui.label(RichText::new(name(&s.world, first)).strong().color(colour(THEME_A.ink)));
            tag(ui, if knock { "knock-over" } else { "static" }, colour(THEME_A.dim));
            if state != PropState::Original {
                let col = match state {
                    PropState::Deleted => colour(THEME_A.err),
                    PropState::OtherMod => colour(THEME_A.warn),
                    _ => colour(THEME_A.lime),
                };
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| tag(ui, state.name(), col));
            }
        });
        let origin = match state {
            PropState::Added(_) => "Added by this map mod".to_string(),
            PropState::OtherMod => "Added by another map mod".to_string(),
            _ => match s.world.placement_ref(&s.region, first) {
                Some(r) => format!(
                    "Original placement · section {} · at {:.2}, {:.2}, {:.2}",
                    r.section, r.at.x, r.at.y, r.at.z
                ),
                None => String::new(),
            },
        };
        ui.label(RichText::new(origin).small().color(colour(THEME_A.dim)));
        let editable = !matches!(state, PropState::OtherMod | PropState::Deleted);
        ui.separator();

        // Position: of the one prop, or the selection's pivot (all move).
        heading(ui, "Position (m)");
        let pivot = pivot(s).unwrap_or_default();
        let mut p = pivot;
        let mut done = false;
        ui.add_enabled_ui(editable, |ui| {
            ui.horizontal(|ui| {
                for (k, axis) in ["X", "Y", "Z"].into_iter().enumerate() {
                    ui.label(RichText::new(axis).color(AXIS[k]));
                    let r = ui.add(egui::DragValue::new(&mut p[k]).speed(0.05).fixed_decimals(2));
                    done |= finished(&r);
                }
            });
        });
        if p != pivot {
            let by = p - pivot;
            apply_each(s, |w, i| edit::placement_parts(w, i).map(|(q, r)| (q + by, r)));
        }
        if done {
            commit(s, &label(s, "Move"));
        }

        // Rotation.
        heading(ui, "Rotation");
        let (_, rot) = edit::placement_parts(&s.world, first).unwrap_or((Vec3::ZERO, Mat3::IDENTITY));
        let h0 = edit::heading_of(rot);
        let mut h = h0;
        let mut done = false;
        ui.add_enabled_ui(editable, |ui| {
            egui::Grid::new("prop-rotation").num_columns(2).show(ui, |ui| {
                ui.label("Heading");
                let r =
                    ui.add(egui::DragValue::new(&mut h).speed(0.5).range(-180.0..=180.0).suffix("°").fixed_decimals(1));
                done |= finished(&r);
                ui.end_row();
                ui.label("Tilt");
                let t = edit::tilt_of(rot);
                ui.horizontal(|ui| {
                    ui.label(format!("{:.1}°, {:.1}°", t.x, t.y));
                    if ui.button("Upright").on_hover_text("Stand it straight, heading kept").clicked() {
                        upright_selection(s);
                    }
                });
                ui.end_row();
                ui.label("Ground");
                match edit::ground_gap(&s.world, first) {
                    Some(g) if g.abs() < 0.05 => tag(ui, "on the ground", colour(THEME_A.ok)),
                    Some(g) if g > 0.0 => tag(ui, &format!("{g:.2} m above the ground"), colour(THEME_A.warn)),
                    Some(g) => tag(ui, &format!("{:.2} m in the ground", -g), colour(THEME_A.warn)),
                    None => tag(ui, "no floor under it", colour(THEME_A.warn)),
                }
                ui.end_row();
            });
        });
        if h != h0 {
            let turn = h - h0;
            apply_each(s, |w, i| edit::placement_parts(w, i).map(|(q, r)| (q, edit::turned(r, turn))));
        }
        if done {
            commit(s, &label(s, "Rotate"));
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let resettable = matches!(state, PropState::Moved | PropState::Deleted);
            if ui.add_enabled(resettable, egui::Button::new("Reset to original")).clicked() {
                reset_selection(s);
            }
            if ui.add_enabled(editable, egui::Button::new("Duplicate")).on_hover_text("Ctrl+D").clicked() {
                duplicate_selection(s, None);
            }
            if ui.add_enabled(editable, egui::Button::new("Delete")).on_hover_text("Del").clicked() {
                delete_selection(s);
            }
        });
        ui.separator();
        heading(ui, "Collision");
        ui.label(RichText::new(if knock {
            "Knock-over props carry their own collision: it moves with the prop."
        } else {
            "Static scenery keeps its collision where the original has it: a moved or deleted piece still blocks there."
        })
        .small()
        .color(colour(THEME_A.dim)));
        if s.selection.len() > 1 {
            heading(ui, &format!("Selection ({})", s.selection.len()));
            ui.label(
                RichText::new("Ctrl+click adds. Moves and turns apply to all, around their centre.")
                    .small()
                    .color(colour(THEME_A.dim)),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::camera::Camera;
    use crate::tools::map::{MapMod, PropEdit};

    fn state() -> EditorState {
        let world = crate::tools::map::edit::tests::world();
        let camera = Camera::looking_at(Vec3::new(0.0, -20.0, 10.0), Vec3::new(10.0, 0.0, 0.0));
        EditorState::new("root".into(), "L4RA", "maps".into(), "my-map", MapMod::default(), world, camera)
    }

    #[test]
    fn selection_actions_are_one_undo_step_each() {
        let mut s = state();
        let n = s.world.placements.len();
        // The floating barrier dropped to the ground.
        s.selection = vec![2];
        drop_selection(&mut s);
        assert!(s.world.placements[2].transform.w_axis.z.abs() < 1e-4);
        assert_eq!(s.undo.undo_label(), Some("Drop to ground XC_BARRIER_03"));
        // Two cones copied 5 m along y: the copies are selected.
        s.selection = vec![0, 1];
        duplicate_selection(&mut s, Some(Vec3::new(15.0, 5.0, 0.0)));
        assert_eq!(s.world.placements.len(), n + 2);
        assert_eq!(s.selection.len(), 2);
        assert!(s.selection.iter().all(|&i| i >= n));
        let at: Vec<Vec3> = s.selection.iter().map(|&i| s.world.placements[i].transform.w_axis.truncate()).collect();
        assert!(at.contains(&Vec3::new(10.0, 5.0, 0.0)) && at.contains(&Vec3::new(20.0, 5.0, 0.0)), "{at:?}");
        assert!(s.adds_changed);
        // Deleting a copy removes its add; deleting an original hides it.
        s.selection = vec![s.selection[0], 0];
        delete_selection(&mut s);
        assert_eq!(s.world.placements.len(), n + 1);
        assert!(s.map.props.iter().any(|e| matches!(e, PropEdit::Delete { .. })));
        assert!(s.selection.is_empty());
        // Undo takes back the delete only.
        assert!(crate::editor::undo::undo(&mut s));
        assert_eq!(s.world.placements.len(), n + 2);
        assert!(s.world.placements[0].flags & crate::world::HIDDEN_FLAG == 0);
        // Reset the dropped barrier: back 2 m up, its edit gone.
        s.selection = vec![2];
        reset_selection(&mut s);
        assert_eq!(s.world.placements[2].transform.w_axis.z, 2.0);
        assert!(!s.map.props.iter().any(|e| matches!(e, PropEdit::Move { .. })));
    }

    #[test]
    fn a_new_prop_stands_on_the_ground_facing_the_heading() {
        let mut s = state();
        let i = place_new(&mut s, "XC_CONE_01", Vec3::new(5.0, 5.0, 0.3), 90.0).unwrap();
        assert_eq!(s.selection, vec![i]);
        let (p, r) = edit::placement_parts(&s.world, i).unwrap();
        assert!(p.z.abs() < 1e-4 && (edit::heading_of(r) - 90.0).abs() < 1e-3, "{p}");
        assert_eq!(s.undo.undo_label(), Some("Add XC_CONE_01"));
        assert!(place_new(&mut s, "NO_SUCH", Vec3::ZERO, 0.0).is_none());
    }

    #[test]
    fn angles_wrap_and_turn_about_a_pivot() {
        assert_eq!(wrap(190.0), -170.0);
        assert_eq!(wrap(-190.0), 170.0);
        assert_eq!(wrap(45.0), 45.0);
        let p = turn_about(Vec3::new(2.0, 1.0, 5.0), Vec3::new(1.0, 1.0, 0.0), 90.0);
        assert!(p.abs_diff_eq(Vec3::new(1.0, 2.0, 5.0), 1e-5), "{p}");
        // A position and its rotation turn the same way.
        let r = edit::turned(Mat3::IDENTITY, 90.0);
        assert!(r.row(0).abs_diff_eq(turn_about(Vec3::X, Vec3::ZERO, 90.0), 1e-5));
    }

    #[test]
    fn a_point_near_a_segment() {
        let (a, b) = (Pos2::new(0.0, 0.0), Pos2::new(10.0, 0.0));
        assert_eq!(to_segment(Pos2::new(5.0, 3.0), a, b), 3.0);
        assert_eq!(to_segment(Pos2::new(-4.0, 3.0), a, b), 5.0);
    }
}
