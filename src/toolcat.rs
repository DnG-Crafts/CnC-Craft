//! A list of common router bits (metric and imperial) to pick from, so the tool library holds
//! only the bits the person actually has.

use crate::app::App;
use crate::model::*;
use egui::{Color32, RichText};

pub struct CatTool {
    pub group: &'static str,
    pub imperial: bool,
    pub tool: Tool,
}

pub const GROUPS: [&str; 10] = [
    "Flat end mills",
    "Down-cut and compression",
    "Single flute (plastics, aluminium)",
    "Ball nose",
    "Tapered ball nose (3D detail)",
    "V-bits",
    "Drills",
    "Surfacing (spoilboard)",
    "Specialty",
    "Drag knives and thread mills",
];

fn frac(inch: f64) -> String {
    // the fraction for an inch size (1/32 steps), or a decimal
    for den in [2u32, 4, 8, 16, 32, 64] {
        let num = inch * den as f64;
        if (num - num.round()).abs() < 1e-6 {
            let n = num.round() as u32;
            let whole = n / den;
            let rest = n % den;
            return match (whole, rest) {
                (0, r) => format!("{r}/{den}\""),
                (w, 0) => format!("{w}\""),
                (w, r) => format!("{w}-{r}/{den}\""),
            };
        }
    }
    format!("{inch:.3}\"")
}

fn mm(v: f64) -> String {
    let s = format!("{v:.2}");
    format!("{} mm", s.trim_end_matches('0').trim_end_matches('.'))
}

#[allow(clippy::too_many_arguments)]
fn t(group: &'static str, imperial: bool, name: String, kind: ToolKind, dia: f64, angle: f64, flutes: u32, flute_len: f64) -> CatTool {
    CatTool {
        group,
        imperial,
        tool: Tool { id: 0, number: 0, name, kind, diameter: dia, angle, flutes, stepdown: dia * 0.5, stepover_pct: 40.0, feed: 1000.0, plunge: 300.0, rpm: 18000.0, flute_length: flute_len },
    }
}

