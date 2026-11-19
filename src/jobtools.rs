//! Feeds and speeds calculator, and resuming a job part-way through.

use crate::app::{App, ACCENT};
use crate::machine::Cmd;
use crate::model::*;
use egui::{Color32, RichText};

/// Materials: name, chip load (mm per tooth) for 3 / 6 / 12 mm bits, best spindle speed,
/// depth per pass as a share of the bit's width.
pub const MATERIALS: [(&str, [f64; 3], f64, f64); 9] = [
    ("Softwood (pine, cedar)", [0.08, 0.20, 0.35], 18000.0, 1.0),
    ("Hardwood (oak, maple, walnut)", [0.06, 0.17, 0.30], 18000.0, 0.5),
    ("MDF", [0.09, 0.22, 0.38], 18000.0, 1.0),
    ("Plywood", [0.07, 0.19, 0.33], 18000.0, 0.75),
    ("Acrylic (cast)", [0.05, 0.13, 0.25], 16000.0, 0.5),
    ("HDPE / plastics", [0.08, 0.20, 0.35], 14000.0, 0.75),
    ("Foam / soft board", [0.15, 0.35, 0.50], 14000.0, 2.0),
    ("Aluminium", [0.013, 0.05, 0.10], 12000.0, 0.15),
    ("Brass", [0.01, 0.04, 0.08], 12000.0, 0.1),
];

/// How stiff the machine is: name and how hard to push it.
pub const RIGIDITY: [(&str, f64); 3] = [("Light (small desktop, 3018-type)", 0.5), ("Medium (hobby router)", 0.75), ("Heavy (steel or big rails)", 1.0)];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Suggestion {
    pub rpm: f64,
    pub feed: f64,
    pub plunge: f64,
    pub stepdown: f64,
    pub stepover_pct: f64,
    pub chip_load: f64,
}

/// Suggested settings for a tool in a material on a machine.
pub fn suggest(tool: &Tool, material: usize, rigidity: usize, max_rpm: f64, max_feed: f64) -> Suggestion {
    let (_, cl, best_rpm, depth_k) = MATERIALS[material.min(MATERIALS.len() - 1)];
    let rig = RIGIDITY[rigidity.min(RIGIDITY.len() - 1)].1;
    // the cutting width that matters: V-bits and ball noses cut narrower than their size at shallow depths
    let d = match tool.kind {
        ToolKind::VBit | ToolKind::BallNose => tool.diameter * 0.6,
        _ => tool.diameter,
    }
    .max(0.2);
    let chip = if d <= 3.0 {
        cl[0] * d / 3.0
    } else if d <= 6.0 {
        cl[0] + (cl[1] - cl[0]) * (d - 3.0) / 3.0
    } else if d <= 12.0 {
        cl[1] + (cl[2] - cl[1]) * (d - 6.0) / 6.0
    } else {
        cl[2]
    } * (0.6 + 0.4 * rig);
    let flutes = tool.flutes.max(1) as f64;
    let mut rpm = best_rpm.min(max_rpm.max(1000.0));
    let mut feed = rpm * flutes * chip;
    let cap = max_feed.max(100.0) * (0.5 + 0.5 * rig);
    if feed > cap {
        // the machine can't go that fast: slow the spindle to keep the chips the right size
        feed = cap;
        rpm = (feed / (flutes * chip)).clamp(8000.0_f64.min(rpm), rpm);
    }
    let plunge = feed * if tool.kind == ToolKind::Drill { 0.2 } else { 0.35 };
    let stepdown = (tool.diameter * depth_k * rig).clamp(0.1, tool.diameter * 2.0);
    let stepover_pct = match tool.kind {
        ToolKind::BallNose => 10.0,
        ToolKind::VBit => 25.0,
        _ => 40.0,
    };
    Suggestion { rpm: rpm.round(), feed: feed.round(), plunge: plunge.round(), stepdown: (stepdown * 10.0).round() / 10.0, stepover_pct, chip_load: chip }
}

