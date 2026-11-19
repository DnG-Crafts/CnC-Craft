//! The Modify panel: object tools working on the selection.

use crate::app::App;
use crate::modify::{self, DepthKind, StoneLayout, Warp};
use crate::model::*;
use crate::panels::len_edit;
use egui::{RichText, Ui};

pub struct ModifyState {
    pub warp: Warp,
    pub warp_amount: f64,
    pub smooth_passes: u32,
    pub hatch_spacing: f64,
    pub hatch_angle: f64,
    pub hatch_cross: bool,
    pub lattice_cell: f64,
    pub lattice_bar: f64,
    pub stone_d: f64,
    pub stone_gap: f64,
    pub stone_layout: StoneLayout,
    pub stone_points: bool,
    pub knock_gap: f64,
    pub inlay_clear: f64,
    pub inlay_mirror: bool,
    pub contour_dist: f64,
    pub stencil_w: f64,
    pub stencil_h: bool,
    pub depth_kind: DepthKind,
    pub depth_dx: f64,
    pub depth_dy: f64,
    pub radial_n: u32,
    pub radial_r: f64,
    pub radial_weld: bool,
    pub blend_steps: u32,
    pub along_count: u32,
    pub along_rotate: bool,
    pub fit_margin: f64,
    pub dock_gap: f64,
    pub outline_kind: u8,
}

impl Default for ModifyState {
    fn default() -> Self {
        ModifyState {
            warp: Warp::Arch,
            warp_amount: 0.3,
            smooth_passes: 2,
            hatch_spacing: 2.0,
            hatch_angle: 45.0,
            hatch_cross: false,
            lattice_cell: 10.0,
            lattice_bar: 2.0,
            stone_d: 3.0,
            stone_gap: 1.0,
            stone_layout: StoneLayout::Outline,
            stone_points: false,
            knock_gap: 0.2,
            inlay_clear: 0.2,
            inlay_mirror: true,
            contour_dist: 3.0,
            stencil_w: 2.0,
            stencil_h: false,
            depth_kind: DepthKind::DropShadow,
            depth_dx: 3.0,
            depth_dy: -3.0,
            radial_n: 8,
            radial_r: 20.0,
            radial_weld: false,
            blend_steps: 4,
            along_count: 8,
            along_rotate: true,
            fit_margin: 2.0,
            dock_gap: 0.0,
            outline_kind: 0,
        }
    }
}

fn hint(ui: &mut Ui, s: &str) {
    ui.label(RichText::new(s).small().weak());
}

impl App {
    fn sel_parts(&self) -> Vec<(u64, Vec<Path>)> {
        self.selected_shapes().iter().map(|s| (s.id, s.paths.clone())).collect()
    }