/// Every bit in the list.
pub fn catalogue() -> Vec<CatTool> {
    let mut v = vec![];
    let fl = |d: f64| (d * 3.5).clamp(2.0, 38.0);
    // flat end mills
    for i in [1.0 / 32.0, 1.0 / 16.0, 3.0 / 32.0, 1.0 / 8.0, 5.0 / 32.0, 3.0 / 16.0, 1.0 / 4.0, 5.0 / 16.0, 3.0 / 8.0, 1.0 / 2.0, 3.0 / 4.0] {
        let d = i * 25.4;
        v.push(t(GROUPS[0], true, format!("{} Flat end mill, 2 flute up-cut", frac(i)), ToolKind::FlatEnd, d, 0.0, 2, fl(d)));
    }
    for d in [0.5f64, 0.8, 1.0, 1.5, 2.0, 2.5, 3.0, 3.175, 4.0, 5.0, 6.0, 6.35, 8.0, 10.0, 12.0, 16.0] {
        let name = if (d - 3.175).abs() < 1e-9 || (d - 6.35).abs() < 1e-9 { format!("{} Flat end mill, 2 flute up-cut ({} shank size)", mm(d), if d < 4.0 { "1/8\"" } else { "1/4\"" }) } else { format!("{} Flat end mill, 2 flute up-cut", mm(d)) };
        v.push(t(GROUPS[0], false, name, ToolKind::FlatEnd, d, 0.0, 2, fl(d)));
    }
    for d in [3.0, 4.0, 6.0, 8.0] {
        v.push(t(GROUPS[0], false, format!("{} Flat end mill, 4 flute", mm(d)), ToolKind::FlatEnd, d, 0.0, 4, fl(d)));
    }
    // down-cut and compression
    for i in [1.0 / 16.0, 1.0 / 8.0, 3.0 / 16.0, 1.0 / 4.0, 3.0 / 8.0, 1.0 / 2.0] {
        let d = i * 25.4;
        v.push(t(GROUPS[1], true, format!("{} Down-cut, 2 flute", frac(i)), ToolKind::FlatEnd, d, 0.0, 2, fl(d)));
    }
    for i in [1.0 / 8.0, 1.0 / 4.0, 3.0 / 8.0, 1.0 / 2.0] {
        let d = i * 25.4;
        v.push(t(GROUPS[1], true, format!("{} Compression, 2 flute", frac(i)), ToolKind::FlatEnd, d, 0.0, 2, fl(d)));
    }
    for d in [2.0, 3.0, 4.0, 6.0, 8.0] {
        v.push(t(GROUPS[1], false, format!("{} Down-cut, 2 flute", mm(d)), ToolKind::FlatEnd, d, 0.0, 2, fl(d)));
    }
    for d in [3.0, 6.0, 8.0, 12.0] {
        v.push(t(GROUPS[1], false, format!("{} Compression, 2 flute", mm(d)), ToolKind::FlatEnd, d, 0.0, 2, fl(d)));
    }
    // single flute
    for i in [1.0 / 16.0, 1.0 / 8.0, 3.0 / 16.0, 1.0 / 4.0] {
        let d = i * 25.4;
        v.push(t(GROUPS[2], true, format!("{} Single flute (O-flute)", frac(i)), ToolKind::FlatEnd, d, 0.0, 1, fl(d)));
    }
    for d in [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 8.0] {
        v.push(t(GROUPS[2], false, format!("{} Single flute (O-flute)", mm(d)), ToolKind::FlatEnd, d, 0.0, 1, fl(d)));
    }
    // ball nose
    for i in [1.0 / 32.0, 1.0 / 16.0, 1.0 / 8.0, 3.0 / 16.0, 1.0 / 4.0, 3.0 / 8.0, 1.0 / 2.0] {
        let d = i * 25.4;
        v.push(t(GROUPS[3], true, format!("{} Ball nose, 2 flute", frac(i)), ToolKind::BallNose, d, 0.0, 2, fl(d)));
    }
    for d in [0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0, 12.0] {
        v.push(t(GROUPS[3], false, format!("{} Ball nose, 2 flute", mm(d)), ToolKind::BallNose, d, 0.0, 2, fl(d)));
    }
    // tapered ball nose: the size is the ball at the tip
    for (tip, ang) in [(0.25, 10.0), (0.5, 10.0), (0.75, 10.0), (1.0, 10.0), (1.5, 15.0), (0.5, 15.0)] {
        v.push(t(GROUPS[4], false, format!("{} tip Tapered ball nose ({ang:.0}° taper, 1/4\" shank)", mm(tip)), ToolKind::BallNose, tip, 0.0, 2, 25.0));
    }
    for (tip_in, label) in [(1.0 / 64.0, "1/64\""), (1.0 / 32.0, "1/32\""), (1.0 / 16.0, "1/16\"")] {
        v.push(t(GROUPS[4], true, format!("{label} tip Tapered ball nose (1/4\" shank)"), ToolKind::BallNose, tip_in * 25.4, 0.0, 2, 25.0));
    }
    // V-bits: the size is the widest cut
    for (i, angs) in [(1.0 / 4.0, &[30.0, 60.0, 90.0][..]), (1.0 / 2.0, &[60.0, 90.0, 120.0, 150.0][..]), (1.0, &[60.0, 90.0][..])] {
        for &a in angs {
            let d = i * 25.4;
            v.push(t(GROUPS[5], true, format!("{a:.0}° V-bit, {} wide", frac(i)), ToolKind::VBit, d, a, 2, d / 2.0 / (a / 2.0f64).to_radians().tan()));
        }
    }
    for (d, angs) in [(3.175, &[20.0, 30.0, 60.0][..]), (6.0, &[30.0, 45.0, 60.0, 90.0][..]), (12.0, &[60.0, 90.0, 120.0][..]), (22.0, &[90.0][..])] {
        for &a in angs {
            let name = if d < 4.0 { format!("{a:.0}° Engraving V-bit, {} wide", mm(d)) } else { format!("{a:.0}° V-bit, {} wide", mm(d)) };
            v.push(t(GROUPS[5], false, name, ToolKind::VBit, d, a, if d < 4.0 { 1 } else { 2 }, d / 2.0 / (a / 2.0f64).to_radians().tan()));
        }
    }
    // drills
    for n in [2u32, 3, 4, 5, 6, 7, 8, 9, 10, 12, 14, 16] {
        let i = n as f64 / 32.0;
        v.push(t(GROUPS[6], true, format!("{} Drill bit", frac(i)), ToolKind::Drill, i * 25.4, 118.0, 2, (i * 25.4 * 6.0).min(60.0)));
    }
    for d in [1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0, 5.5, 6.0, 6.5, 7.0, 8.0, 8.5, 10.0, 10.2, 12.0] {
        v.push(t(GROUPS[6], false, format!("{} Drill bit", mm(d)), ToolKind::Drill, d, 118.0, 2, (d * 6.0).min(60.0)));
    }
    // surfacing
    for i in [1.0, 1.5, 2.0, 2.5] {
        v.push(t(GROUPS[7], true, format!("{} Surfacing bit (1/4\" or 1/2\" shank)", frac(i)), ToolKind::FlatEnd, i * 25.4, 0.0, 3, 6.0));
    }
    for d in [22.0, 25.0, 32.0, 38.0, 50.0] {
        v.push(t(GROUPS[7], false, format!("{} Surfacing bit", mm(d)), ToolKind::FlatEnd, d, 0.0, 3, 6.0));
    }
    // specialty
    v.push(t(GROUPS[8], true, "3/8\" Keyhole bit (for hanging slots)".into(), ToolKind::FlatEnd, 9.525, 0.0, 2, 5.0));
    v.push(t(GROUPS[8], false, "10 mm Keyhole bit (for hanging slots)".into(), ToolKind::FlatEnd, 10.0, 0.0, 2, 5.0));
    for i in [1.0 / 4.0, 1.0 / 2.0] {
        v.push(t(GROUPS[8], true, format!("{} Core box (round groove)", frac(i)), ToolKind::BallNose, i * 25.4, 0.0, 2, i * 25.4));
    }
    v.push(t(GROUPS[8], true, "1/2\" 45° Chamfer bit".into(), ToolKind::VBit, 12.7, 90.0, 2, 6.35));
    v.push(t(GROUPS[8], false, "12 mm 45° Chamfer bit".into(), ToolKind::VBit, 12.0, 90.0, 2, 6.0));
    v.push(t(GROUPS[8], false, "6 mm Corn cob (roughing)".into(), ToolKind::FlatEnd, 6.0, 0.0, 6, 20.0));
    v.push(t(GROUPS[8], true, "1/8\" Corn cob (PCB / roughing)".into(), ToolKind::FlatEnd, 3.175, 0.0, 6, 12.0));
    // drag knives and thread mills
    for (a, off) in [(30.0, 0.25), (45.0, 0.25), (60.0, 0.5)] {
        let mut c = t(GROUPS[9], false, format!("Drag knife, {a:.0}° blade ({} offset)", mm(off)), ToolKind::DragKnife, off * 2.0, a, 1, 0.0);
        c.tool.rpm = 0.0;
        v.push(c);
    }
    for (d, len) in [(3.0, 8.0), (4.0, 10.0), (6.0, 15.0), (10.0, 25.0)] {
        v.push(t(GROUPS[9], false, format!("{} Thread mill, single point 60°", mm(d)), ToolKind::ThreadMill, d, 60.0, 3, len));
    }
    v.push(t(GROUPS[9], true, "1/4\" Thread mill, single point 60°".into(), ToolKind::ThreadMill, 6.35, 60.0, 3, 15.0));
    v
}

