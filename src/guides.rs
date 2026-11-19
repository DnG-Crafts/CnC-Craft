//! Workspace helpers: ruler guides, smart guides while moving, symmetry drawing,
//! dimension lines and bounding-box nesting.

use crate::app::{App, CanvasTool};
use crate::geom;
use crate::model::*;
use crate::panels::{len_edit, section};
use egui::{Color32, Pos2, RichText, Stroke, Ui};

pub const GUIDE_COL: Color32 = Color32::from_rgb(70, 200, 230);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Symmetry {
    Off,
    MirrorX,
    MirrorY,
    FourWay,
    Radial,
}

impl Symmetry {
    pub const ALL: [Symmetry; 5] = [Symmetry::Off, Symmetry::MirrorX, Symmetry::MirrorY, Symmetry::FourWay, Symmetry::Radial];
    pub fn label(self) -> &'static str {
        match self {
            Symmetry::Off => "Off",
            Symmetry::MirrorX => "Mirror left-right",
            Symmetry::MirrorY => "Mirror top-bottom",
            Symmetry::FourWay => "Four-way mirror",
            Symmetry::Radial => "Radial (mandala)",
        }
    }
}

/// x' = a x + b y + c ; y' = d x + e y + f
pub type Affine = [f64; 6];

fn apply(m: &Affine, p: P) -> P {
    [m[0] * p[0] + m[1] * p[1] + m[2], m[3] * p[0] + m[4] * p[1] + m[5]]
}

fn about(c: P, a: f64, b: f64, d: f64, e: f64) -> Affine {
    // M(p - c) + c
    [a, b, c[0] - a * c[0] - b * c[1], d, e, c[1] - d * c[0] - e * c[1]]
}

impl App {
    // ------------------------------------------------------------ symmetry

    /// The extra copies symmetry drawing makes (identity left out).
    pub fn sym_transforms(&self) -> Vec<Affine> {
        let c = self.design.sym_center;
        match self.design.sym {
            Symmetry::Off => vec![],
            Symmetry::MirrorX => vec![about(c, -1.0, 0.0, 0.0, 1.0)],
            Symmetry::MirrorY => vec![about(c, 1.0, 0.0, 0.0, -1.0)],
            Symmetry::FourWay => vec![about(c, -1.0, 0.0, 0.0, 1.0), about(c, 1.0, 0.0, 0.0, -1.0), about(c, -1.0, 0.0, 0.0, -1.0)],
            Symmetry::Radial => {
                let n = self.design.sym_n.clamp(2, 64);
                (1..n)
                    .map(|k| {
                        let ang = std::f64::consts::TAU * k as f64 / n as f64;
                        let (s, co) = ang.sin_cos();
                        if self.design.sym_alt && k % 2 == 1 {
                            // mirror alternate slices (kaleidoscope): reflect across the slice's axis
                            let a2 = ang;
                            let (s2, c2) = (a2.sin(), a2.cos());
                            about(c, c2, s2, s2, -c2)
                        } else {
                            about(c, co, -s, s, co)
                        }
                    })
                    .collect()
            }
        }
    }

    /// Adds a freshly drawn shape, plus its symmetry copies (grouped) when symmetry is on.
    pub fn add_drawn(&mut self, kind: ShapeKind, paths: Vec<Path>, text: Option<String>) -> u64 {
        let id = self.add_shape(kind, paths.clone(), text.clone());
        let ts = self.sym_transforms();
        if ts.is_empty() {
            return id;
        }
        let g = self.doc.new_id();
        if let Some(s) = self.doc.shapes.iter_mut().find(|s| s.id == id) {
            s.group = Some(g);
        }
        let mut ids = vec![id];
        for m in ts {
            let mirrored = (m[0] * m[4] - m[1] * m[3]) < 0.0;
            let new_paths: Vec<Path> = paths
                .iter()
                .map(|p| {
                    let mut pts: Vec<P> = p.pts.iter().map(|q| apply(&m, *q)).collect();
                    if mirrored {
                        pts.reverse();
                    }
                    Path { pts, closed: p.closed }
                })
                .collect();
            let nid = self.doc.new_id();
            self.doc.shapes.push(Shape { id: nid, kind, paths: new_paths, group: Some(g), text: text.clone(), fx: None });
            ids.push(nid);
        }
        self.selection = ids;
        id
    }