impl App {
    pub fn feeds_window(&mut self, ctx: &egui::Context) {
        if !self.design.feeds_open {
            return;
        }
        let mut open = true;
        let mut apply: Option<(Suggestion, bool)> = None;
        egui::Window::new("Feeds and speeds").open(&mut open).default_width(400.0).collapsible(false).show(ctx, |ui| {
            let Some(tool) = self.selected_tool.and_then(|id| self.settings.tools.iter().find(|t| t.id == id)).cloned() else {
                ui.label("Select a tool in the tool library first.");
                return;
            };
            ui.label(RichText::new(format!("T{} {}", tool.number, tool.name)).strong());
            if matches!(tool.kind, ToolKind::DragKnife) {
                ui.label("A drag knife doesn't spin; run it at about 1500 mm/min.");
                return;
            }
            egui::Grid::new("fsgrid").num_columns(2).spacing([10.0, 5.0]).show(ui, |ui| {
                ui.label("Material");
                egui::ComboBox::from_id_salt("fsmat").selected_text(MATERIALS[self.design.feeds_material].0).width(230.0).show_ui(ui, |ui| {
                    for (i, m) in MATERIALS.iter().enumerate() {
                        ui.selectable_value(&mut self.design.feeds_material, i, m.0);
                    }
                });
                ui.end_row();
                ui.label("Machine");
                egui::ComboBox::from_id_salt("fsrig").selected_text(RIGIDITY[self.design.feeds_rigidity].0).width(230.0).show_ui(ui, |ui| {
                    for (i, m) in RIGIDITY.iter().enumerate() {
                        ui.selectable_value(&mut self.design.feeds_rigidity, i, m.0);
                    }
                });
                ui.end_row();
            });
            let m = &self.settings.machine;
            let s = suggest(&tool, self.design.feeds_material, self.design.feeds_rigidity, m.max_rpm, m.max_feed);
            ui.separator();
            egui::Grid::new("fsres").num_columns(3).spacing([14.0, 4.0]).show(ui, |ui| {
                ui.label("");
                ui.label(RichText::new("Suggested").color(ACCENT));
                ui.label(RichText::new("Now").weak());
                ui.end_row();
                let row = |ui: &mut egui::Ui, name: &str, a: String, b: String| {
                    ui.label(name);
                    ui.label(RichText::new(a).strong());
                    ui.label(RichText::new(b).weak());
                    ui.end_row();
                };
                row(ui, "Spindle speed", format!("{:.0} rpm", s.rpm), format!("{:.0} rpm", tool.rpm));
                row(ui, "Feed rate", format!("{:.0} mm/min", s.feed), format!("{:.0} mm/min", tool.feed));
                row(ui, "Plunge rate", format!("{:.0} mm/min", s.plunge), format!("{:.0} mm/min", tool.plunge));
                row(ui, "Depth per pass", format!("{:.1} mm", s.stepdown), format!("{:.1} mm", tool.stepdown));
                row(ui, "Stepover", format!("{:.0} %", s.stepover_pct), format!("{:.0} %", tool.stepover_pct));
            });
            ui.label(RichText::new(format!("Chip load {:.3} mm per tooth with {} flute(s). Start here and listen to the cut: squealing or fine dust means go faster or slow the spindle; chatter means take less depth.", s.chip_load, tool.flutes.max(1))).small());
            ui.label(RichText::new(format!("Uses your machine's limits: {:.0} rpm, {:.0} mm/min (Machine settings).", m.max_rpm, m.max_feed)).small().weak());
            ui.horizontal(|ui| {
                if ui.button("Apply to this tool").clicked() {
                    apply = Some((s, false));
                }
                if ui.button("Apply to the tool and its toolpaths").clicked() {
                    apply = Some((s, true));
                }
            });
        });
        if let (Some((s, tps)), Some(id)) = (apply, self.selected_tool) {
            if let Some(t) = self.settings.tools.iter_mut().find(|t| t.id == id) {
                t.rpm = s.rpm;
                t.feed = s.feed;
                t.plunge = s.plunge;
                t.stepdown = s.stepdown;
                t.stepover_pct = s.stepover_pct;
            }
            if tps {
                self.checkpoint();
                for tp in self.doc.toolpaths.iter_mut().filter(|tp| tp.tool_id == id) {
                    tp.rpm = s.rpm;
                    tp.feed = s.feed;
                    tp.plunge = s.plunge;
                    tp.stepdown = s.stepdown;
                    tp.stepover_pct = s.stepover_pct;
                }
                self.job_dirty = true;
            }
            self.status = "Feeds and speeds applied".into();
        }
        if !open {
            self.design.feeds_open = false;
        }
    }

