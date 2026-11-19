//! Shape tools panel (align, distribute, combine, corners, paths, arrays, order)
//! and the position/size fields with a reference point.

use crate::app::App;
use crate::design;
use crate::geom::{self, BoolOp};
use crate::model::*;
use crate::panels::{len_edit, section};
use egui::{RichText, Ui};

impl App {
    pub fn shape_tab(&mut self, ui: &mut Ui, arrange: bool) {
        let n = self.selection.len();
        let u = self.settings.units;
        if let Some((mn, mx)) = self.selection_bbox() {
            ui.label(format!("{n} selected · {} × {}", self.len_text(mx[0] - mn[0]), self.len_text(mx[1] - mn[1])));
        } else {
            ui.label(RichText::new("Select shapes on the design to use these tools.").weak());
        }
        let b = |ui: &mut Ui, label: &str, tip: &str, enabled: bool| ui.add_enabled(enabled, egui::Button::new(label).min_size(egui::vec2(96.0, 22.0))).on_hover_text(tip).clicked();
        let has = n > 0;
        let many = n > 1;

        if arrange {
        section(ui, "Align");
        ui.label(RichText::new("With one shape selected, it lines up with the stock.").small().weak());
        egui::Grid::new("align").spacing([4.0, 4.0]).show(ui, |ui| {
            for (i, (k, l)) in [("left", "Left"), ("hcenter", "Center"), ("right", "Right"), ("top", "Top"), ("vcenter", "Middle"), ("bottom", "Bottom")].iter().enumerate() {
                if b(ui, l, "Line up the selection", has) {
                    self.align(k);
                }
                if i % 3 == 2 {
                    ui.end_row();
                }
            }
        });
        section(ui, "Spacing and size");
        egui::Grid::new("dist").spacing([4.0, 4.0]).show(ui, |ui| {
            if b(ui, "Space across", "Equal gaps left to right", n > 2) {
                self.distribute(true);
            }
            if b(ui, "Space down", "Equal gaps top to bottom", n > 2) {
                self.distribute(false);
            }
            ui.end_row();
            if b(ui, "Same width", "Match the first selected shape", many) {
                self.same_size(true, false);
            }
            if b(ui, "Same height", "Match the first selected shape", many) {
                self.same_size(false, true);
            }
            if b(ui, "Same size", "Match the first selected shape", many) {
                self.same_size(true, true);
            }
            ui.end_row();
        });
        ui.horizontal(|ui| {
            ui.label("Scale");
            ui.add(egui::DragValue::new(&mut self.design.scale_pct).speed(0.5).range(1.0..=10000.0).suffix(" %"));
            if ui.add_enabled(has, egui::Button::new("Apply")).clicked() {
                let k = self.design.scale_pct / 100.0;
                if let Some((mn, mx)) = self.selection_bbox() {
                    let c = [(mn[0] + mx[0]) / 2.0, (mn[1] + mx[1]) / 2.0];
                    self.modify_selected(|s| geom::scale_about(s, c, k, k));
                }
            }
        });

        }
        if !arrange {
        section(ui, "Combine");
        egui::Grid::new("combine").spacing([4.0, 4.0]).show(ui, |ui| {
            if b(ui, "Weld", "Merge overlapping shapes into one outline", many) {
                self.boolean(BoolOp::Union);
            }
            if b(ui, "Subtract", "Cut the other shapes out of the first one", many) {
                self.boolean(BoolOp::Difference);
            }
            if b(ui, "Intersect", "Keep only where the shapes overlap", many) {
                self.boolean(BoolOp::Intersection);
            }
            ui.end_row();
            if b(ui, "Exclude", "Keep the parts that don't overlap", many) {
                self.boolean(BoolOp::Xor);
            }
            if b(ui, "Group", "Select and move them together", many) {
                self.group();
            }
            if b(ui, "Ungroup", "", has) {
                self.ungroup();
            }
            ui.end_row();
        });
        ui.horizontal(|ui| {
            ui.label("Offset");
            len_edit(ui, &mut self.offset_dist, u, 0.1);
            if ui.add_enabled(has, egui::Button::new("Make outline")).on_hover_text("Positive grows, negative shrinks").clicked() {
                self.offset_selected();
            }
        });

        section(ui, "Corners");
        ui.horizontal(|ui| {
            ui.label("Radius");
            len_edit(ui, &mut self.design.corner_r, u, 0.1);
            self.design.corner_r = self.design.corner_r.max(0.0);
            if ui.add_enabled(has, egui::Button::new("Round")).clicked() {
                self.round_selected(false);
            }
            if ui.add_enabled(has, egui::Button::new("Chamfer")).clicked() {
                self.round_selected(true);
            }
        });

        section(ui, "Paths");
        egui::Grid::new("paths").spacing([4.0, 4.0]).show(ui, |ui| {
            if b(ui, "Join", "Connect open lines whose ends touch", has) {
                self.join_selected();
            }
            if b(ui, "Close", "Close open shapes", has) {
                self.close_selected();
            }
            if b(ui, "Reverse", "Reverse the cutting direction", has) {
                self.modify_selected(|s| s.paths.iter_mut().for_each(|p| p.pts.reverse()));
            }
            ui.end_row();
            if b(ui, "Break apart", "Split shapes into their separate outlines", has) {
                self.break_apart();
            }
            if b(ui, "Remove copies", "Delete shapes that sit exactly on top of another", true) {
                self.delete_duplicates();
            }
            if b(ui, "To drill points", "Replace shapes with a drill point at their centers", has) {
                self.to_drill_points();
            }
            ui.end_row();
        });
        ui.horizontal(|ui| {
            ui.label("Simplify");
            len_edit(ui, &mut self.design.simplify_tol, u, 0.01);
            self.design.simplify_tol = self.design.simplify_tol.max(0.001);
            if ui.add_enabled(has, egui::Button::new("Apply")).on_hover_text("Remove points that hardly change the shape").clicked() {
                self.simplify_selected();
            }
        });

        }
        if arrange {
        section(ui, "Arrays");
        if ui.add_enabled(has, egui::Button::new("Grid array…")).clicked() {
            self.array_open = true;
        }
        egui::Grid::new("circ").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
            ui.label("Copies around a circle");
            ui.add(egui::DragValue::new(&mut self.design.circ_count).range(2..=360));
            ui.end_row();
            ui.label("Total angle");
            ui.add(egui::DragValue::new(&mut self.design.circ_angle).speed(1.0).range(1.0..=360.0).suffix("°"));
            ui.end_row();
            ui.label("Center");
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.design.circ_use_sel, "Selection");
                if !self.design.circ_use_sel {
                    len_edit(ui, &mut self.design.circ_center[0], u, 0.5);
                    len_edit(ui, &mut self.design.circ_center[1], u, 0.5);
                }
            });
            ui.end_row();
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.design.circ_rotate, "Turn the copies");
            if ui.small_button("Stock center").clicked() {
                self.design.circ_center = [self.doc.stock.width / 2.0, self.doc.stock.height / 2.0];
                self.design.circ_use_sel = false;
            }
        });
        if ui.add_enabled(has, egui::Button::new("Make circular array")).clicked() {
            self.circular_array();
        }

        section(ui, "Order");
        ui.horizontal(|ui| {
            if ui.add_enabled(has, egui::Button::new("To front")).on_hover_text("Cut last within its toolpath").clicked() {
                self.reorder(true);
            }
            if ui.add_enabled(has, egui::Button::new("To back")).on_hover_text("Cut first within its toolpath").clicked() {
                self.reorder(false);
            }
            if ui.button("Select all").clicked() {
                self.select_all();
            }
            if ui.add_enabled(has, egui::Button::new("Invert")).on_hover_text("Select everything else").clicked() {
                let sel = self.selection.clone();
                self.selection = self.doc.shapes.iter().map(|s| s.id).filter(|id| !sel.contains(id)).collect();
            }
        });
        }
    }

    // ---------------------------------------------------------------- actions

    pub fn distribute(&mut self, horizontal: bool) {
        let ax = if horizontal { 0 } else { 1 };
        let mut items: Vec<(u64, P, P)> = self.selected_shapes().iter().filter_map(|s| s.bbox().map(|(a, b)| (s.id, a, b))).collect();
        if items.len() < 3 {
            return;
        }
        items.sort_by(|a, b| (a.1[ax] + a.2[ax]).partial_cmp(&(b.1[ax] + b.2[ax])).unwrap());
        let start = items[0].1[ax];
        let end = items.last().unwrap().2[ax];
        let total: f64 = items.iter().map(|i| i.2[ax] - i.1[ax]).sum();
        let gap = (end - start - total) / (items.len() - 1) as f64;
        self.checkpoint();
        let mut pos = start;
        for (id, a, b) in items {
            let mut d = [0.0, 0.0];
            d[ax] = pos - a[ax];
            if let Some(s) = self.doc.shapes.iter_mut().find(|s| s.id == id) {
                geom::translate(s, d);
            }
            pos += b[ax] - a[ax] + gap;
        }
    }

    pub fn same_size(&mut self, w: bool, h: bool) {
        let Some(first) = self.selection.first().and_then(|id| self.doc.shape(*id)).and_then(|s| s.bbox()) else { return };
        let (rw, rh) = (first.1[0] - first.0[0], first.1[1] - first.0[1]);
        let lock = self.lock_aspect;
        self.modify_selected(|s| {
            let Some((a, b)) = s.bbox() else { return };
            let (sw, sh) = ((b[0] - a[0]).max(1e-9), (b[1] - a[1]).max(1e-9));
            let mut kx = if w { rw / sw } else { 1.0 };
            let mut ky = if h { rh / sh } else { 1.0 };
            if lock && w != h {
                if w {
                    ky = kx;
                } else {
                    kx = ky;
                }
            }
            let c = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
            geom::scale_about(s, c, kx, ky);
        });
    }

    pub fn round_selected(&mut self, chamfer: bool) {
        let r = self.design.corner_r;
        if r <= 0.0 {
            self.status = "Set a corner radius first".into();
            return;
        }
        self.modify_selected(|s| {
            for p in &mut s.paths {
                *p = design::round_corners(p, r, chamfer);
            }
            if s.kind != ShapeKind::Point {
                s.kind = ShapeKind::Path;
            }
        });
        self.status = if chamfer { "Corners chamfered".into() } else { "Corners rounded".into() };
    }

    pub fn join_selected(&mut self) {
        let sel = self.selected_shapes();
        if sel.is_empty() {
            return;
        }
        let paths: Vec<Path> = sel.iter().flat_map(|s| s.paths.clone()).collect();
        let before = paths.len();
        let joined = design::join_paths(paths, 0.05);
        let after = joined.len();
        let ids = self.selection.clone();
        self.checkpoint();
        let first = ids[0];
        self.doc.shapes.retain(|s| s.id == first || !ids.contains(&s.id));
        if let Some(s) = self.doc.shapes.iter_mut().find(|s| s.id == first) {
            s.paths = joined;
            s.kind = ShapeKind::Path;
            s.group = None;
        }
        for tp in &mut self.doc.toolpaths {
            tp.shape_ids.retain(|id| *id == first || !ids.contains(id));
        }
        self.selection = vec![first];
        self.status = format!("Joined {before} lines into {after}");
    }

    pub fn close_selected(&mut self) {
        self.modify_selected(|s| {
            for p in &mut s.paths {
                if p.pts.len() >= 3 {
                    if geom::dist(p.pts[0], *p.pts.last().unwrap()) < 1e-6 {
                        p.pts.pop();
                    }
                    p.closed = true;
                }
            }
        });
    }

    pub fn simplify_selected(&mut self) {
        let tol = self.design.simplify_tol;
        let mut before = 0;
        let mut after = 0;
        for s in self.selected_shapes() {
            before += s.paths.iter().map(|p| p.pts.len()).sum::<usize>();
        }
        self.modify_selected(|s| {
            for p in &mut s.paths {
                if p.pts.len() > 3 {
                    let mut pts = p.pts.clone();
                    if p.closed {
                        pts.push(pts[0]);
                    }
                    let mut out = geom::simplify(&pts, tol);
                    if p.closed {
                        out.pop();
                    }
                    if out.len() >= if p.closed { 3 } else { 2 } {
                        p.pts = out;
                    }
                }
            }
        });
        for s in self.selected_shapes() {
            after += s.paths.iter().map(|p| p.pts.len()).sum::<usize>();
        }
        self.status = format!("Points: {before} → {after}");
    }

    pub fn break_apart(&mut self) {
        let ids = self.selection.clone();
        let targets: Vec<Shape> = self.selected_shapes().into_iter().filter(|s| s.paths.len() > 1).cloned().collect();
        if targets.is_empty() {
            self.status = "Nothing to break apart".into();
            return;
        }
        self.checkpoint();
        let mut new_sel: Vec<u64> = ids.iter().copied().filter(|id| !targets.iter().any(|t| t.id == *id)).collect();
        for t in targets {
            let pos = self.doc.shapes.iter().position(|s| s.id == t.id).unwrap_or(self.doc.shapes.len());
            self.doc.shapes.retain(|s| s.id != t.id);
            let mut k = pos;
            for p in t.paths {
                let id = self.doc.new_id();
                for tp in &mut self.doc.toolpaths {
                    if tp.shape_ids.contains(&t.id) {
                        tp.shape_ids.push(id);
                    }
                }
                self.doc.shapes.insert(k.min(self.doc.shapes.len()), Shape { id, kind: ShapeKind::Path, paths: vec![p], group: None, text: None, fx: None });
                k += 1;
                new_sel.push(id);
            }
            for tp in &mut self.doc.toolpaths {
                tp.shape_ids.retain(|id| *id != t.id);
            }
        }
        self.selection = new_sel;
    }

    pub fn delete_duplicates(&mut self) {
        let mut seen = std::collections::HashSet::new();
        let mut dup = vec![];
        for s in &self.doc.shapes {
            if !seen.insert(design::path_key(&s.paths)) {
                dup.push(s.id);
            }
        }
        if dup.is_empty() {
            self.status = "No copies found".into();
            return;
        }
        self.checkpoint();
        self.doc.shapes.retain(|s| !dup.contains(&s.id));
        for tp in &mut self.doc.toolpaths {
            tp.shape_ids.retain(|id| !dup.contains(id));
        }
        self.prune_selection();
        self.status = format!("Removed {} copies", dup.len());
    }

    pub fn to_drill_points(&mut self) {
        self.modify_selected(|s| {
            if let Some((a, b)) = s.bbox() {
                s.paths = vec![Path { pts: vec![[(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]], closed: false }];
                s.kind = ShapeKind::Point;
            }
        });
    }

    pub fn circular_array(&mut self) {
        let Some((mn, mx)) = self.selection_bbox() else { return };
        let center = if self.design.circ_use_sel { [(mn[0] + mx[0]) / 2.0, (mn[1] + mx[1]) / 2.0 - (mx[1] - mn[1])] } else { self.design.circ_center };
        let n = self.design.circ_count.max(2);
        let total = self.design.circ_angle;
        let step = if (total - 360.0).abs() < 1e-6 { 360.0 / n as f64 } else { total / (n - 1) as f64 };
        let src: Vec<Shape> = self.selected_shapes().into_iter().cloned().collect();
        let sc = [(mn[0] + mx[0]) / 2.0, (mn[1] + mx[1]) / 2.0];
        self.checkpoint();
        let mut sel = self.selection.clone();
        for i in 1..n {
            let ang = step * i as f64;
            let g = self.doc.new_id();
            for s in &src {
                let mut c = s.clone();
                c.id = self.doc.new_id();
                if c.group.is_some() {
                    c.group = Some(g);
                }
                if self.design.circ_rotate {
                    geom::rotate_about(&mut c, center, ang);
                } else {
                    // move without turning: rotate the selection center only
                    let (sn, cs) = ang.to_radians().sin_cos();
                    let (x, y) = (sc[0] - center[0], sc[1] - center[1]);
                    let nc = [center[0] + x * cs - y * sn, center[1] + x * sn + y * cs];
                    geom::translate(&mut c, [nc[0] - sc[0], nc[1] - sc[1]]);
                }
                for tp in &mut self.doc.toolpaths {
                    if tp.shape_ids.contains(&s.id) {
                        tp.shape_ids.push(c.id);
                    }
                }
                sel.push(c.id);
                self.doc.shapes.push(c);
            }
        }
        self.selection = sel;
        self.status = format!("{} copies around a circle", n);
    }

    pub fn reorder(&mut self, front: bool) {
        let sel = self.selection.clone();
        self.checkpoint();
        let (mut picked, rest): (Vec<Shape>, Vec<Shape>) = std::mem::take(&mut self.doc.shapes).into_iter().partition(|s| sel.contains(&s.id));
        let mut out = rest;
        if front {
            out.append(&mut picked);
        } else {
            picked.append(&mut out);
            out = picked;
        }
        self.doc.shapes = out;
        // keep toolpath shape lists in drawing order so cut order follows
        let order: Vec<u64> = self.doc.shapes.iter().map(|s| s.id).collect();
        for tp in &mut self.doc.toolpaths {
            tp.shape_ids.sort_by_key(|id| order.iter().position(|o| o == id).unwrap_or(usize::MAX));
        }
    }
}