    pub fn paint_symmetry(&self, painter: &egui::Painter, rect: egui::Rect, preview: Option<&Path>) {
        if self.design.sym == Symmetry::Off {
            return;
        }
        let ts = |p: P| self.to_screen(rect, p);
        let c = self.design.sym_center;
        let s = Stroke::new(1.0f32, Color32::from_rgb(90, 150, 255));
        let far = 1e4;
        let axis = |ang: f64| {
            let d = [ang.cos() * far, ang.sin() * far];
            painter.extend(egui::Shape::dashed_line(&[ts([c[0] - d[0], c[1] - d[1]]), ts([c[0] + d[0], c[1] + d[1]])], s, 6.0, 4.0));
        };
        match self.design.sym {
            Symmetry::MirrorX => axis(std::f64::consts::FRAC_PI_2),
            Symmetry::MirrorY => axis(0.0),
            Symmetry::FourWay => {
                axis(0.0);
                axis(std::f64::consts::FRAC_PI_2);
            }
            Symmetry::Radial => {
                let n = self.design.sym_n.clamp(2, 64);
                for k in 0..n {
                    let a = std::f64::consts::TAU * k as f64 / n as f64;
                    let d = [a.cos() * far, a.sin() * far];
                    painter.extend(egui::Shape::dashed_line(&[ts(c), ts([c[0] + d[0], c[1] + d[1]])], s, 6.0, 4.0));
                }
            }
            Symmetry::Off => {}
        }
        if let Some(p) = preview {
            for m in self.sym_transforms() {
                let pts: Vec<Pos2> = p.pts.iter().map(|q| ts(apply(&m, *q))).collect();
                let st = Stroke::new(1.0f32, Color32::from_rgba_unmultiplied(240, 140, 50, 110));
                if p.closed {
                    painter.add(egui::Shape::closed_line(pts, st));
                } else {
                    painter.add(egui::Shape::line(pts, st));
                }
            }
        }
    }

    // ------------------------------------------------------------ guides

    /// Snaps a point to the nearest guides (each axis on its own) within `tol`.
    pub fn guide_snap(&self, p: P, tol: f64) -> (P, bool) {
        let mut q = p;
        let mut hit = false;
        if !self.design.show_guides {
            return (q, false);
        }
        let mut bx = tol;
        let mut by = tol;
        for (vertical, pos) in &self.doc.guides {
            if *vertical {
                let d = (p[0] - pos).abs();
                if d < bx {
                    bx = d;
                    q[0] = *pos;
                    hit = true;
                }
            } else {
                let d = (p[1] - pos).abs();
                if d < by {
                    by = d;
                    q[1] = *pos;
                    hit = true;
                }
            }
        }
        (q, hit)
    }

    /// The guide under a screen point (within 4 px).
    pub fn guide_at(&self, rect: egui::Rect, s: Pos2) -> Option<usize> {
        if !self.design.show_guides {
            return None;
        }
        self.doc.guides.iter().position(|(vertical, pos)| {
            let g = self.to_screen(rect, [*pos, *pos]);
            if *vertical {
                (g.x - s.x).abs() < 4.0
            } else {
                (g.y - s.y).abs() < 4.0
            }
        })
    }

    pub fn paint_guides(&self, painter: &egui::Painter, rect: egui::Rect) {
        if !self.design.show_guides {
            return;
        }
        let s = Stroke::new(1.0f32, GUIDE_COL);
        for (vertical, pos) in &self.doc.guides {
            let g = self.to_screen(rect, [*pos, *pos]);
            if *vertical {
                painter.extend(egui::Shape::dashed_line(&[Pos2::new(g.x, rect.top()), Pos2::new(g.x, rect.bottom())], s, 7.0, 4.0));
            } else {
                painter.extend(egui::Shape::dashed_line(&[Pos2::new(rect.left(), g.y), Pos2::new(rect.right(), g.y)], s, 7.0, 4.0));
            }
        }
        // smart guide lines while moving
        let sg = Stroke::new(1.0f32, Color32::from_rgb(255, 80, 220));
        for (vertical, pos) in &self.design.smart_lines {
            let g = self.to_screen(rect, [*pos, *pos]);
            if *vertical {
                painter.line_segment([Pos2::new(g.x, rect.top()), Pos2::new(g.x, rect.bottom())], sg);
            } else {
                painter.line_segment([Pos2::new(rect.left(), g.y), Pos2::new(rect.right(), g.y)], sg);
            }
        }
    }

