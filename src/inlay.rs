//! V-carve inlay: from the selected shapes it makes a matching pair, a V-carved pocket in the
//! work piece and a mirrored plug (with a border to carve it from) placed beside it, so the plug
//! can be glued in upside down and planed flush.

use crate::app::App;
use crate::model::*;
use crate::panels::len_edit;
use egui::RichText;

/// The plug's outlines: the shapes mirrored left-to-right about their middle and moved `dx` along,
/// plus a rectangle `margin` round them to carve the plug's background from.
pub fn plug_paths(parts: &[Vec<Path>], margin: f64, dx: f64) -> Option<(Vec<Path>, Path)> {
    let pts = parts.iter().flatten().flat_map(|p| p.pts.iter());
    let (mn, mx) = bbox_of(pts)?;
    let cx = (mn[0] + mx[0]) / 2.0;
    let mut mirrored = vec![];
    for p in parts.iter().flatten().filter(|p| p.closed && p.pts.len() >= 3) {
        let mut q = p.clone();
        q.pts.iter_mut().for_each(|v| v[0] = 2.0 * cx - v[0] + dx);
        q.pts.reverse();
        mirrored.push(q);
    }
    if mirrored.is_empty() {
        return None;
    }
    let (x0, x1, y0, y1) = (mn[0] - margin + dx, mx[0] + margin + dx, mn[1] - margin, mx[1] + margin);
    let border = Path { pts: vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]], closed: true };
    Some((mirrored, border))
}

impl App {
    pub fn inlay_window(&mut self, ctx: &egui::Context) {
        if !self.design.inlay_open {
            return;
        }
        let mut open = true;
        let mut go = false;
        let u = self.settings.units;
        egui::Window::new("V-Carve inlay").open(&mut open).default_width(380.0).collapsible(false).show(ctx, |ui| {
            ui.label(RichText::new("Makes a V-carved pocket from the selected shapes and a mirrored plug beside them. Carve the plug from the contrasting wood, flip it, glue it into the pocket and plane it flush."));
            let d = &mut self.design;
            egui::Grid::new("inlaygrid").num_columns(2).spacing([10.0, 5.0]).show(ui, |ui| {
                ui.label("Pocket depth");
                len_edit(ui, &mut d.inlay_depth, u, 0.1);
                d.inlay_depth = d.inlay_depth.max(0.2);
                ui.end_row();
                ui.label("Glue gap").on_hover_text("Room left under the plug for glue");
                len_edit(ui, &mut d.inlay_gap, u, 0.05);
                d.inlay_gap = d.inlay_gap.clamp(0.0, d.inlay_depth - 0.1);
                ui.end_row();
                ui.label("Plug clearance").on_hover_text("How far the plug's background stands off the work when pressed in");
                len_edit(ui, &mut d.inlay_clear, u, 0.1);
                d.inlay_clear = d.inlay_clear.max(0.0);
                ui.end_row();
                ui.label("Plug border");
                len_edit(ui, &mut d.inlay_margin, u, 0.5);
                d.inlay_margin = d.inlay_margin.max(1.0);
                ui.end_row();
            });
            let (dp, gap, cl) = (self.design.inlay_depth, self.design.inlay_gap, self.design.inlay_clear);
            ui.label(RichText::new(format!("Pocket: V-carve to {dp:.1} mm.  Plug: start depth {:.1} mm, then {cl:.1} mm deeper.", dp - gap)).small());
            ui.separator();
            if ui.add_enabled(!self.selection.is_empty(), egui::Button::new("Make pocket and plug")).on_disabled_hover_text("Select closed shapes or text first").clicked() {
                go = true;
            }
        });
        if go {
            self.make_inlay();
        }
        if !open {
            self.design.inlay_open = false;
        }
    }

    pub fn make_inlay(&mut self) {
        let sel = self.selection.clone();
        let parts: Vec<Vec<Path>> = self.selected_shapes().iter().map(|s| s.paths.clone()).collect();
        let Some((mn, mx)) = self.selection_bbox() else { return };
        let (dp, gap, cl, margin) = (self.design.inlay_depth, self.design.inlay_gap, self.design.inlay_clear, self.design.inlay_margin);
        let dx = (mx[0] - mn[0]) + 2.0 * margin + 10.0;
        let Some((plug, border)) = plug_paths(&parts, margin, dx) else {
            self.status = "V-Carve inlay needs closed shapes or text".into();
            return;
        };
        let Some(vbit) = self.settings.tools.iter().find(|t| t.kind == ToolKind::VBit).cloned() else {
            self.status = "V-Carve inlay needs a V-bit in the tool library".into();
            return;
        };
        self.checkpoint();
        let plug_ids = self.add_parts(vec![plug], "Inlay plug");
        let border_ids = self.add_parts(vec![vec![border]], "Inlay plug border");
        let g = self.doc.new_id();
        for id in plug_ids.iter().chain(&border_ids) {
            if let Some(s) = self.doc.shapes.iter_mut().find(|s| s.id == *id) {
                s.group = Some(g);
            }
        }
        let id = self.doc.new_id();
        let mut pocket = Toolpath::new(id, OpKind::VCarve, &vbit, sel, self.doc.toolpaths.len());
        pocket.name = "Inlay pocket".into();
        pocket.start_depth = 0.0;
        pocket.vcarve_max_depth = dp;
        self.doc.toolpaths.push(pocket);
        let id = self.doc.new_id();
        let ids: Vec<u64> = plug_ids.iter().chain(&border_ids).copied().collect();
        let mut plugtp = Toolpath::new(id, OpKind::VCarve, &vbit, ids.clone(), self.doc.toolpaths.len());
        plugtp.name = "Inlay plug (cut from the inlay wood)".into();
        plugtp.start_depth = dp - gap;
        plugtp.vcarve_max_depth = cl.max(0.1);
        self.doc.toolpaths.push(plugtp);
        self.selected_tp = Some(id);
        self.selection = ids;
        self.job_dirty = true;
        self.status = "Inlay pocket and mirrored plug made - the plug is meant for a separate piece of wood".into();
    }
}
