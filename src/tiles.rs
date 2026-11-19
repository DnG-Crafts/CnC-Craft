//! Split into Tiles (designs bigger than the stock or bed, with jointed seams) and Rubber Stamp.

use crate::app::App;
use crate::geom::{self, dist, BoolOp};
use crate::model::*;
use crate::modify;
use crate::panels::{len_edit, section};
use egui::{RichText, Ui};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Joint {
    Straight,
    Dovetail,
    Puzzle,
}

impl Joint {
    pub const ALL: [Joint; 3] = [Joint::Straight, Joint::Dovetail, Joint::Puzzle];
    pub fn label(self) -> &'static str {
        match self {
            Joint::Straight => "Straight",
            Joint::Dovetail => "Dovetail",
            Joint::Puzzle => "Puzzle knob",
        }
    }
}

/// One seam segment from `a` to `b` with joints along it.
fn seam(a: P, b: P, joint: Joint, size: f64, spacing: f64, seed: &mut u64) -> Vec<P> {
    let l = dist(a, b);
    if joint == Joint::Straight || l < 1e-9 {
        return vec![a, b];
    }
    let n = ((l / spacing.max(1.0)).round() as usize).max(1);
    let d = [(b[0] - a[0]) / l, (b[1] - a[1]) / l];
    let nrm = [-d[1], d[0]];
    let at = |s: f64, off: f64| [a[0] + d[0] * s + nrm[0] * off, a[1] + d[1] * s + nrm[1] * off];
    let seg = l / n as f64;
    let mut pts = vec![a];
    for k in 0..n {
        let flip = k % 2 == 1;
        match joint {
            Joint::Dovetail => {
                let c = (k as f64 + 0.5) * seg;
                let s = size.min(seg * 0.4);
                let dir = if flip { -1.0 } else { 1.0 };
                let (hb, ht) = (s * 0.35, s * 0.55);
                pts.push(at(c - hb, 0.0));
                pts.push(at(c - ht, s * dir));
                pts.push(at(c + ht, s * dir));
                pts.push(at(c + hb, 0.0));
            }
            Joint::Puzzle => {
                let p0 = at(k as f64 * seg, 0.0);
                let p1 = at((k + 1) as f64 * seg, 0.0);
                let knob = crate::design::puzzle_edge(p0, p1, flip, seed);
                pts.extend(knob.into_iter().skip(1));
                continue;
            }
            Joint::Straight => {}
        }
    }
    if dist(*pts.last().unwrap(), b) > 1e-9 {
        pts.push(b);
    }
    pts
}

/// The outline of every tile, row by row from the bottom: (column, row, outline).
pub fn tile_outlines(mn: P, mx: P, cols: u32, rows: u32, joint: Joint, size: f64, spacing: f64) -> Vec<(u32, u32, Vec<P>)> {
    let (cols, rows) = (cols.max(1) as usize, rows.max(1) as usize);
    let xs: Vec<f64> = (0..=cols).map(|i| mn[0] + (mx[0] - mn[0]) * i as f64 / cols as f64).collect();
    let ys: Vec<f64> = (0..=rows).map(|j| mn[1] + (mx[1] - mn[1]) * j as f64 / rows as f64).collect();
    let mut seed = 99u64;
    // vertical seams v[i][j]: x = xs[i], from ys[j] to ys[j+1]; horizontal h[j][i]: y = ys[j], xs[i]..xs[i+1]
    let mut v = vec![vec![vec![]; rows]; cols + 1];
    for (i, col) in v.iter_mut().enumerate() {
        for (j, cell) in col.iter_mut().enumerate() {
            let interior = i > 0 && i < cols;
            *cell = seam([xs[i], ys[j]], [xs[i], ys[j + 1]], if interior { joint } else { Joint::Straight }, size, spacing, &mut seed);
        }
    }
    let mut h = vec![vec![vec![]; cols]; rows + 1];
    for (j, row) in h.iter_mut().enumerate() {
        for (i, cell) in row.iter_mut().enumerate() {
            let interior = j > 0 && j < rows;
            *cell = seam([xs[i], ys[j]], [xs[i + 1], ys[j]], if interior { joint } else { Joint::Straight }, size, spacing, &mut seed);
        }
    }
    let mut out = vec![];
    for j in 0..rows {
        for i in 0..cols {
            let mut ring: Vec<P> = h[j][i].clone();
            ring.extend(v[i + 1][j].iter().skip(1));
            ring.extend(h[j + 1][i].iter().rev().skip(1));
            let left: Vec<P> = v[i][j].iter().rev().skip(1).copied().collect();
            ring.extend(left.iter().take(left.len().saturating_sub(1)));
            out.push((i as u32, j as u32, ring));
        }
    }
    out
}