    /// Starts a guide drag: out of a ruler (a new guide), or on an existing guide with the Select
    /// tool when no shape is under the pointer.
    pub fn guide_drag_start(&mut self, rect: egui::Rect, s: Pos2, start: P, tol: f64) -> Option<crate::app::Drag> {
        const RULER: f32 = 20.0;
        if self.tool == CanvasTool::Pan {
            return None;
        }
        let top_bottom = s.y < rect.top() + RULER || s.y > rect.bottom() - RULER;
        let sides = s.x < rect.left() + RULER || s.x > rect.right() - RULER;
        if top_bottom || sides {
            // dragging down from the top/bottom ruler makes a horizontal guide; from the sides a vertical one
            let vertical = sides && !top_bottom;
            self.design.show_guides = true;
            self.doc.guides.push((vertical, if vertical { start[0] } else { start[1] }));
            self.status = "Drag the guide into place; drop it back on a ruler to remove it".into();
            return Some(crate::app::Drag::Guide { vertical, idx: self.doc.guides.len() - 1 });
        }
        if self.tool == CanvasTool::Select && self.hits_at(start, tol).is_empty() {
            if let Some(i) = self.guide_at(rect, s) {
                return Some(crate::app::Drag::Guide { vertical: self.doc.guides[i].0, idx: i });
            }
        }
        None
    }

    // ------------------------------------------------------------ smart guides

    /// Nudges a move so the selection's edges or centre line up with other shapes, the stock or
    /// guides. Returns the corrected move and remembers the lines to draw.
    pub fn smart_adjust(&mut self, d: P, applied: P, tol: f64) -> P {
        self.design.smart_lines.clear();
        if !self.design.smart_guides {
            return d;
        }
        let Some((mn, mx)) = self.selection_bbox() else { return d };
        let shift = [d[0] - applied[0], d[1] - applied[1]];
        let xs_self = [mn[0] + shift[0], (mn[0] + mx[0]) / 2.0 + shift[0], mx[0] + shift[0]];
        let ys_self = [mn[1] + shift[1], (mn[1] + mx[1]) / 2.0 + shift[1], mx[1] + shift[1]];
        let (w, h) = (self.doc.stock.width, self.doc.stock.height);
        let mut xs = vec![0.0, w / 2.0, w];
        let mut ys = vec![0.0, h / 2.0, h];
        for s in self.doc.shapes.iter().filter(|s| !self.selection.contains(&s.id)) {
            if let Some((a, b)) = s.bbox() {
                xs.extend([a[0], (a[0] + b[0]) / 2.0, b[0]]);
                ys.extend([a[1], (a[1] + b[1]) / 2.0, b[1]]);
            }
        }
        for (v, p) in &self.doc.guides {
            if *v {
                xs.push(*p);
            } else {
                ys.push(*p);
            }
        }
        let best = |own: &[f64; 3], cands: &[f64]| -> Option<(f64, f64)> {
            let mut b: Option<(f64, f64)> = None;
            for o in own {
                for c in cands {
                    let dd = c - o;
                    if dd.abs() <= tol && b.map(|x| dd.abs() < x.0.abs()).unwrap_or(true) {
                        b = Some((dd, *c));
                    }
                }
            }
            b
        };
        let mut out = d;
        if let Some((dx, line)) = best(&xs_self, &xs) {
            out[0] += dx;
            self.design.smart_lines.push((true, line));
        }
        if let Some((dy, line)) = best(&ys_self, &ys) {
            out[1] += dy;
            self.design.smart_lines.push((false, line));
        }
        out
    }

