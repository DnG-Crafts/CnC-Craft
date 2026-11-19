//! Live effects: changes a shape carries that can be adjusted, reordered or
//! removed at any time. The shape keeps its original outline; the effects are worked out from it
//! in order, and "Apply for good" builds them into the shape.

use crate::app::App;
use crate::design;
use crate::geom;
use crate::model::*;
use crate::modify::{self, Warp};
use crate::panels::{len_edit, section};
use egui::{Color32, Pos2, RichText, Sense, Stroke, Ui};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Effect {
    Offset { distance: f64 },
    RoundCorners { radius: f64, chamfer: bool },
    Warp { kind: Warp, amount: f64 },
    Simplify { tolerance: f64 },
    Smooth { passes: u32 },
    RadialRepeat { copies: u32, radius: f64, weld: bool },
    /// a (cols+1) × (rows+1) grid of handles over the shape's bounds, row by row from the top;
    /// each move is a fraction of the bounds' width and height (y down, like the screen)
    Envelope { cols: u32, rows: u32, moves: Vec<(f64, f64)> },
}

impl Effect {
    pub fn choices() -> Vec<Effect> {
        vec![
            Effect::Offset { distance: 1.0 },
            Effect::RoundCorners { radius: 2.0, chamfer: false },
            Effect::Warp { kind: Warp::Arch, amount: 0.2 },
            Effect::Simplify { tolerance: 0.1 },
            Effect::Smooth { passes: 2 },
            Effect::RadialRepeat { copies: 8, radius: 20.0, weld: false },
            Effect::envelope(2, 2),
        ]
    }
    pub fn envelope(cols: u32, rows: u32) -> Effect {
        Effect::Envelope { cols, rows, moves: vec![(0.0, 0.0); ((cols + 1) * (rows + 1)) as usize] }
    }
    pub fn name(&self) -> &'static str {
        match self {
            Effect::Offset { .. } => "Offset",
            Effect::RoundCorners { .. } => "Round corners",
            Effect::Warp { .. } => "Warp",
            Effect::Simplify { .. } => "Simplify",
            Effect::Smooth { .. } => "Smooth",
            Effect::RadialRepeat { .. } => "Radial repeat",
            Effect::Envelope { .. } => "Envelope",
        }
    }

    /// The effect worked out on `paths`. Anything that would leave nothing passes the outline
    /// through unchanged, so a setting never makes the shape vanish.
    pub fn apply(&self, paths: Vec<Path>) -> Vec<Path> {
        let out = match self {
            Effect::Offset { distance } => {
                if distance.abs() < 1e-9 {
                    return paths;
                }
                let r = modify::region(&paths);
                if r.0.is_empty() {
                    return paths;
                }
                geom::region_to_paths(&geom::offset_region(&r, *distance))
            }
            Effect::RoundCorners { radius, chamfer } => paths.iter().map(|p| design::round_corners(p, *radius, *chamfer)).collect(),
            Effect::Warp { kind, amount } => match bbox_of(paths.iter().flat_map(|p| p.pts.iter())) {
                Some(b) => modify::warp(&paths, b, *kind, *amount),
                None => paths.clone(),
            },
            Effect::Simplify { tolerance } => paths
                .iter()
                .map(|p| {
                    let mut pts = p.pts.clone();
                    if p.closed && !pts.is_empty() {
                        pts.push(pts[0]);
                    }
                    let mut s = geom::simplify(&pts, tolerance.max(0.001));
                    if p.closed {
                        s.pop();
                    }
                    if s.len() >= if p.closed { 3 } else { 2 } {
                        Path { pts: s, closed: p.closed }
                    } else {
                        p.clone()
                    }
                })
                .collect(),
            Effect::Smooth { passes } => modify::smooth(&paths, *passes),
            Effect::RadialRepeat { copies, radius, weld } => modify::radial_repeat(&paths, *copies, *radius, *weld),
            Effect::Envelope { cols, rows, moves } => envelope(&paths, *cols, *rows, moves),
        };
        if out.is_empty() {
            paths
        } else {
            out
        }
    }
}

pub fn apply_all(base: &[Path], effects: &[Effect]) -> Vec<Path> {
    effects.iter().fold(base.to_vec(), |acc, e| e.apply(acc))
}