/// A rubber stamp from shapes: the background to cut away around the (mirrored) design, an
/// optional shoulder band, and the cut line round the stamp.
pub fn rubber_stamp(parts: &[Vec<Path>], margin: f64, shoulder: f64) -> Option<(Vec<Path>, Option<Vec<Path>>, Vec<Path>)> {
    let region = modify::merged(parts);
    let paths = geom::region_to_paths(&region);
    let (mn, mx) = bbox_of(paths.iter().flat_map(|p| p.pts.iter()))?;
    let cx = (mn[0] + mx[0]) / 2.0;
    let mirrored: Vec<Path> = paths.iter().map(|p| Path { pts: p.pts.iter().rev().map(|q| [2.0 * cx - q[0], q[1]]).collect(), closed: true }).collect();
    let mreg = modify::region(&mirrored);
    let outline = vec![geom::rect_path([mn[0] - margin, mn[1] - margin], [mx[0] + margin, mx[1] + margin], margin.min(3.0))];
    let oreg = modify::region(&outline);
    if shoulder > 0.0 {
        let grown = geom::offset_region(&mreg, shoulder);
        let background = geom::region_to_paths(&geom::boolean(&oreg, &grown, BoolOp::Difference));
        let band = geom::region_to_paths(&geom::boolean(&geom::boolean(&grown, &mreg, BoolOp::Difference), &oreg, BoolOp::Intersection));
        Some((background, (!band.is_empty()).then_some(band), outline))
    } else {
        Some((geom::region_to_paths(&geom::boolean(&oreg, &mreg, BoolOp::Difference)), None, outline))
    }
}

impl App {
    /// Cuts the selection into a grid of jointed tiles, spread apart and grouped per tile.
    pub fn split_into_tiles(&mut self) {
        let Some((mn, mx)) = self.selection_bbox() else { return };
        let d = &self.design;
        let (cols, rows, joint, size, spacing, spread) = (d.tile_cols, d.tile_rows, d.tile_joint, d.tile_size, d.tile_spacing, d.tile_spread);
        let pad = 0.5;
        let tiles = tile_outlines([mn[0] - pad, mn[1] - pad], [mx[0] + pad, mx[1] + pad], cols, rows, joint, size, spacing);
        let sources: Vec<Shape> = self.selected_shapes().into_iter().cloned().collect();
        self.checkpoint();
        let mut new_sel = vec![];
        for (i, j, ring) in &tiles {
            let tile = modify::region(&[Path { pts: ring.clone(), closed: true }]);
            let shift = [*i as f64 * spread, *j as f64 * spread];
            let g = self.doc.new_id();
            let mut ids = vec![];
            for s in &sources {
                let closed: Vec<Path> = s.paths.iter().filter(|p| p.closed && p.pts.len() > 2).cloned().collect();
                let open: Vec<Vec<P>> = s.paths.iter().filter(|p| !p.closed || p.pts.len() == 1).map(|p| p.pts.clone()).collect();
                let mut piece = if closed.is_empty() { vec![] } else { geom::region_to_paths(&geom::boolean(&modify::region(&closed), &tile, BoolOp::Intersection)) };
                piece.extend(modify::clip_lines(open.into_iter().filter(|l| l.len() >= 2).collect(), &tile));
                // drill points stay with the tile they sit in
                for p in s.paths.iter().filter(|p| p.pts.len() == 1) {
                    if geom::point_in_poly(p.pts[0], ring) {
                        piece.push(p.clone());
                    }
                }
                if piece.is_empty() {
                    continue;
                }
                for p in &mut piece {
                    for q in &mut p.pts {
                        *q = [q[0] + shift[0], q[1] + shift[1]];
                    }
                }
                let id = self.doc.new_id();
                let kind = if piece.iter().all(|p| p.pts.len() == 1) { ShapeKind::Point } else { ShapeKind::Path };
                self.doc.shapes.push(Shape { id, kind, paths: piece, group: Some(g), text: None, fx: None });
                for tp in &mut self.doc.toolpaths {
                    if tp.shape_ids.contains(&s.id) {
                        tp.shape_ids.push(id);
                    }
                }
                ids.push(id);
            }
            if ids.len() == 1 {
                if let Some(s) = self.doc.shapes.iter_mut().find(|s| s.id == ids[0]) {
                    s.group = None;
                }
            }
            new_sel.extend(ids);
        }
        let old: Vec<u64> = sources.iter().map(|s| s.id).collect();
        self.remove_into(&old, None);
        self.selection = new_sel;
        self.job_dirty = true;
        self.status = format!("Split into {} tiles", tiles.len());
    }

