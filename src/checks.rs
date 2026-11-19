//! Checks before cutting: the job is simulated in cutting order to catch rapid moves into
//! material still there, and the settings are checked for common mistakes. Errors stop
//! Start / Save G-code; warnings ask first.

use crate::app::App;
use crate::model::*;
use crate::sim::Sim;
use egui::{Color32, RichText};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Pending {
    Start,
    Save,
}

impl App {
    /// (is_error, message) for everything that looks wrong with the job.
    pub fn check_job(&mut self) -> Vec<(bool, String)> {
        if self.job_dirty {
            self.regenerate();
        }
        let mut out: Vec<(bool, String)> = vec![];
        let job = &self.job;
        let doc = &self.doc;
        let m = &self.settings.machine;
        if job.moves.is_empty() {
            out.push((true, "There is nothing to cut: add a toolpath and assign shapes to it.".into()));
            return out;
        }
        for w in &job.warnings {
            out.push((false, w.clone()));
        }
        let (sw, sh, thick) = (doc.stock.width, doc.stock.height, doc.stock.thickness);
        for (i, tp) in doc.toolpaths.iter().enumerate().filter(|(_, t)| t.enabled) {
            let tool = job.tools.get(i).and_then(|t| t.clone());
            if tp.feed <= 0.0 || tp.plunge <= 0.0 {
                out.push((true, format!("{}: the feed or plunge rate is zero.", tp.name)));
            }
            let knife = tool.as_ref().map_or(false, |t| t.kind == ToolKind::DragKnife);
            if tp.rpm <= 0.0 && !knife {
                out.push((false, format!("{}: no spindle speed is set.", tp.name)));
            }
            if knife && tp.rpm > 0.0 {
                out.push((true, format!("{}: a drag knife must not spin - set the spindle speed to 0.", tp.name)));
            }
            if knife && tp.op != OpKind::DragKnife {
                out.push((false, format!("{}: a drag knife can only follow lines with the Drag knife toolpath.", tp.name)));
            }
            if let Some(t) = &tool {
                if t.kind == ToolKind::ThreadMill && tp.op != OpKind::Thread {
                    out.push((false, format!("{}: a thread mill is only meant for thread milling.", tp.name)));
                }
                if t.kind == ToolKind::Drill && tp.op != OpKind::Drill {
                    out.push((false, format!("{}: a drill bit can only plunge, but this toolpath moves sideways.", tp.name)));
                }
                if t.kind == ToolKind::VBit && matches!(tp.op, OpKind::Pocket | OpKind::ProfileOutside | OpKind::ProfileInside | OpKind::Surface) {
                    out.push((false, format!("{}: a V-bit gives sloped walls here; a flat end mill is usual.", tp.name)));
                }
            }
            if !job.moves.iter().any(|mv| mv.tp == i) {
                out.push((false, format!("{}: it makes no moves.", tp.name)));
            }
        }
        // how deep and how far the job goes
        let mut min_z = f64::MAX;
        let mut outside = vec![];
        let (mut mnx, mut mny, mut mxx, mut mxy) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for mv in job.moves.iter().filter(|mv| !mv.rapid) {
            let tp = &doc.toolpaths[mv.tp];
            let r = job.tools.get(mv.tp).and_then(|t| t.as_ref()).map(|t| t.radius()).unwrap_or(0.0);
            if tp.op != OpKind::Surface {
                min_z = min_z.min(mv.z);
                let slack = r + 1.0;
                if mv.z < 0.0 && (mv.x < -slack || mv.y < -slack || mv.x > sw + slack || mv.y > sh + slack) && !outside.contains(&mv.tp) {
                    outside.push(mv.tp);
                }
            }
            mnx = mnx.min(mv.x - r);
            mny = mny.min(mv.y - r);
            mxx = mxx.max(mv.x + r);
            mxy = mxy.max(mv.y + r);
        }
        for i in outside {
            out.push((false, format!("{}: it cuts outside the stock.", doc.toolpaths[i].name)));
        }
        if min_z < -(thick + 1.0) {
            out.push((false, format!("The job cuts {:.1} mm into the spoilboard (more than 1 mm below the stock).", -(min_z + thick))));
        }
        if mxx - mnx > m.bed_w + 1e-6 || mxy - mny > m.bed_h + 1e-6 {
            out.push((true, format!("The job is {:.0} × {:.0} mm, bigger than the machine's {:.0} × {:.0} mm work area.", mxx - mnx, mxy - mny, m.bed_w, m.bed_h)));
        }
        if sw > m.bed_w + 1e-6 || sh > m.bed_h + 1e-6 {
            out.push((false, format!("The stock ({:.0} × {:.0} mm) is bigger than the machine's work area.", sw, sh)));
        }
        let travel = m.safe_z + thick.max(-min_z);
        if travel > m.z_travel {
            out.push((true, format!("The job needs {:.1} mm of Z travel; the machine has {:.1} mm.", travel, m.z_travel)));
        }
        if doc.toolpaths.iter().any(|t| t.enabled && t.op == OpKind::Surface && t.shape_ids.is_empty()) {
            out.push((false, "Surfacing flattens the whole stock: clear clamps and screws out of the way first.".into()));
        }
        // bits cutting deeper than their flutes, and parts cut free with nothing holding them
        for (i, tp) in doc.toolpaths.iter().enumerate().filter(|(_, t)| t.enabled) {
            let deepest = job.moves.iter().filter(|mv| mv.tp == i && !mv.rapid).map(|mv| -mv.z).fold(0.0, f64::max);
            if let Some(Some(t)) = job.tools.get(i) {
                if t.flute_length > 0.0 && deepest > t.flute_length + 1e-6 {
                    out.push((false, format!("{}: cuts {:.1} mm deep but the bit's flutes are only {:.1} mm long.", tp.name, deepest, t.flute_length)));
                }
            }
            if matches!(tp.op, OpKind::ProfileOutside | OpKind::ProfileInside) && !tp.tabs && tp.start_depth + tp.cut_depth >= thick - 1e-6 {
                out.push((false, format!("{}: cuts right through with no tabs, so the piece will come loose (fine if it's held by screws, tape or vacuum).", tp.name)));
            }
        }
        // clamps and other keep-out zones
        let zones: Vec<&Shape> = doc.shapes.iter().filter(|s| doc.keepout.contains(&s.id)).collect();
        if !zones.is_empty() {
            let near = |p: P, r: f64| zones.iter().find(|z| crate::geom::inside_shape(z, p) || crate::geom::outline_dist(z, p) <= r).map(|z| z.id);
            let mut hit: Vec<usize> = vec![];
            let mut over = false;
            let mut prev = [0.0, 0.0, m.safe_z];
            for mv in &job.moves {
                let r = job.tools.get(mv.tp).and_then(|t| t.as_ref()).map(|t| t.radius()).unwrap_or(0.0);
                let len = ((mv.x - prev[0]).powi(2) + (mv.y - prev[1]).powi(2)).sqrt();
                let n = ((len / 1.0).ceil() as usize).max(1);
                for k in 0..=n {
                    let t = k as f64 / n as f64;
                    let p = [prev[0] + (mv.x - prev[0]) * t, prev[1] + (mv.y - prev[1]) * t];
                    let z = prev[2] + (mv.z - prev[2]) * t;
                    if near(p, r).is_none() {
                        continue;
                    }
                    if z < self.settings.clamp_height - 1e-6 {
                        if mv.rapid || z > 0.0 {
                            over = true;
                        } else if !hit.contains(&mv.tp) {
                            hit.push(mv.tp);
                        }
                    }
                }
                prev = [mv.x, mv.y, mv.z];
            }
            for i in hit {
                out.push((true, format!("{}: the bit would cut into a clamp (keep-out zone).", doc.toolpaths[i].name)));
            }
            if over {
                out.push((true, format!("The bit passes over a clamp lower than the clamp height ({:.0} mm). Raise the safe height in Machine settings above your clamps.", self.settings.clamp_height)));
            }
        }
        // rapid moves into material that is still there
        let mut sim = Sim::new(&doc.stock, 50_000);
        let mut crashed: Vec<usize> = vec![];
        let n = job.moves.len();
        while sim.done_moves < n {
            let i = sim.done_moves;
            let mv = job.moves[i];
            let from = sim.tool_pos(job);
            let lifting_only = (mv.x - from[0]).abs() < 1e-6 && (mv.y - from[1]).abs() < 1e-6 && mv.z >= from[2];
            if mv.rapid && !lifting_only && (mv.z < -0.05 || from[2] < -0.05) && !crashed.contains(&mv.tp) {
                let len = ((mv.x - from[0]).powi(2) + (mv.y - from[1]).powi(2)).sqrt();
                let steps = ((len / (sim.cell * 0.7)).ceil() as usize).max(1);
                for k in 0..=steps {
                    let t = k as f64 / steps as f64;
                    let (x, y, z) = (from[0] + (mv.x - from[0]) * t, from[1] + (mv.y - from[1]) * t, from[2] + (mv.z - from[2]) * t);
                    let (ci, cj) = ((x / sim.cell).floor() as i64, (y / sim.cell).floor() as i64);
                    if ci < 0 || cj < 0 || ci >= sim.w as i64 || cj >= sim.h as i64 {
                        continue;
                    }
                    // solid material all round the tip (the grid is coarser than the bit's tip)
                    let mut lowest = f64::MAX;
                    for dj in -1..=1i64 {
                        for di in -1..=1i64 {
                            let (a, b) = ((ci + di).clamp(0, sim.w as i64 - 1) as usize, (cj + dj).clamp(0, sim.h as i64 - 1) as usize);
                            lowest = lowest.min(sim.heights[b * sim.w + a] as f64);
                        }
                    }
                    if lowest > z + 0.3 {
                        crashed.push(mv.tp);
                        out.push((true, format!("{}: a rapid move runs into material that is still there (near X {:.0} Y {:.0}). The bit would crash.", doc.toolpaths[mv.tp].name, x, y)));
                        break;
                    }
                }
            }
            let len3 = ((mv.x - from[0]).powi(2) + (mv.y - from[1]).powi(2) + (mv.z - from[2]).powi(2)).sqrt();
            sim.advance(job, len3 + 1e-9);
            if sim.done_moves == i {
                break;
            }
        }
        out.sort_by_key(|x| !x.0);
        out
    }