fn bernstein(n: u32, i: u32, t: f64) -> f64 {
    let binom = (0..i).fold(1.0, |acc, k| acc * (n - k) as f64 / (k + 1) as f64);
    binom * t.powi(i as i32) * (1.0 - t).powi((n - i) as i32)
}

/// Bends paths by the envelope's handles (a smooth Bézier patch over their bounds).
fn envelope(paths: &[Path], cols: u32, rows: u32, moves: &[(f64, f64)]) -> Vec<Path> {
    let (cols, rows) = (cols.max(1), rows.max(1));
    if moves.len() != ((cols + 1) * (rows + 1)) as usize || moves.iter().all(|m| *m == (0.0, 0.0)) {
        return paths.to_vec();
    }
    let Some((mn, mx)) = bbox_of(paths.iter().flat_map(|p| p.pts.iter())) else { return paths.to_vec() };
    let (w, h) = ((mx[0] - mn[0]).max(1e-9), (mx[1] - mn[1]).max(1e-9));
    let step = w.max(h) / 200.0;
    paths
        .iter()
        .map(|p| Path {
            pts: modify::densify(&p.pts, p.closed, step)
                .into_iter()
                .map(|q| {
                    let u = ((q[0] - mn[0]) / w).clamp(0.0, 1.0);
                    let v = ((mx[1] - q[1]) / h).clamp(0.0, 1.0); // from the top
                    let (mut dx, mut dy) = (0.0, 0.0);
                    for j in 0..=rows {
                        let bv = bernstein(rows, j, v);
                        for i in 0..=cols {
                            let (mxv, myv) = moves[(j * (cols + 1) + i) as usize];
                            let b = bv * bernstein(cols, i, u);
                            dx += mxv * w * b;
                            dy += myv * h * b;
                        }
                    }
                    [q[0] + dx, q[1] - dy]
                })
                .collect(),
            closed: p.closed,
        })
        .collect()
}