    pub fn tiles_ui(&mut self, ui: &mut Ui) {
        let u = self.settings.units;
        section(ui, "Split into tiles");
        ui.label(RichText::new("For designs bigger than your stock: cuts the selection into a grid of tiles whose seams lock together.").small().weak());
        if let Some((mn, mx)) = self.selection_bbox() {
            let (w, h) = (mx[0] - mn[0], mx[1] - mn[1]);
            let (sw, sh) = (self.doc.stock.width, self.doc.stock.height);
            if ui.small_button("Suggest tiles to fit the stock").clicked() {
                self.design.tile_cols = ((w / sw).ceil() as u32).max(1);
                self.design.tile_rows = ((h / sh).ceil() as u32).max(1);
            }
            let tw = w / self.design.tile_cols.max(1) as f64;
            let th = h / self.design.tile_rows.max(1) as f64;
            if tw > sw || th > sh {
                ui.label(RichText::new(format!("⚠ Each tile is {} × {}, bigger than the stock", self.len_text(tw), self.len_text(th))).color(egui::Color32::from_rgb(250, 200, 70)));
            }
        }
        let d = &mut self.design;
        egui::Grid::new("tilegrid").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
            ui.label("Tiles across × down");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut d.tile_cols).range(1..=20));
                ui.label("×");
                ui.add(egui::DragValue::new(&mut d.tile_rows).range(1..=20));
            });
            ui.end_row();
            ui.label("Joints");
            egui::ComboBox::from_id_salt("tilejoint").selected_text(d.tile_joint.label()).show_ui(ui, |ui| {
                for j in Joint::ALL {
                    ui.selectable_value(&mut d.tile_joint, j, j.label());
                }
            });
            ui.end_row();
            ui.label("Joint size");
            len_edit(ui, &mut d.tile_size, u, 0.2);
            ui.end_row();
            ui.label("Joints about every");
            len_edit(ui, &mut d.tile_spacing, u, 1.0);
            ui.end_row();
            ui.label("Spread tiles apart by");
            len_edit(ui, &mut d.tile_spread, u, 0.5);
            ui.end_row();
        });
        if ui.add_enabled(!self.selection.is_empty(), egui::Button::new("Split into tiles")).clicked() {
            self.split_into_tiles();
        }
    }

    pub fn stamp_ui(&mut self, ui: &mut Ui) {
        let u = self.settings.units;
        let d = &mut self.design;
        egui::CollapsingHeader::new(RichText::new("Rubber stamp").strong()).show(ui, |ui| {
            ui.label(RichText::new("Makes a stamp from the selection: the design mirrored and left raised, the background cut away, a sloped shoulder round it and a cut line, placed beside it.").small().weak());
            egui::Grid::new("stampgrid").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                ui.label("Border");
                len_edit(ui, &mut d.stamp_margin, u, 0.2);
                ui.end_row();
                ui.label("Shoulder");
                len_edit(ui, &mut d.stamp_shoulder, u, 0.1);
                ui.end_row();
                ui.label("Relief depth");
                len_edit(ui, &mut d.stamp_depth, u, 0.1);
                ui.end_row();
                ui.label("Toolpaths");
                ui.checkbox(&mut d.stamp_toolpaths, "Add pocket and cut-out toolpaths");
                ui.end_row();
            });
        });
        let go = ui.add_enabled(!self.selection.is_empty(), egui::Button::new("Make rubber stamp")).clicked();
        if go {
            self.make_stamp();
        }
    }

    pub fn make_stamp(&mut self) {
        let parts: Vec<Vec<Path>> = self.selected_shapes().iter().map(|s| s.paths.clone()).collect();
        let Some((mn, mx)) = self.selection_bbox() else { return };
        let (margin, shoulder, depth, with_tp) = (self.design.stamp_margin, self.design.stamp_shoulder, self.design.stamp_depth, self.design.stamp_toolpaths);
        let Some((background, band, outline)) = rubber_stamp(&parts, margin, shoulder) else {
            self.status = "Rubber stamp needs closed shapes or text".into();
            return;
        };
        // place it beside the design
        let dx = (mx[0] - mn[0]) + 2.0 * margin + 10.0;
        let shift = |v: Vec<Path>| -> Vec<Path> { v.into_iter().map(|mut p| { p.pts.iter_mut().for_each(|q| q[0] += dx); p }).collect() };
        self.checkpoint();
        let bg = self.add_parts(vec![shift(background)], "Stamp background");
        let sh = band.map(|b| self.add_parts(vec![shift(b)], "Stamp shoulder")).unwrap_or_default();
        let ol = self.add_parts(vec![shift(outline)], "Stamp outline");
        let g = self.doc.new_id();
        for id in bg.iter().chain(&sh).chain(&ol) {
            if let Some(s) = self.doc.shapes.iter_mut().find(|s| s.id == *id) {
                s.group = Some(g);
            }
        }
        if with_tp {
            // the smallest flat bit clears the background; a V-bit carves the sloped shoulder
            let flat = self.settings.tools.iter().filter(|t| t.kind == ToolKind::FlatEnd).min_by(|a, b| a.diameter.total_cmp(&b.diameter)).or(self.settings.tools.first()).cloned();
            let vbit = self.settings.tools.iter().find(|t| t.kind == ToolKind::VBit).cloned().or(flat.clone());
            if let (Some(flat), Some(vbit)) = (flat, vbit) {
                let add = |app: &mut App, op: OpKind, ids: &[u64], cut: f64, name: &str| {
                    if ids.is_empty() {
                        return;
                    }
                    let id = app.doc.new_id();
                    let tool = if op == OpKind::VCarve { &vbit } else { &flat };
                    let mut tp = Toolpath::new(id, op, tool, ids.to_vec(), app.doc.toolpaths.len());
                    tp.cut_depth = cut;
                    tp.vcarve_max_depth = cut;
                    tp.name = name.into();
                    if op == OpKind::ProfileOutside {
                        tp.tabs = true;
                    }
                    app.doc.toolpaths.push(tp);
                };
                add(self, OpKind::Pocket, &bg, depth, "Stamp background");
                add(self, OpKind::VCarve, &sh, depth / 2.0, "Stamp shoulder");
                let thick = self.doc.stock.thickness;
                add(self, OpKind::ProfileOutside, &ol, thick, "Stamp cut-out");
            }
        }
        self.selection = bg.into_iter().chain(sh).chain(ol).collect();
        self.job_dirty = true;
        self.status = "Rubber stamp made beside the design".into();
    }
}