    // ------------------------------------------------------------ dimensions

    pub fn paint_dims(&self, painter: &egui::Painter, rect: egui::Rect, preview: Option<(P, P)>) {
        let ts = |p: P| self.to_screen(rect, p);
        let col = Color32::from_rgb(250, 220, 90);
        let draw = |a: P, b: P, faint: bool| {
            let c = if faint { col.gamma_multiply(0.6) } else { col };
            let (pa, pb) = (ts(a), ts(b));
            let s = Stroke::new(1.2f32, c);
            painter.line_segment([pa, pb], s);
            let d = (pb - pa).normalized();
            let n = egui::vec2(-d.y, d.x) * 6.0;
            painter.line_segment([pa - n, pa + n], s);
            painter.line_segment([pb - n, pb + n], s);
            // arrow heads
            for (p, dir) in [(pa, d), (pb, -d)] {
                painter.line_segment([p, p + dir * 8.0 + n * 0.6], s);
                painter.line_segment([p, p + dir * 8.0 - n * 0.6], s);
            }
            let mid = Pos2::new((pa.x + pb.x) / 2.0, (pa.y + pb.y) / 2.0) + n * 1.6;
            let text = self.len_text(geom::dist(a, b));
            let galley = painter.layout_no_wrap(text, egui::FontId::proportional(11.5), c);
            let r = egui::Rect::from_center_size(mid, galley.size() + egui::vec2(6.0, 2.0));
            painter.rect_filled(r, 2.0, Color32::from_black_alpha(160));
            painter.galley(r.min + egui::vec2(3.0, 1.0), galley, c);
        };
        for (a, b) in &self.doc.dims {
            draw(*a, *b, false);
        }
        if let Some((a, b)) = preview {
            draw(a, b, true);
        }
    }

    /// The dimension line nearest a point (within `tol`).
    pub fn dim_at(&self, p: P, tol: f64) -> Option<usize> {
        self.doc.dims.iter().position(|(a, b)| geom::seg_dist(p, *a, *b) <= tol)
    }

    // ------------------------------------------------------------ nesting

    /// Packs the selected parts (groups kept together) into the stock in rows, biggest first.
    /// Parts that don't fit are left where they were.
    pub fn nest_selected(&mut self) {
        let sel = self.selection.clone();
        if sel.is_empty() {
            self.status = "Select the parts to nest first".into();
            return;
        }
        // units: groups or single shapes
        let mut units: Vec<Vec<u64>> = vec![];
        let mut keys: Vec<u64> = vec![];
        for s in self.doc.shapes.iter().filter(|s| sel.contains(&s.id)) {
            let key = s.group.unwrap_or(u64::MAX - s.id);
            match keys.iter().position(|k| *k == key) {
                Some(i) => units[i].push(s.id),
                None => {
                    keys.push(key);
                    units.push(vec![s.id]);
                }
            }
        }
        let gap = self.design.nest_spacing.max(0.0);
        let margin = self.design.nest_margin.max(0.0);
        let (sw, sh) = (self.doc.stock.width, self.doc.stock.height);
        let bbox_of_unit = |doc: &Document, u: &[u64]| bbox_of(doc.shapes.iter().filter(|s| u.contains(&s.id)).flat_map(|s| s.paths.iter().flat_map(|p| p.pts.iter())));
        self.checkpoint();
        // optionally turn parts so their long side lies along X (packs rows tighter)
        if self.design.nest_rotate {
            for u in &units {
                if let Some((a, b)) = bbox_of_unit(&self.doc, u) {
                    if (b[1] - a[1]) > (b[0] - a[0]) * 1.05 {
                        let c = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
                        for s in self.doc.shapes.iter_mut().filter(|s| u.contains(&s.id)) {
                            geom::rotate_about(s, c, 90.0);
                        }
                    }
                }
            }
        }
        let mut items: Vec<(Vec<u64>, P, P)> = units.into_iter().filter_map(|u| bbox_of_unit(&self.doc, &u).map(|(a, b)| (u, a, b))).collect();
        items.sort_by(|x, y| (y.2[1] - y.1[1]).total_cmp(&(x.2[1] - x.1[1])));
        let (mut x, mut y, mut row_h) = (margin, margin, 0.0f64);
        let mut placed = 0;
        let mut left_out = 0;
        for (u, a, b) in items {
            let (w, h) = (b[0] - a[0], b[1] - a[1]);
            if x + w > sw - margin + 1e-9 && x > margin {
                x = margin;
                y += row_h + gap;
                row_h = 0.0;
            }
            if x + w > sw - margin + 1e-9 || y + h > sh - margin + 1e-9 {
                left_out += 1;
                continue;
            }
            let d = [x - a[0], y - a[1]];
            for s in self.doc.shapes.iter_mut().filter(|s| u.contains(&s.id)) {
                geom::translate(s, d);
            }
            x += w + gap;
            row_h = row_h.max(h);
            placed += 1;
        }
        self.job_dirty = true;
        self.status = if left_out > 0 { format!("Nested {placed} part(s); {left_out} didn't fit on the stock and stayed put") } else { format!("Nested {placed} part(s)") };
    }