impl App {
    /// The Live effects section of the Shape tab, for one selected shape.
    pub fn effects_ui(&mut self, ui: &mut Ui) {
        section(ui, "Live effects");
        if self.selection.len() != 1 {
            ui.label(RichText::new("Select one shape to give it effects you can change later: offset, round corners, warp, simplify, smooth, radial repeat and envelope.").weak());
            return;
        }
        let id = self.selection[0];
        let Some(shape) = self.doc.shape(id).cloned() else { return };
        // another tool changed the outline: the effects are now part of it
        if let Some(fx) = &shape.fx {
            if fx.cached != shape.paths {
                if let Some(s) = self.doc.shapes.iter_mut().find(|s| s.id == id) {
                    s.fx = None;
                }
                self.status = "The shape was changed by another tool, so its effects were applied for good".into();
                return;
            }
        }
        let units = self.settings.units;
        let orig_fx = shape.fx.clone();
        let mut fx = orig_fx.clone().unwrap_or(LiveFx { base: shape.paths.clone(), effects: vec![], cached: shape.paths.clone() });
        let mut apply_good = false;
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("addfx").selected_text("Add effect…").show_ui(ui, |ui| {
                for e in Effect::choices() {
                    if ui.selectable_label(false, e.name()).clicked() {
                        fx.effects.push(e);
                    }
                }
            });
            if ui.add_enabled(!fx.effects.is_empty(), egui::Button::new("Apply for good")).on_hover_text("Build the effects into the shape").clicked() {
                apply_good = true;
            }
        });
        if fx.effects.is_empty() {
            ui.label(RichText::new("No effects yet. They apply in order from the top; the shape itself isn't changed, so nothing is lost by trying.").small().weak());
        }
        let mut action: Option<(usize, i32)> = None; // (index, -1 up / +1 down / 0 remove)
        let n = fx.effects.len();
        let base_bb = bbox_of(fx.base.iter().flat_map(|p| p.pts.iter()));
        for i in 0..n {
            let pre = apply_all(&fx.base, &fx.effects[..i]);
            let e = &mut fx.effects[i];
            egui::Frame::default()
                .stroke(Stroke::new(1.0f32, crate::design_ui::tone(ui.ctx(), Color32::from_rgb(62, 68, 78))))
                .corner_radius(4.0)
                .inner_margin(egui::Margin::same(6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{}. {}", i + 1, e.name())).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("Remove").clicked() {
                                action = Some((i, 0));
                            }
                            if ui.add_enabled(i + 1 < n, egui::Button::new("Down").small()).on_hover_text("Apply later").clicked() {
                                action = Some((i, 1));
                            }
                            if ui.add_enabled(i > 0, egui::Button::new("Up").small()).on_hover_text("Apply earlier").clicked() {
                                action = Some((i, -1));
                            }
                        });
                    });
                    match e {
                        Effect::Offset { distance } => {
                            ui.horizontal(|ui| {
                                ui.label("Distance");
                                len_edit(ui, distance, units, 0.1);
                            });
                        }
                        Effect::RoundCorners { radius, chamfer } => {
                            ui.horizontal(|ui| {
                                ui.label("Radius");
                                len_edit(ui, radius, units, 0.1);
                                *radius = radius.max(0.0);
                                ui.checkbox(chamfer, "Chamfer");
                            });
                        }
                        Effect::Warp { kind, amount } => {
                            ui.horizontal(|ui| {
                                egui::ComboBox::from_id_salt(("fxwarp", i)).selected_text(kind.label()).show_ui(ui, |ui| {
                                    for w in Warp::ALL {
                                        ui.selectable_value(kind, w, w.label());
                                    }
                                });
                                ui.add(egui::Slider::new(amount, -1.0..=1.0));
                            });
                        }
                        Effect::Simplify { tolerance } => {
                            ui.horizontal(|ui| {
                                ui.label("Tolerance");
                                len_edit(ui, tolerance, units, 0.01);
                                *tolerance = tolerance.max(0.001);
                            });
                        }
                        Effect::Smooth { passes } => {
                            ui.horizontal(|ui| {
                                ui.label("Strength");
                                ui.add(egui::DragValue::new(passes).range(1..=6));
                            });
                        }
                        Effect::RadialRepeat { copies, radius, weld } => {
                            ui.horizontal(|ui| {
                                ui.label("Copies");
                                ui.add(egui::DragValue::new(copies).range(2..=360));
                                ui.label("Radius");
                                len_edit(ui, radius, units, 0.2);
                                ui.checkbox(weld, "Weld");
                            });
                        }
                        Effect::Envelope { cols, rows, moves } => {
                            let (mut c, mut r) = (*cols, *rows);
                            ui.horizontal(|ui| {
                                ui.label("Grid");
                                ui.add(egui::DragValue::new(&mut c).range(1..=4));
                                ui.label("×");
                                ui.add(egui::DragValue::new(&mut r).range(1..=4));
                                if ui.button("Reset").clicked() {
                                    moves.iter_mut().for_each(|m| *m = (0.0, 0.0));
                                }
                            });
                            if (c, r) != (*cols, *rows) {
                                *cols = c;
                                *rows = r;
                                *moves = vec![(0.0, 0.0); ((c + 1) * (r + 1)) as usize];
                            }
                            envelope_editor(ui, &pre, *cols, *rows, moves, i);
                        }
                    }
                });
        }
        let _ = base_bb;
        if let Some((i, dir)) = action {
            match dir {
                0 => {
                    fx.effects.remove(i);
                }
                -1 => fx.effects.swap(i, i - 1),
                _ => fx.effects.swap(i, i + 1),
            }
        }
        if apply_good {
            if let Some(s) = self.doc.shapes.iter_mut().find(|s| s.id == id) {
                let before_fx = s.fx.take();
                let _ = before_fx;
            }
            self.status = "Effects applied for good".into();
            self.job_dirty = true;
            return;
        }
        let new_fx = if fx.effects.is_empty() { None } else { Some(fx.clone()) };
        if new_fx != orig_fx && !(orig_fx.is_none() && fx.effects.is_empty()) {
            let before = self.doc.clone();
            if let Some(s) = self.doc.shapes.iter_mut().find(|s| s.id == id) {
                if let Some(mut f) = new_fx {
                    let result = apply_all(&f.base, &f.effects);
                    f.cached = result.clone();
                    s.paths = result;
                    s.fx = Some(f);
                } else {
                    s.paths = fx.base.clone();
                    s.fx = None;
                }
            }
            self.checkpoint_coalesced(&format!("fx{id}"), before);
        }
    }
}