    pub fn resume_window(&mut self, ctx: &egui::Context) {
        if !self.design.resume_open {
            return;
        }
        if self.job_dirty {
            self.regenerate();
        }
        let mut open = true;
        let mut go = false;
        let mut save = false;
        let n = self.job.gcode.len();
        egui::Window::new("Resume a job").open(&mut open).default_width(460.0).collapsible(false).show(ctx, |ui| {
            if n == 0 {
                ui.label("There is no job yet - add toolpaths first.");
                return;
            }
            ui.label("Pick up a job part-way through, after a broken bit, a stop or a power cut. Home the machine and set the work zero exactly as before (and the new bit's Z) first.");
            let starts: Vec<(usize, String)> = self
                .job
                .gcode
                .iter()
                .enumerate()
                .filter_map(|(i, l)| {
                    let t = l.trim_start_matches(['(', ';', ' ']);
                    t.strip_prefix("--- ").map(|r| (i, r.trim_end_matches([')', ' ', '-']).to_string()))
                })
                .collect();
            ui.horizontal(|ui| {
                ui.label("Start of a toolpath");
                egui::ComboBox::from_id_salt("resumetp").selected_text("Choose…").width(260.0).show_ui(ui, |ui| {
                    for (i, name) in &starts {
                        if ui.selectable_label(false, format!("{name}  (line {})", i + 1)).clicked() {
                            self.design.resume_line = i + 1;
                        }
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.label("Or line");
                let mut line = self.design.resume_line.max(1);
                ui.add(egui::DragValue::new(&mut line).range(1..=n).speed(1.0));
                ui.add(egui::Slider::new(&mut line, 1..=n).show_value(false));
                self.design.resume_line = line;
            });
            if let Some((a, _)) = self.job_progress {
                if a > 0 && ui.small_button(format!("Where the last job stopped (line {a})")).clicked() {
                    self.design.resume_line = a.clamp(1, n);
                }
            }
            let from = self.design.resume_line.clamp(1, n) - 1;
            egui::Frame::default().fill(Color32::from_rgb(30, 34, 40)).inner_margin(6.0).show(ui, |ui| {
                for i in from.saturating_sub(2)..(from + 3).min(n) {
                    let t = format!("{:>6}  {}", i + 1, self.job.gcode[i]);
                    ui.label(if i == from { RichText::new(t).monospace().color(ACCENT) } else { RichText::new(t).monospace().weak() });
                }
            });
            let (_, _, pos, _, _, _) = crate::cam::state_before(&self.job.gcode, from);
            if let (Some(x), Some(y), Some(z)) = (pos[0], pos[1], pos[2]) {
                ui.label(format!("The bit goes to X {x:.2} Y {y:.2} at safe height, starts the spindle and lowers slowly to Z {z:.2}."));
            }
            ui.horizontal(|ui| {
                if ui.add_enabled(self.connected.is_some() && !self.job_running, egui::Button::new(RichText::new("▶ Start from here").strong()).fill(Color32::from_rgb(40, 140, 70))).on_disabled_hover_text("Connect to the machine first").clicked() {
                    go = true;
                }
                if ui.button("Save as G-code…").clicked() {
                    save = true;
                }
            });
        });
        if go || save {
            let m = &self.settings.machine;
            let safe = m.safe_z.max(1.0) + self.doc.stock.top_z();
            let from = self.design.resume_line.clamp(1, n.max(1)) - 1;
            let prog = crate::cam::adapt_dialect(crate::cam::resume_gcode(&self.job.gcode, from, safe, m.spindle_delay), m.dialect);
            if go {
                let len = prog.len();
                self.machine.send(Cmd::Job(prog));
                self.job_running = true;
                self.paused = false;
                self.job_progress = Some((0, len));
                self.design.resume_open = false;
                self.status = format!("Job resumed from line {}", from + 1);
            } else if let Some(p) = rfd::FileDialog::new().add_filter("G-code", &["nc", "gcode", "tap", "ngc"]).set_file_name("job_resume.nc").save_file() {
                self.status = match std::fs::write(&p, prog.join("\n") + "\n") {
                    Ok(()) => format!("Resumed G-code saved to {}", p.display()),
                    Err(e) => format!("Could not save: {e}"),
                };
            }
        }
        if !open {
            self.design.resume_open = false;
        }
    }
}