/// Is a bit like this already in the library?
pub fn in_library(lib: &[Tool], t: &Tool) -> bool {
    lib.iter().any(|l| l.kind == t.kind && (l.diameter - t.diameter).abs() < 0.01 && (!matches!(t.kind, ToolKind::VBit | ToolKind::DragKnife) || (l.angle - t.angle).abs() < 0.5) && l.flutes == t.flutes)
}

impl App {
    pub fn catalog_window(&mut self, ctx: &egui::Context) {
        if !self.design.catalog_open {
            return;
        }
        let cat = catalogue();
        let mut open = true;
        let mut add = false;
        egui::Window::new("Add tools from the list").open(&mut open).default_width(520.0).default_height(520.0).collapsible(false).show(ctx, |ui| {
            ui.label("Tick the bits you have, then add them to your tool library.");
            let d = &mut self.design;
            ui.horizontal(|ui| {
                ui.radio_value(&mut d.catalog_units, 0, "All");
                ui.radio_value(&mut d.catalog_units, 1, "Metric");
                ui.radio_value(&mut d.catalog_units, 2, "Imperial");
                ui.separator();
                egui::ComboBox::from_id_salt("catgroup").selected_text(if d.catalog_group == 0 { "Every kind" } else { GROUPS[d.catalog_group - 1] }).width(190.0).show_ui(ui, |ui| {
                    ui.selectable_value(&mut d.catalog_group, 0, "Every kind");
                    for (i, g) in GROUPS.iter().enumerate() {
                        ui.selectable_value(&mut d.catalog_group, i + 1, *g);
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.label("Search");
                ui.add(egui::TextEdit::singleline(&mut d.catalog_search).hint_text("e.g. 1/4, 6 mm, V-bit, down-cut").desired_width(260.0));
            });
            let q = d.catalog_search.to_lowercase();
            let shown: Vec<usize> = cat
                .iter()
                .enumerate()
                .filter(|(_, c)| match d.catalog_units {
                    1 => !c.imperial,
                    2 => c.imperial,
                    _ => true,
                })
                .filter(|(_, c)| d.catalog_group == 0 || GROUPS[d.catalog_group - 1] == c.group)
                .filter(|(_, c)| q.is_empty() || q.split_whitespace().all(|w| c.tool.name.to_lowercase().contains(w) || c.group.to_lowercase().contains(w)))
                .map(|(i, _)| i)
                .collect();
            ui.separator();
            egui::ScrollArea::vertical().max_height(360.0).auto_shrink([false, false]).show(ui, |ui| {
                let mut last_group = "";
                for &i in &shown {
                    let c = &cat[i];
                    if c.group != last_group {
                        ui.add_space(4.0);
                        ui.label(RichText::new(c.group).strong().color(crate::app::ACCENT));
                        last_group = c.group;
                    }
                    let have = in_library(&self.settings.tools, &c.tool);
                    ui.horizontal(|ui| {
                        if have {
                            let mut on = true;
                            ui.add_enabled(false, egui::Checkbox::new(&mut on, &c.tool.name));
                            ui.label(RichText::new("in your library").small().color(Color32::from_rgb(90, 200, 110)));
                        } else {
                            let mut on = self.design.catalog_pick.contains(&i);
                            if ui.checkbox(&mut on, &c.tool.name).changed() {
                                if on {
                                    self.design.catalog_pick.insert(i);
                                } else {
                                    self.design.catalog_pick.remove(&i);
                                }
                            }
                        }
                    });
                }
                if shown.is_empty() {
                    ui.label(RichText::new("Nothing matches.").weak());
                }
            });
            ui.separator();
            let n = self.design.catalog_pick.iter().filter(|i| cat.get(**i).map_or(false, |c| !in_library(&self.settings.tools, &c.tool))).count();
            ui.horizontal(|ui| {
                if ui.add_enabled(n > 0, egui::Button::new(RichText::new(format!("Add {n} tool{}", if n == 1 { "" } else { "s" })).strong())).clicked() {
                    add = true;
                }
                if ui.add_enabled(n > 0, egui::Button::new("Clear ticks")).clicked() {
                    self.design.catalog_pick.clear();
                }
            });
            ui.label(RichText::new(format!("Speeds are set for {} on a {} machine; change them with Feeds & speeds… in the tool library.", crate::jobtools::MATERIALS[self.design.feeds_material].0.to_lowercase(), crate::jobtools::RIGIDITY[self.design.feeds_rigidity].0.split(' ').next().unwrap_or("medium").to_lowercase())).small());
        });
        if add {
            let mut picks: Vec<usize> = self.design.catalog_pick.drain().collect();
            picks.sort();
            let m = &self.settings.machine;
            let (mr, mf, mat, rig) = (m.max_rpm, m.max_feed, self.design.feeds_material, self.design.feeds_rigidity);
            let mut added = 0;
            for i in picks {
                let Some(c) = cat.get(i) else { continue };
                if in_library(&self.settings.tools, &c.tool) {
                    continue;
                }
                let mut tool = c.tool.clone();
                tool.id = self.settings.tools.iter().map(|t| t.id).max().unwrap_or(0) + 1;
                tool.number = self.settings.tools.iter().map(|t| t.number).max().unwrap_or(0) + 1;
                if tool.kind != ToolKind::DragKnife {
                    let s = crate::jobtools::suggest(&tool, mat, rig, mr, mf);
                    tool.rpm = s.rpm;
                    tool.feed = s.feed;
                    tool.plunge = s.plunge;
                    tool.stepdown = s.stepdown;
                    tool.stepover_pct = s.stepover_pct;
                } else {
                    tool.feed = 1500.0;
                    tool.plunge = 300.0;
                }
                self.selected_tool = Some(tool.id);
                self.design.tool_scroll_to = Some(tool.id);
                self.settings.tools.push(tool);
                added += 1;
            }
            self.status = format!("Added {added} tool{} to the library", if added == 1 { "" } else { "s" });
        }
        if !open {
            self.design.catalog_open = false;
        }
    }
}