/// A small preview of the outline with the envelope's handle grid; drag the handles to bend it.
fn envelope_editor(ui: &mut Ui, paths: &[Path], cols: u32, rows: u32, moves: &mut [(f64, f64)], idx: usize) {
    let Some((mn, mx)) = bbox_of(paths.iter().flat_map(|p| p.pts.iter())) else { return };
    let (w, h) = ((mx[0] - mn[0]).max(1e-9), (mx[1] - mn[1]).max(1e-9));
    let avail = ui.available_width().min(300.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(avail, 190.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, Color32::from_rgb(28, 31, 36));
    let inner = rect.shrink(36.0);
    let k = (inner.width() as f64 / w).min(inner.height() as f64 / h);
    let box_w = (w * k) as f32;
    let box_h = (h * k) as f32;
    let area = egui::Rect::from_center_size(inner.center(), egui::vec2(box_w, box_h));
    let to = |p: P| Pos2::new(area.left() + ((p[0] - mn[0]) * k) as f32, area.top() + ((mx[1] - p[1]) * k) as f32);
    // the bent outline
    let bent = envelope(paths, cols, rows, moves);
    for p in paths {
        let pts: Vec<Pos2> = p.pts.iter().map(|q| to(*q)).collect();
        painter.add(if p.closed { egui::Shape::closed_line(pts, Stroke::new(1.0f32, Color32::from_gray(90))) } else { egui::Shape::line(pts, Stroke::new(1.0f32, Color32::from_gray(90))) });
    }
    for p in &bent {
        let pts: Vec<Pos2> = p.pts.iter().map(|q| to(*q)).collect();
        painter.add(if p.closed { egui::Shape::closed_line(pts, Stroke::new(1.5f32, Color32::from_rgb(70, 160, 255))) } else { egui::Shape::line(pts, Stroke::new(1.5f32, Color32::from_rgb(70, 160, 255))) });
    }
    // handles
    let handle_pos = |i: u32, j: u32, m: (f64, f64)| {
        Pos2::new(area.left() + box_w * (i as f32 / cols as f32 + m.0 as f32), area.top() + box_h * (j as f32 / rows as f32 + m.1 as f32))
    };
    for j in 0..=rows {
        for i in 0..cols {
            let a = handle_pos(i, j, moves[(j * (cols + 1) + i) as usize]);
            let b = handle_pos(i + 1, j, moves[(j * (cols + 1) + i + 1) as usize]);
            painter.line_segment([a, b], Stroke::new(1.0f32, Color32::from_rgb(255, 200, 80).gamma_multiply(0.6)));
        }
    }
    for i in 0..=cols {
        for j in 0..rows {
            let a = handle_pos(i, j, moves[(j * (cols + 1) + i) as usize]);
            let b = handle_pos(i, j + 1, moves[((j + 1) * (cols + 1) + i) as usize]);
            painter.line_segment([a, b], Stroke::new(1.0f32, Color32::from_rgb(255, 200, 80).gamma_multiply(0.6)));
        }
    }
    for j in 0..=rows {
        for i in 0..=cols {
            let n = (j * (cols + 1) + i) as usize;
            let c = handle_pos(i, j, moves[n]);
            let r = ui.interact(egui::Rect::from_center_size(c, egui::vec2(14.0, 14.0)), ui.id().with(("envh", idx, n)), Sense::drag());
            if r.dragged() {
                let d = r.drag_delta();
                moves[n].0 += (d.x / box_w.max(1.0)) as f64;
                moves[n].1 += (d.y / box_h.max(1.0)) as f64;
            }
            let hot = r.hovered() || r.dragged();
            painter.circle(c, if hot { 6.0 } else { 4.5 }, if hot { Color32::from_rgb(255, 200, 80) } else { Color32::WHITE }, Stroke::new(1.2f32, Color32::from_rgb(200, 140, 30)));
        }
    }
    ui.label(RichText::new("Drag the handles to bend the shape.").small().weak());
}