    /// Runs the checks before Start or Save; returns true when it's fine to go ahead now.
    pub fn checks_pass(&mut self, action: Pending) -> bool {
        if self.design.checks_bypass {
            self.design.checks_bypass = false;
            return true;
        }
        let found = self.check_job();
        if found.is_empty() {
            return true;
        }
        self.design.checks = found;
        self.design.checks_pending = Some(action);
        self.design.checks_open = true;
        false
    }

    pub fn checks_window(&mut self, ctx: &egui::Context) {
        if !self.design.checks_open {
            return;
        }
        let mut open = true;
        let mut go = false;
        let errors = self.design.checks.iter().filter(|c| c.0).count();
        egui::Window::new("Checks before cutting").open(&mut open).default_width(480.0).collapsible(false).show(ctx, |ui| {
            if self.design.checks.is_empty() {
                ui.label(RichText::new("✔ Everything looks fine.").color(Color32::from_rgb(90, 210, 110)));
            }
            for (err, msg) in &self.design.checks {
                ui.horizontal_wrapped(|ui| {
                    if *err {
                        ui.label(RichText::new("Error").strong().color(Color32::from_rgb(240, 90, 80)));
                    } else {
                        ui.label(RichText::new("Warning").strong().color(Color32::from_rgb(250, 200, 70)));
                    }
                    ui.label(msg);
                });
            }
            ui.separator();
            ui.horizontal(|ui| {
                if let Some(p) = self.design.checks_pending {
                    let label = if p == Pending::Start { "Start anyway" } else { "Save anyway" };
                    if ui.add_enabled(errors == 0, egui::Button::new(label)).on_disabled_hover_text("Fix the errors first").clicked() {
                        go = true;
                    }
                }
                if ui.button("Check again").clicked() {
                    self.design.checks = self.check_job();
                }
            });
        });
        if go {
            self.design.checks_open = false;
            self.design.checks_bypass = true;
            match self.design.checks_pending.take() {
                Some(Pending::Start) => self.start_job(),
                Some(Pending::Save) => self.save_gcode(),
                None => {}
            }
            return;
        }
        if !open {
            self.design.checks_open = false;
            self.design.checks_pending = None;
        }
    }

    pub fn open_checks(&mut self) {
        self.design.checks = self.check_job();
        self.design.checks_pending = None;
        self.design.checks_open = true;
    }
}