    pub fn nest_ui(&mut self, ui: &mut Ui) {
        let u = self.settings.units;
        section(ui, "Nesting");
        ui.label(RichText::new("Packs the selected parts onto the stock in rows to save material. Groups stay together.").small().weak());
        egui::Grid::new("nestgrid").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
            ui.label("Spacing");
            len_edit(ui, &mut self.design.nest_spacing, u, 0.1);
            ui.end_row();
            ui.label("Edge margin");
            len_edit(ui, &mut self.design.nest_margin, u, 0.1);
            ui.end_row();
            ui.label("Rotation");
            ui.checkbox(&mut self.design.nest_rotate, "Turn parts to lie flat");
            ui.end_row();
        });
        if ui.add_enabled(!self.selection.is_empty(), egui::Button::new("Nest selected")).clicked() {
            self.nest_selected();
        }
    }

    /// Extra View-menu items: guides, smart guides, symmetry, dimensions.
    pub fn view_menu_extra(&mut self, ui: &mut Ui) {
        ui.separator();
        ui.checkbox(&mut self.design.show_guides, "Show guides (drag from a ruler)");
        if ui.add_enabled(!self.doc.guides.is_empty(), egui::Button::new("Clear guides")).clicked() {
            ui.close_menu();
            self.doc.guides.clear();
        }
        ui.checkbox(&mut self.design.smart_guides, "Smart guides");
        ui.checkbox(&mut self.design.snap_objects, "Snap to objects");
        ui.checkbox(&mut self.settings.snap_grid, "Snap to grid");
        ui.separator();
        ui.menu_button("Symmetry", |ui| {
            for s in Symmetry::ALL {
                if ui.radio_value(&mut self.design.sym, s, s.label()).clicked() && s != Symmetry::Off && self.design.sym_center == [0.0, 0.0] {
                    self.design.sym_center = [self.doc.stock.width / 2.0, self.doc.stock.height / 2.0];
                }
            }
            if self.design.sym == Symmetry::Radial {
                ui.horizontal(|ui| {
                    ui.label("Copies");
                    ui.add(egui::DragValue::new(&mut self.design.sym_n).range(2..=64));
                });
                ui.checkbox(&mut self.design.sym_alt, "Mirror alternate slices");
            }
            ui.separator();
            if ui.button("Centre on the stock").clicked() {
                self.design.sym_center = [self.doc.stock.width / 2.0, self.doc.stock.height / 2.0];
            }
            if ui.add_enabled(!self.selection.is_empty(), egui::Button::new("Centre on the selection")).clicked() {
                if let Some((a, b)) = self.selection_bbox() {
                    self.design.sym_center = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
                }
            }
        });
        ui.separator();
        if ui.button("Dimension tool").clicked() {
            ui.close_menu();
            self.tool = CanvasTool::Dimension;
        }
        if ui.add_enabled(!self.doc.dims.is_empty(), egui::Button::new("Clear dimensions")).clicked() {
            ui.close_menu();
            self.checkpoint();
            self.doc.dims.clear();
        }
    }
}