    fn largest_selected(&self) -> Option<u64> {
        self.selected_shapes()
            .iter()
            .filter_map(|s| s.bbox().map(|(a, b)| (s.id, (b[0] - a[0]).hypot(b[1] - a[1]))))
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|x| x.0)
    }

    /// Adds shapes (one per entry) without moving them; grouped when several. Returns their ids.
    pub fn add_parts(&mut self, parts: Vec<Vec<Path>>, label: &str) -> Vec<u64> {
        let parts: Vec<Vec<Path>> = parts.into_iter().filter(|p| !p.is_empty()).collect();
        if parts.is_empty() {
            self.status = format!("{label}: nothing came out - check the selection and settings");
            return vec![];
        }
        let group = if parts.len() > 1 { Some(self.doc.new_id()) } else { None };
        let mut ids = vec![];
        for paths in parts {
            let id = self.doc.new_id();
            let kind = if paths.iter().all(|p| p.pts.len() == 1) { ShapeKind::Point } else { ShapeKind::Path };
            self.doc.shapes.push(Shape { id, kind, paths, group, text: None, fx: None });
            ids.push(id);
        }
        self.status = format!("{label} done");
        ids
    }

    /// Removes shapes, handing their toolpaths over to `heir`.
    pub fn remove_into(&mut self, old: &[u64], heir: Option<u64>) {
        self.doc.shapes.retain(|s| !old.contains(&s.id));
        for tp in &mut self.doc.toolpaths {
            let had = tp.shape_ids.iter().any(|id| old.contains(id));
            tp.shape_ids.retain(|id| !old.contains(id));
            if let (true, Some(h)) = (had, heir) {
                if !tp.shape_ids.contains(&h) {
                    tp.shape_ids.push(h);
                }
            }
        }
    }

    /// Replaces each selected shape's outline with `f(paths)` (empty results leave it alone).
    fn replace_each(&mut self, label: &str, f: impl Fn(&[Path]) -> Option<Vec<Path>>) {
        let parts = self.sel_parts();
        if parts.is_empty() {
            return;
        }
        self.checkpoint();
        let mut changed = 0;
        for (id, paths) in parts {
            if let Some(new) = f(&paths).filter(|n| !n.is_empty()) {
                if let Some(s) = self.doc.shapes.iter_mut().find(|s| s.id == id) {
                    s.paths = new;
                    if s.kind != ShapeKind::Point {
                        s.kind = ShapeKind::Path;
                    }
                    changed += 1;
                }
            }
        }
        if changed == 0 {
            self.undo.pop();
        }
        self.status = format!("{label}: {changed} shape(s) changed");
    }

    pub fn modify_tab(&mut self, ui: &mut Ui) {
        let u = self.settings.units;
        let n = self.selection.len();
        let has = n > 0;
        ui.label(RichText::new("Tools for changing shapes. Select shapes on the design, set the tool, then press its button.").weak());
        if !has {
            ui.label(RichText::new("Nothing is selected yet.").strong());
        }
        let m = &mut self.design.modify;
        let mut act: Option<&str> = None;
        let btn = |ui: &mut Ui, label: &str, key: &'static str, enabled: bool, act: &mut Option<&str>| {
            if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                *act = Some(key);
            }
        };

        egui::CollapsingHeader::new(RichText::new("Warp").strong()).default_open(true).show(ui, |ui| {
            hint(ui, "Bend the selection inside its bounds: arch, bulge, taper, wave, flag and more.");
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("warpkind").selected_text(m.warp.label()).show_ui(ui, |ui| {
                    for w in Warp::ALL {
                        ui.selectable_value(&mut m.warp, w, w.label());
                    }
                });
                ui.add(egui::Slider::new(&mut m.warp_amount, -1.0..=1.0).text("amount"));
            });
            btn(ui, "Warp the selection", "warp", has, &mut act);
        });
        egui::CollapsingHeader::new(RichText::new("Smooth").strong()).show(ui, |ui| {
            hint(ui, "Round off jagged outlines (traced or imported) into gentle curves.");
            ui.horizontal(|ui| {
                ui.label("Strength");
                ui.add(egui::DragValue::new(&mut m.smooth_passes).range(1..=6));
                btn(ui, "Smooth", "smooth", has, &mut act);
            });
        });
        egui::CollapsingHeader::new(RichText::new("Hatch fill").strong()).show(ui, |ui| {
            hint(ui, "Parallel lines across the shapes, for engraving a fill with a V-bit or for texture.");
            ui.horizontal(|ui| {
                ui.label("Spacing");
                len_edit(ui, &mut m.hatch_spacing, u, 0.05);
                ui.label("Angle");
                ui.add(egui::DragValue::new(&mut m.hatch_angle).speed(1.0).suffix("°"));
                ui.checkbox(&mut m.hatch_cross, "Cross");
            });
            btn(ui, "Make hatch lines", "hatch", has, &mut act);
        });
        egui::CollapsingHeader::new(RichText::new("Lattice").strong()).show(ui, |ui| {
            hint(ui, "Square holes on a grid inside the shape: vents, grilles and panels.");
            ui.horizontal(|ui| {
                ui.label("Cell");
                len_edit(ui, &mut m.lattice_cell, u, 0.1);
                ui.label("Bar");
                len_edit(ui, &mut m.lattice_bar, u, 0.05);
            });
            btn(ui, "Make lattice", "lattice", has, &mut act);
        });
        egui::CollapsingHeader::new(RichText::new("Dot / hole template").strong()).show(ui, |ui| {
            hint(ui, "Evenly spaced holes along the outline or filling the shape (a rhinestone-style template).");
            ui.horizontal(|ui| {
                ui.label("Hole");
                len_edit(ui, &mut m.stone_d, u, 0.05);
                ui.label("Gap");
                len_edit(ui, &mut m.stone_gap, u, 0.05);
            });
            egui::ComboBox::from_id_salt("stonelay").selected_text(m.stone_layout.label()).show_ui(ui, |ui| {
                for l in StoneLayout::ALL {
                    ui.selectable_value(&mut m.stone_layout, l, l.label());
                }
            });
            ui.checkbox(&mut m.stone_points, "Drill points instead of circles");
            btn(ui, "Make holes", "stones", has, &mut act);
        });
        egui::CollapsingHeader::new(RichText::new("Knockout and inlay").strong()).show(ui, |ui| {
            hint(ui, "Knockout: the largest selected shape gets a recess where the others sit.");
            ui.horizontal(|ui| {
                ui.label("Gap");
                len_edit(ui, &mut m.knock_gap, u, 0.01);
                btn(ui, "Knockout", "knockout", n > 1, &mut act);
            });
            hint(ui, "Inlay: a pocket and a matching insert (placed to the right) from the selection.");
            ui.horizontal(|ui| {
                ui.label("Clearance");
                len_edit(ui, &mut m.inlay_clear, u, 0.01);
                ui.checkbox(&mut m.inlay_mirror, "Mirror insert");
                btn(ui, "Inlay", "inlay", has, &mut act);
            });
        });
        egui::CollapsingHeader::new(RichText::new("Outlines").strong()).show(ui, |ui| {
            hint(ui, "Contour: one cut line around everything, grown by the distance (badges, coasters).");
            ui.horizontal(|ui| {
                ui.label("Distance");
                len_edit(ui, &mut m.contour_dist, u, 0.1);
                btn(ui, "Contour", "contour", has, &mut act);
            });
            ui.horizontal(|ui| {
                ui.radio_value(&mut m.outline_kind, 0, "Convex hull");
                ui.radio_value(&mut m.outline_kind, 1, "Tightest rectangle");
                btn(ui, "Make", "hull", has, &mut act);
            });
        });
        egui::CollapsingHeader::new(RichText::new("Stencil bridges").strong()).show(ui, |ui| {
            hint(ui, "Strips across holes (the middle of O, A, B) so the islands stay attached.");
            ui.horizontal(|ui| {
                ui.label("Width");
                len_edit(ui, &mut m.stencil_w, u, 0.05);
                ui.checkbox(&mut m.stencil_h, "Across");
                btn(ui, "Add bridges", "stencil", has, &mut act);
            });
        });
        egui::CollapsingHeader::new(RichText::new("Shadow and 3D").strong()).show(ui, |ui| {
            egui::ComboBox::from_id_salt("depthk").selected_text(m.depth_kind.label()).show_ui(ui, |ui| {
                for k in DepthKind::ALL {
                    ui.selectable_value(&mut m.depth_kind, k, k.label());
                }
            });
            ui.horizontal(|ui| {
                if m.depth_kind == DepthKind::CastShadow {
                    ui.label("Lean");
                    ui.add(egui::DragValue::new(&mut m.depth_dx).speed(0.02).range(-3.0..=3.0));
                    ui.label("Squash");
                    ui.add(egui::DragValue::new(&mut m.depth_dy).speed(0.01).range(0.02..=2.0));
                } else {
                    ui.label("Offset X");
                    len_edit(ui, &mut m.depth_dx, u, 0.1);
                    ui.label("Y");
                    len_edit(ui, &mut m.depth_dy, u, 0.1);
                }
            });
            btn(ui, "Make", "depth", has, &mut act);
        });
        egui::CollapsingHeader::new(RichText::new("Radial repeat").strong()).show(ui, |ui| {
            hint(ui, "Copies turned round a point below the shape, like flower petals.");
            ui.horizontal(|ui| {
                ui.label("Copies");
                ui.add(egui::DragValue::new(&mut m.radial_n).range(2..=360));
                ui.label("Radius");
                len_edit(ui, &mut m.radial_r, u, 0.2);
                ui.checkbox(&mut m.radial_weld, "Weld");
            });
            btn(ui, "Repeat", "radial", has, &mut act);
        });
        egui::CollapsingHeader::new(RichText::new("Blend, clip and knife").strong()).show(ui, |ui| {
            hint(ui, "Blend: select two shapes; in-between shapes morph from one to the other.");
            ui.horizontal(|ui| {
                ui.label("Steps");
                ui.add(egui::DragValue::new(&mut m.blend_steps).range(1..=200));
                btn(ui, "Blend", "blend", n == 2, &mut act);
            });
            hint(ui, "Clip: keep only what lies inside the largest selected shape (it is removed).");
            btn(ui, "Clip to the largest shape", "clip", n > 1, &mut act);
            hint(ui, "Knife: draw a straight line across shapes (Line tool), select the line and the shapes, then cut.");
            btn(ui, "Cut with the line", "knife", n > 1, &mut act);
        });
        egui::CollapsingHeader::new(RichText::new("Place and fit").strong()).show(ui, |ui| {
            hint(ui, "Copy along path: copies of the other shapes spaced along the largest one.");
            ui.horizontal(|ui| {
                ui.label("Copies");
                ui.add(egui::DragValue::new(&mut m.along_count).range(1..=500));
                ui.checkbox(&mut m.along_rotate, "Follow the path");
                btn(ui, "Copy along", "along", n > 1, &mut act);
            });
            hint(ui, "Fit into: scale the other shapes to fit inside the largest one.");
            ui.horizontal(|ui| {
                ui.label("Margin");
                len_edit(ui, &mut m.fit_margin, u, 0.1);
                btn(ui, "Fit into", "fit", n > 1, &mut act);
            });
            hint(ui, "Dock: slide the shapes against each other towards one side.");
            ui.horizontal(|ui| {
                ui.label("Gap");
                len_edit(ui, &mut m.dock_gap, u, 0.1);
                for (k, l) in [("dock_l", "◀"), ("dock_r", "▶"), ("dock_u", "▲"), ("dock_d", "▼")] {
                    btn(ui, l, k, n > 1, &mut act);
                }
            });
        });
        if let Some(a) = act {
            self.run_modify(a);
        }
    }

    pub fn run_modify(&mut self, which: &str) {
        let m = &self.design.modify;
        let parts = self.sel_parts();
        let all: Vec<Vec<Path>> = parts.iter().map(|p| p.1.clone()).collect();
        match which {
            "warp" => {
                let Some(area) = self.selection_bbox() else { return };
                let (k, a) = (m.warp, m.warp_amount);
                self.replace_each("Warp", |p| Some(modify::warp(p, area, k, a)));
            }
            "smooth" => {
                let n = m.smooth_passes;
                self.replace_each("Smooth", |p| Some(modify::smooth(p, n)));
            }
            "stencil" => {
                let (w, h) = (m.stencil_w, m.stencil_h);
                self.replace_each("Stencil bridges", |p| modify::stencil_bridges(p, w, h));
            }
            "hatch" => {
                let lines = modify::hatch(&all, m.hatch_spacing, m.hatch_angle, m.hatch_cross);
                self.checkpoint();
                let ids = self.add_parts(vec![lines], "Hatch fill");
                if !ids.is_empty() {
                    self.selection = ids;
                }
            }
            "lattice" => {
                let l = modify::lattice(&all, m.lattice_cell, m.lattice_bar);
                if l.len() < 2 {
                    self.status = "Lattice: the shape is too small for that cell size".into();
                    return;
                }
                self.checkpoint();
                let ids = self.add_parts(vec![l], "Lattice");
                let old: Vec<u64> = parts.iter().map(|p| p.0).collect();
                self.remove_into(&old, ids.first().copied());
                self.selection = ids;
            }
            "stones" => {
                let c = modify::stone_centres(&all, m.stone_d, m.stone_gap, m.stone_layout);
                let holes = modify::holes_at(&c, m.stone_d, m.stone_points);
                self.checkpoint();
                let ids = self.add_parts(holes, "Hole template");
                if !ids.is_empty() {
                    self.selection = ids;
                }
            }
            "knockout" => {
                let Some(big) = self.largest_selected() else { return };
                let bottom: Vec<Vec<Path>> = parts.iter().filter(|p| p.0 == big).map(|p| p.1.clone()).collect();
                let top: Vec<Vec<Path>> = parts.iter().filter(|p| p.0 != big).map(|p| p.1.clone()).collect();
                let res = modify::knockout(&bottom, &top, m.knock_gap);
                if res.is_empty() {
                    self.status = "Knockout: nothing left".into();
                    return;
                }
                self.checkpoint();
                if let Some(s) = self.doc.shapes.iter_mut().find(|s| s.id == big) {
                    s.paths = res;
                    s.kind = ShapeKind::Path;
                }
                self.status = "Knockout done".into();
            }
            "inlay" => {
                let Some((pocket, insert)) = modify::inlay(&all, m.inlay_clear, m.inlay_mirror) else {
                    self.status = "Inlay needs closed shapes".into();
                    return;
                };
                self.checkpoint();
                let ids = self.add_parts(vec![pocket], "Inlay pocket");
                let ins = self.add_parts(vec![insert], "Inlay");
                let old: Vec<u64> = parts.iter().map(|p| p.0).collect();
                self.remove_into(&old, ids.first().copied());
                self.selection = ids.into_iter().chain(ins).collect();
            }
            "contour" => {
                let c = modify::contour(&all, m.contour_dist);
                self.checkpoint();
                let ids = self.add_parts(vec![c], "Contour");
                if !ids.is_empty() {
                    self.selection = ids;
                }
            }
            "hull" => {
                let pts: Vec<P> = all.iter().flatten().flat_map(|p| p.pts.iter().copied()).collect();
                let out = if m.outline_kind == 0 { Some(modify::convex_hull(&pts)) } else { modify::min_area_rect(&pts) };
                let Some(o) = out.filter(|o| o.len() >= 3) else { return };
                self.checkpoint();
                let ids = self.add_parts(vec![vec![Path { pts: o, closed: true }]], "Outline");
                self.selection = ids;
            }
            "depth" => {
                let d = modify::depth(&all, m.depth_kind, m.depth_dx, m.depth_dy);
                let lbl = m.depth_kind.label();
                self.checkpoint();
                let ids = self.add_parts(vec![d], lbl);
                // shadows sit behind the design
                if let Some(id) = ids.first() {
                    if let Some(i) = self.doc.shapes.iter().position(|s| s.id == *id) {
                        let s = self.doc.shapes.remove(i);
                        let first = self.doc.shapes.iter().position(|x| parts.iter().any(|p| p.0 == x.id)).unwrap_or(0);
                        self.doc.shapes.insert(first, s);
                    }
                }
            }
            "radial" => {
                let (n, r, w) = (m.radial_n, m.radial_r, m.radial_weld);
                self.replace_each("Radial repeat", |p| Some(modify::radial_repeat(p, n, r, w)));
            }
            "blend" => {
                if parts.len() != 2 {
                    return;
                }
                let steps = modify::blend(&parts[0].1, &parts[1].1, m.blend_steps);
                self.checkpoint();
                let ids = self.add_parts(steps, "Blend");
                self.selection.extend(ids);
            }
            "clip" => {
                let Some(big) = self.largest_selected() else { return };
                let mask = modify::region(&parts.iter().find(|p| p.0 == big).map(|p| p.1.clone()).unwrap_or_default());
                if mask.0.is_empty() {
                    self.status = "Clip: the largest shape must be closed".into();
                    return;
                }
                self.checkpoint();
                for (id, paths) in parts.iter().filter(|p| p.0 != big) {
                    let c = modify::clip(paths, &mask);
                    if let Some(s) = self.doc.shapes.iter_mut().find(|s| s.id == *id) {
                        s.paths = c;
                        s.kind = ShapeKind::Path;
                    }
                }
                self.doc.shapes.retain(|s| s.id == big || !s.paths.is_empty());
                self.remove_into(&[big], None);
                self.prune_selection();
                self.status = "Clipped".into();
            }
            "knife" => {
                // the knife is a selected open line; its first and last points set the cut
                let knife = self.selected_shapes().iter().find(|s| s.paths.len() == 1 && !s.paths[0].closed && s.paths[0].pts.len() >= 2).map(|s| (s.id, s.paths[0].pts[0], *s.paths[0].pts.last().unwrap()));
                let Some((kid, a, b)) = knife else {
                    self.status = "Knife: draw a straight line across the shapes with the Line tool and select it too".into();
                    return;
                };
                self.checkpoint();
                let mut cut = 0;
                for (id, paths) in parts.iter().filter(|p| p.0 != kid) {
                    if let Some(pieces) = modify::knife(paths, a, b) {
                        let ids = self.add_parts(pieces, "Knife");
                        self.remove_into(&[*id], ids.first().copied());
                        // every piece keeps the original's toolpaths
                        for tp in &mut self.doc.toolpaths {
                            if let Some(first) = ids.first() {
                                if tp.shape_ids.contains(first) {
                                    for i in &ids[1..] {
                                        tp.shape_ids.push(*i);
                                    }
                                }
                            }
                        }
                        cut += 1;
                    }
                }
                self.remove_into(&[kid], None);
                self.prune_selection();
                if cut == 0 {
                    self.undo.pop();
                    self.status = "Knife: the line doesn't cross right through any shape".into();
                } else {
                    self.status = format!("Knife cut {cut} shape(s)");
                }
            }
            "along" => {
                let Some(big) = self.largest_selected() else { return };
                let Some(guide) = parts.iter().find(|p| p.0 == big).and_then(|p| p.1.first().cloned()) else { return };
                let items: Vec<Shape> = self.selected_shapes().into_iter().filter(|s| s.id != big).cloned().collect();
                let Some((mn, mx)) = bbox_of(items.iter().flat_map(|s| s.paths.iter().flat_map(|p| p.pts.iter()))) else { return };
                let c = [(mn[0] + mx[0]) / 2.0, (mn[1] + mx[1]) / 2.0];
                let spots = modify::along_path(&guide, m.along_count as usize);
                let rotate = m.along_rotate;
                self.checkpoint();
                let mut new_ids = vec![];
                for (p, ang) in spots {
                    let g = self.doc.new_id();
                    for it in &items {
                        let mut s = it.clone();
                        s.id = self.doc.new_id();
                        s.group = if items.len() > 1 { Some(g) } else { None };
                        crate::geom::translate(&mut s, [p[0] - c[0], p[1] - c[1]]);
                        if rotate {
                            crate::geom::rotate_about(&mut s, p, ang.to_degrees());
                        }
                        new_ids.push(s.id);
                        self.doc.shapes.push(s);
                    }
                }
                self.selection = new_ids;
                self.status = "Copied along the path".into();
            }
            "fit" => {
                let Some(big) = self.largest_selected() else { return };
                let Some((fa, fb)) = self.doc.shape(big).and_then(|s| s.bbox()) else { return };
                let ids: Vec<u64> = self.selection.iter().copied().filter(|i| *i != big).collect();
                let Some((mn, mx)) = bbox_of(self.doc.shapes.iter().filter(|s| ids.contains(&s.id)).flat_map(|s| s.paths.iter().flat_map(|p| p.pts.iter()))) else { return };
                let margin = m.fit_margin;
                let room = |f: f64, i: f64| if i > 1e-9 { (f - 2.0 * margin).max(0.01) / i } else { f64::MAX };
                let k = room(fb[0] - fa[0], mx[0] - mn[0]).min(room(fb[1] - fa[1], mx[1] - mn[1]));
                let (fc, ic) = ([(fa[0] + fb[0]) / 2.0, (fa[1] + fb[1]) / 2.0], [(mn[0] + mx[0]) / 2.0, (mn[1] + mx[1]) / 2.0]);
                self.checkpoint();
                for s in self.doc.shapes.iter_mut().filter(|s| ids.contains(&s.id)) {
                    crate::geom::scale_about(s, ic, k, k);
                    crate::geom::translate(s, [fc[0] - ic[0], fc[1] - ic[1]]);
                }
                self.status = "Fitted inside".into();
            }
            "dock_l" | "dock_r" | "dock_u" | "dock_d" => {
                let Some((amn, amx)) = self.selection_bbox() else { return };
                let gap = m.dock_gap;
                let mut items: Vec<(u64, P, P)> = self.selected_shapes().iter().filter_map(|s| s.bbox().map(|(a, b)| (s.id, a, b))).collect();
                let key = |i: &(u64, P, P)| match which {
                    "dock_l" => i.1[0],
                    "dock_r" => -i.2[0],
                    "dock_d" => i.1[1],
                    _ => -i.2[1],
                };
                items.sort_by(|a, b| key(a).total_cmp(&key(b)));
                self.checkpoint();
                let mut edge = match which {
                    "dock_l" => amn[0],
                    "dock_r" => amx[0],
                    "dock_d" => amn[1],
                    _ => amx[1],
                };
                for (id, a, b) in items {
                    let d = match which {
                        "dock_l" => [edge - a[0], 0.0],
                        "dock_r" => [edge - b[0], 0.0],
                        "dock_d" => [0.0, edge - a[1]],
                        _ => [0.0, edge - b[1]],
                    };
                    if let Some(s) = self.doc.shapes.iter_mut().find(|s| s.id == id) {
                        crate::geom::translate(s, d);
                    }
                    edge = match which {
                        "dock_l" => b[0] + d[0] + gap,
                        "dock_r" => a[0] + d[0] - gap,
                        "dock_d" => b[1] + d[1] + gap,
                        _ => a[1] + d[1] - gap,
                    };
                }
                self.status = "Docked".into();
            }
            _ => {}
        }
        self.job_dirty = true;
    }
}
