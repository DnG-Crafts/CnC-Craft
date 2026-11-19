//! Window layout: icon toolbar, two-column tool palette, grouped number bar and the right panel
//! (with the Arrange object list, Move and History panels).

use crate::app::{App, CanvasTool, RightTab, ACCENT};
use crate::design_ui::tone;
use crate::geom::BoolOp;
use crate::model::*;
use crate::panels::{len_edit, paint_tool_icon, section};
use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, Ui};

pub const HEADER_BLUE: Color32 = Color32::from_rgb(16, 82, 122);

fn tb_icon(p: &egui::Painter, r: Rect, kind: &str, col: Color32) {
    let c = r.center();
    let s = Stroke::new(1.5f32, col);
    let pt = |x: f32, y: f32| Pos2::new(c.x + x, c.y + y);
    let line = |a: Pos2, b: Pos2| {
        p.line_segment([a, b], s);
    };
    let poly = |v: Vec<Pos2>| {
        p.add(egui::Shape::line(v, s));
    };
    let rect = |x0: f32, y0: f32, x1: f32, y1: f32| {
        p.rect_stroke(Rect::from_min_max(pt(x0, y0), pt(x1, y1)), 1.0, s, egui::StrokeKind::Middle);
    };
    match kind {
        "new" => {
            poly(vec![pt(-6.0, -9.0), pt(3.0, -9.0), pt(7.0, -5.0), pt(7.0, 9.0), pt(-6.0, 9.0), pt(-6.0, -9.0)]);
            line(pt(-2.0, 1.0), pt(4.0, 1.0));
            line(pt(1.0, -2.0), pt(1.0, 4.0));
        }
        "open" => {
            poly(vec![pt(-9.0, 7.0), pt(-9.0, -7.0), pt(-3.0, -7.0), pt(-1.0, -4.0), pt(7.0, -4.0), pt(7.0, -1.0)]);
            poly(vec![pt(-9.0, 7.0), pt(-5.0, -1.0), pt(10.0, -1.0), pt(6.0, 7.0), pt(-9.0, 7.0)]);
        }
        "save" => {
            rect(-8.0, -8.0, 8.0, 8.0);
            rect(-4.0, -8.0, 4.0, -3.0);
            rect(-5.0, 2.0, 5.0, 8.0);
        }
        "import" | "export" => {
            poly(vec![pt(-9.0, 3.0), pt(-9.0, 9.0), pt(9.0, 9.0), pt(9.0, 3.0)]);
            line(pt(0.0, -9.0), pt(0.0, 5.0));
            if kind == "import" {
                poly(vec![pt(-4.0, 1.0), pt(0.0, 5.0), pt(4.0, 1.0)]);
            } else {
                poly(vec![pt(-4.0, -5.0), pt(0.0, -9.0), pt(4.0, -5.0)]);
            }
        }
        "undo" | "redo" => {
            let m = if kind == "undo" { 1.0 } else { -1.0 };
            let pts: Vec<Pos2> = (0..=12)
                .map(|i| {
                    let a = std::f32::consts::PI * (1.1 - i as f32 / 12.0 * 1.2);
                    pt(m * 6.0 * a.cos(), 2.0 - 6.0 * a.sin())
                })
                .collect();
            let start = pts[0];
            poly(pts);
            line(start, start + egui::vec2(m * 4.0, 1.0));
            line(start, start + egui::vec2(m * 1.0, -4.0));
        }
        "cut" => {
            p.circle_stroke(pt(-4.0, 6.0), 3.0, s);
            p.circle_stroke(pt(4.0, 6.0), 3.0, s);
            line(pt(-2.0, 4.0), pt(5.0, -9.0));
            line(pt(2.0, 4.0), pt(-5.0, -9.0));
        }
        "copy" => {
            rect(-8.0, -8.0, 3.0, 4.0);
            rect(-3.0, -3.0, 8.0, 9.0);
        }
        "paste" => {
            rect(-7.0, -7.0, 7.0, 9.0);
            rect(-3.0, -9.0, 3.0, -5.0);
        }
        "delete" => {
            poly(vec![pt(-6.0, -5.0), pt(-5.0, 9.0), pt(5.0, 9.0), pt(6.0, -5.0)]);
            line(pt(-8.0, -6.0), pt(8.0, -6.0));
            rect(-3.0, -9.0, 3.0, -6.0);
        }
        "fit" => {
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                poly(vec![pt(sx * 9.0, sy * 4.0), pt(sx * 9.0, sy * 9.0), pt(sx * 4.0, sy * 9.0)]);
            }
            rect(-4.0, -4.0, 4.0, 4.0);
        }
        "zoomin" | "zoomout" | "zoomsel" => {
            p.circle_stroke(pt(-2.0, -2.0), 6.0, s);
            line(pt(2.5, 2.5), pt(8.0, 8.0));
            line(pt(-5.0, -2.0), pt(1.0, -2.0));
            if kind == "zoomin" {
                line(pt(-2.0, -5.0), pt(-2.0, 1.0));
            }
            if kind == "zoomsel" {
                p.rect_stroke(Rect::from_center_size(pt(-2.0, -2.0), egui::vec2(5.0, 5.0)), 0.0, s, egui::StrokeKind::Middle);
            }
        }
        "preview" => {
            poly(vec![pt(0.0, -9.0), pt(8.0, -5.0), pt(8.0, 5.0), pt(0.0, 9.0), pt(-8.0, 5.0), pt(-8.0, -5.0), pt(0.0, -9.0)]);
            poly(vec![pt(-8.0, -5.0), pt(0.0, -1.0), pt(8.0, -5.0)]);
            line(pt(0.0, -1.0), pt(0.0, 9.0));
        }
        "gcode" => {
            for (i, y) in [-6.0, -2.0, 2.0, 6.0].iter().enumerate() {
                p.circle_filled(pt(-7.0, *y), 1.2, col);
                line(pt(-4.0, *y), pt(if i % 2 == 0 { 8.0 } else { 4.0 }, *y));
            }
        }
        "settings" => {
            p.circle_stroke(c, 5.0, s);
            p.circle_stroke(c, 2.0, s);
            for i in 0..8 {
                let a = std::f32::consts::TAU * i as f32 / 8.0;
                p.line_segment([pt(5.0 * a.cos(), 5.0 * a.sin()), pt(8.5 * a.cos(), 8.5 * a.sin())], Stroke::new(2.4f32, col));
            }
        }
        "machine" => {
            p.circle_stroke(c, 7.0, s);
            line(pt(-10.0, 0.0), pt(10.0, 0.0));
            line(pt(0.0, -10.0), pt(0.0, 10.0));
        }
        "group" | "ungroup" => {
            rect(-8.0, -8.0, 1.0, 1.0);
            rect(-1.0, -1.0, 8.0, 8.0);
            if kind == "group" {
                p.rect_stroke(Rect::from_min_max(pt(-10.0, -10.0), pt(10.0, 10.0)), 0.0, Stroke::new(1.0f32, col.gamma_multiply(0.6)), egui::StrokeKind::Middle);
            }
        }
        "fliph" => {
            line(pt(0.0, -10.0), pt(0.0, 10.0));
            p.add(egui::Shape::closed_line(vec![pt(-2.5, -7.0), pt(-9.0, 6.0), pt(-2.5, 6.0)], s));
            p.add(egui::Shape::convex_polygon(vec![pt(2.5, -7.0), pt(2.5, 6.0), pt(9.0, 6.0)], col.gamma_multiply(0.6), s));
        }
        "flipv" => {
            line(pt(-10.0, 0.0), pt(10.0, 0.0));
            p.add(egui::Shape::closed_line(vec![pt(-7.0, -2.5), pt(6.0, -9.0), pt(6.0, -2.5)], s));
            p.add(egui::Shape::convex_polygon(vec![pt(-7.0, 2.5), pt(6.0, 2.5), pt(6.0, 9.0)], col.gamma_multiply(0.6), s));
        }
        "al_left" | "al_hc" | "al_right" => {
            let x = match kind {
                "al_left" => -9.0,
                "al_hc" => 0.0,
                _ => 9.0,
            };
            line(pt(x, -10.0), pt(x, 10.0));
            let (w1, w2) = (12.0, 7.0);
            let off = |w: f32| match kind {
                "al_left" => (-9.0, -9.0 + w),
                "al_hc" => (-w / 2.0, w / 2.0),
                _ => (9.0 - w, 9.0),
            };
            let (a, b) = off(w1);
            p.rect_filled(Rect::from_min_max(pt(a, -7.0), pt(b, -2.0)), 0.0, col);
            let (a, b) = off(w2);
            p.rect_filled(Rect::from_min_max(pt(a, 2.0), pt(b, 7.0)), 0.0, col);
        }
        "al_top" | "al_vc" | "al_bottom" => {
            let y = match kind {
                "al_top" => -9.0,
                "al_vc" => 0.0,
                _ => 9.0,
            };
            line(pt(-10.0, y), pt(10.0, y));
            let off = |h: f32| match kind {
                "al_top" => (-9.0, -9.0 + h),
                "al_vc" => (-h / 2.0, h / 2.0),
                _ => (9.0 - h, 9.0),
            };
            let (a, b) = off(12.0);
            p.rect_filled(Rect::from_min_max(pt(-7.0, a), pt(-2.0, b)), 0.0, col);
            let (a, b) = off(7.0);
            p.rect_filled(Rect::from_min_max(pt(2.0, a), pt(7.0, b)), 0.0, col);
        }
        "dist_h" => {
            for x in [-8.0, 0.0, 8.0] {
                p.rect_filled(Rect::from_center_size(pt(x, 0.0), egui::vec2(3.0, 14.0)), 0.0, col);
            }
        }
        "dist_v" => {
            for y in [-8.0, 0.0, 8.0] {
                p.rect_filled(Rect::from_center_size(pt(0.0, y), egui::vec2(14.0, 3.0)), 0.0, col);
            }
        }
        // boolean and shape tools for the palette
        "weld" | "subtract" | "intersect" | "exclude" => {
            let a = pt(-3.0, 0.0);
            let b = pt(3.5, 0.0);
            let fill = col.gamma_multiply(0.55);
            match kind {
                "weld" => {
                    p.circle_filled(a, 6.5, fill);
                    p.circle_filled(b, 6.5, fill);
                }
                "subtract" => {
                    p.circle_filled(a, 6.5, fill);
                    p.circle_filled(b, 6.5, tone_bg());
                    p.circle_stroke(b, 6.5, Stroke::new(1.0f32, col.gamma_multiply(0.5)));
                }
                "intersect" => {
                    p.circle_stroke(a, 6.5, Stroke::new(1.0f32, col.gamma_multiply(0.5)));
                    p.circle_stroke(b, 6.5, Stroke::new(1.0f32, col.gamma_multiply(0.5)));
                    for i in -4..=4 {
                        let y = i as f32 * 1.2;
                        let half = ((6.5f32 * 6.5 - y * y).max(0.0)).sqrt();
                        let x0 = b.x - half;
                        let x1 = a.x + half;
                        if x1 > x0 {
                            p.line_segment([Pos2::new(x0, c.y + y), Pos2::new(x1, c.y + y)], Stroke::new(1.3f32, fill));
                        }
                    }
                }
                _ => {
                    p.circle_filled(a, 6.5, fill);
                    p.circle_filled(b, 6.5, fill);
                    for i in -4..=4 {
                        let y = i as f32 * 1.2;
                        let half = ((6.5f32 * 6.5 - y * y).max(0.0)).sqrt();
                        let x0 = b.x - half;
                        let x1 = a.x + half;
                        if x1 > x0 {
                            p.line_segment([Pos2::new(x0, c.y + y), Pos2::new(x1, c.y + y)], Stroke::new(1.3f32, tone_bg()));
                        }
                    }
                }
            }
            p.circle_stroke(a, 6.5, s);
            p.circle_stroke(b, 6.5, s);
        }
        "offset" => {
            rect(-4.0, -4.0, 4.0, 4.0);
            p.rect_stroke(Rect::from_center_size(c, egui::vec2(18.0, 18.0)), 4.0, s, egui::StrokeKind::Middle);
        }
        "array" => {
            for y in [-6.0, 0.0, 6.0] {
                for x in [-6.0, 0.0, 6.0] {
                    p.rect_stroke(Rect::from_center_size(pt(x, y), egui::vec2(4.0, 4.0)), 0.0, s, egui::StrokeKind::Middle);
                }
            }
        }
        "carray" => {
            for i in 0..8 {
                let a = std::f32::consts::TAU * i as f32 / 8.0;
                p.circle_stroke(pt(7.0 * a.cos(), 7.0 * a.sin()), 1.8, s);
            }
        }
        "round" => {
            poly(vec![pt(-8.0, 8.0), pt(-8.0, 0.0)]);
            let pts: Vec<Pos2> = (0..=8).map(|i| {
                let a = std::f32::consts::PI * (1.0 + 0.5 * i as f32 / 8.0);
                pt(0.0 + 8.0 * a.cos(), 0.0 + 8.0 * a.sin())
            }).collect();
            poly(pts);
            line(pt(0.0, -8.0), pt(8.0, -8.0));
        }
        "chamfer" => {
            poly(vec![pt(-8.0, 8.0), pt(-8.0, 0.0), pt(0.0, -8.0), pt(8.0, -8.0)]);
        }
        _ => {
            p.circle_stroke(c, 6.0, s);
        }
    }
}

fn tone_bg() -> Color32 {
    Color32::from_rgb(38, 42, 49)
}

/// An icon-only button. Returns true when clicked.
fn icon_button(ui: &mut Ui, kind: &str, tip: &str, enabled: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(30.0, 28.0), if enabled { Sense::click() } else { Sense::hover() });
    if resp.hovered() && enabled {
        ui.painter().rect_filled(rect, 4.0, tone(ui.ctx(), Color32::from_rgb(66, 74, 86)));
    }
    let col = if enabled { ui.visuals().text_color() } else { ui.visuals().weak_text_color().gamma_multiply(0.6) };
    tb_icon(ui.painter(), rect, kind, col);
    resp.on_hover_text(tip).clicked() && enabled
}

/// An icon button that stays lit while selected (like the tool buttons). Returns true when clicked.
fn toggle_icon(ui: &mut Ui, kind: &str, tip: &str, selected: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(30.0, 28.0), Sense::click());
    let bg = if selected {
        Color32::from_rgb(30, 110, 160)
    } else if resp.hovered() {
        tone(ui.ctx(), Color32::from_rgb(66, 74, 86))
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 4.0, bg);
    if selected {
        ui.painter().rect_stroke(rect, 4.0, Stroke::new(1.0f32, Color32::from_rgb(120, 190, 240)), egui::StrokeKind::Inside);
    }
    tb_icon(ui.painter(), rect, kind, if selected { Color32::WHITE } else { ui.visuals().text_color() });
    resp.on_hover_text(tip).clicked()
}

/// Height of one row of fields on the number bar (every field is exactly this tall).
const ROW_H: f32 = 20.0;
/// Gap between the two rows.
const ROW_GAP: f32 = 2.0;
/// Inside height of every box on the number bar: exactly two rows.
const GROUP_H: f32 = ROW_H * 2.0 + ROW_GAP;
/// Outside height of a box (inside + margins + border).
const GROUP_OUTER_H: f32 = GROUP_H + 8.0;

/// A bordered box on the number bar. Two-row boxes fill it exactly; single-row boxes are
/// centred with fixed padding, so every box has the same top, bottom and middle line.
fn group_frame(ui: &mut Ui, single_row: bool, add: impl FnOnce(&mut Ui)) {
    egui::Frame::default()
        .stroke(Stroke::new(1.0f32, tone(ui.ctx(), Color32::from_rgb(62, 68, 78))))
        .corner_radius(3.0)
        .inner_margin(egui::Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.set_height(GROUP_H);
            // the frame inherits the bar's left-to-right layout, so stack explicitly
            ui.vertical(|ui| {
                ui.set_height(GROUP_H);
                ui.spacing_mut().item_spacing.y = 0.0;
                if single_row {
                    ui.add_space((GROUP_H - ROW_H) / 2.0);
                }
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.y = ROW_GAP;
                    add(ui)
                });
            });
        });
}

/// Compact sizes for the number bar: small text and 20 px rows.
fn compact_style(ui: &mut Ui) {
    let st = ui.style_mut();
    st.spacing.interact_size.y = ROW_H;
    st.spacing.button_padding = egui::vec2(5.0, 1.0);
    st.spacing.item_spacing = egui::vec2(5.0, ROW_GAP);
    for (ts, f) in st.text_styles.iter_mut() {
        if matches!(ts, egui::TextStyle::Body | egui::TextStyle::Button | egui::TextStyle::Monospace) {
            f.size = 12.5;
        }
    }
}

impl App {
    pub fn toolbar(&mut self, ctx: &egui::Context) {
        let frame = egui::Frame::default().fill(tone(ctx, Color32::from_rgb(40, 44, 51))).inner_margin(egui::Margin::symmetric(8, 3));
        egui::TopBottomPanel::top("toolbar").frame(frame).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let has = !self.selection.is_empty();
                let many = self.selection.len() > 1;
                if icon_button(ui, "new", "New project", true) {
                    self.new_project();
                }
                if icon_button(ui, "open", "Open a project or drawing (Ctrl+O)", true) {
                    self.open_dialog();
                }
                if icon_button(ui, "save", "Save the project (Ctrl+S)", true) {
                    self.save(false);
                }
                if icon_button(ui, "import", "Import SVG or DXF (Ctrl+I)", true) {
                    self.import_dialog();
                }
                if icon_button(ui, "export", "Save G-code for your machine (Ctrl+E)", true) {
                    self.save_gcode();
                }
                ui.separator();
                if icon_button(ui, "undo", "Undo (Ctrl+Z)", !self.undo.is_empty()) {
                    self.do_undo();
                }
                if icon_button(ui, "redo", "Redo (Ctrl+Y)", !self.redo.is_empty()) {
                    self.do_redo();
                }
                ui.separator();
                if icon_button(ui, "cut", "Cut (Ctrl+X)", has) {
                    self.copy();
                    self.delete_selected();
                }
                if icon_button(ui, "copy", "Copy (Ctrl+C)", has) {
                    self.copy();
                }
                if icon_button(ui, "paste", "Paste (Ctrl+V)", !self.clipboard.is_empty()) {
                    self.paste([5.0, -5.0]);
                }
                if icon_button(ui, "delete", "Delete (Del)", has) {
                    self.delete_selected();
                }
                ui.separator();
                if icon_button(ui, "fit", "Zoom to the stock", true) {
                    self.needs_fit = true;
                }
                if icon_button(ui, "zoomin", "Zoom in", true) {
                    self.view.zoom = (self.view.zoom * 1.25).min(400.0);
                }
                if icon_button(ui, "zoomout", "Zoom out", true) {
                    self.view.zoom = (self.view.zoom / 1.25).max(0.02);
                }
                if icon_button(ui, "zoomsel", "Zoom to the selection", has) {
                    if let (Some(bb), Some(r)) = (self.selection_bbox(), self.design.canvas_rect) {
                        self.fit_view(r.shrink(20.0), bb);
                    }
                }
                ui.separator();
                if icon_button(ui, "preview", "Cut preview", true) {
                    self.preview_open = true;
                }
                if icon_button(ui, "gcode", "View the G-code", true) {
                    self.gcode_open = true;
                }
                if icon_button(ui, "settings", "Preferences", true) {
                    self.design.prefs_open = true;
                }
                if icon_button(ui, "machine", "Machine settings", true) {
                    self.settings_open = true;
                }
                ui.separator();
                if icon_button(ui, "group", "Group (Ctrl+G)", many) {
                    self.group();
                }
                if icon_button(ui, "ungroup", "Ungroup", has) {
                    self.ungroup();
                }
                ui.separator();
                if icon_button(ui, "fliph", "Flip left-right", has) {
                    self.flip(true);
                }
                if icon_button(ui, "flipv", "Flip top-bottom", has) {
                    self.flip(false);
                }
                ui.separator();
                for (k, a, tip) in [
                    ("al_left", "left", "Align left edges"),
                    ("al_hc", "hcenter", "Align centers left-right"),
                    ("al_right", "right", "Align right edges"),
                    ("al_top", "top", "Align top edges"),
                    ("al_vc", "vcenter", "Align centers top-bottom"),
                    ("al_bottom", "bottom", "Align bottom edges"),
                ] {
                    if icon_button(ui, k, tip, has) {
                        self.align(a);
                    }
                }
                if icon_button(ui, "dist_h", "Space evenly left to right", self.selection.len() > 2) {
                    self.distribute(true);
                }
                if icon_button(ui, "dist_v", "Space evenly top to bottom", self.selection.len() > 2) {
                    self.distribute(false);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let stop = egui::Button::new(RichText::new("STOP").color(Color32::WHITE).strong()).fill(Color32::from_rgb(190, 45, 45));
                    if ui.add(stop).on_hover_text("Stop the machine now").clicked() {
                        self.stop_job();
                        self.realtime(0x18);
                    }
                    if self.connected.is_some() {
                        if ui.button(if self.paused { "Resume" } else { "Pause" }).clicked() {
                            self.realtime(if self.paused { b'~' } else { b'!' });
                            self.paused = !self.paused;
                        }
                        let start = egui::Button::new(RichText::new("Start").color(Color32::WHITE)).fill(Color32::from_rgb(40, 140, 70));
                        if ui.add_enabled(!self.job_running, start).clicked() {
                            self.start_job();
                        }
                        if ui.button("Frame").clicked() {
                            self.frame_job();
                        }
                        ui.label(RichText::new(format!("{}", self.mstate)).color(Color32::from_rgb(90, 210, 110)));
                    }
                });
            });
        });
    }

    pub fn tool_palette(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("palette").exact_width(80.0).resizable(false).frame(egui::Frame::default().fill(ctx.style().visuals.panel_fill).inner_margin(egui::Margin::symmetric(4, 4))).show(ctx, |ui| {
            // fill right up to the canvas (the design view paints over anything past it), so no
            // dark strip is left between the palette and the ruler
            let r = ui.max_rect();
            ui.painter().rect_filled(egui::Rect::from_min_max(egui::pos2(r.left() - 4.0, r.top() - 4.0), egui::pos2(r.right() + 40.0, r.bottom() + 4.0)), 0.0, ui.visuals().panel_fill);
            ui.add_space(6.0);
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
            // the Round-corner tool is picked with the Round/Chamfer buttons at the bottom
            let tools: Vec<CanvasTool> = CanvasTool::ALL.iter().copied().filter(|t| *t != CanvasTool::Corner).collect();
            egui::Grid::new("palette_grid").spacing([4.0, 4.0]).show(ui, |ui| {
                for (i, t) in tools.iter().enumerate() {
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(32.0, 30.0), Sense::click());
                    let active = self.tool == *t;
                    let bg = if active {
                        Color32::from_rgb(30, 110, 160)
                    } else if resp.hovered() {
                        tone(ui.ctx(), Color32::from_rgb(64, 71, 82))
                    } else {
                        Color32::TRANSPARENT
                    };
                    ui.painter().rect_filled(rect, 4.0, bg);
                    let col = if active { Color32::WHITE } else { ui.visuals().text_color() };
                    if *t == CanvasTool::Corner && self.design.chamfer {
                        // the corner tool's icon shows which kind of corner it makes
                        tb_icon(ui.painter(), rect, "chamfer", col);
                    } else {
                        paint_tool_icon(ui.painter(), rect, *t, col);
                    }
                    if resp.on_hover_text(t.label()).clicked() {
                        self.tool = *t;
                        self.line_pts.clear();
                    }
                    if i % 2 == 1 {
                        ui.end_row();
                    }
                }
            });
            ui.separator();
            let has = !self.selection.is_empty();
            let many = self.selection.len() > 1;
            egui::Grid::new("palette_ops").spacing([4.0, 4.0]).show(ui, |ui| {
                if icon_button(ui, "weld", "Weld shapes together", many) {
                    self.boolean(BoolOp::Union);
                }
                if icon_button(ui, "subtract", "Subtract from the first shape", many) {
                    self.boolean(BoolOp::Difference);
                }
                ui.end_row();
                if icon_button(ui, "intersect", "Keep only the overlap", many) {
                    self.boolean(BoolOp::Intersection);
                }
                if icon_button(ui, "exclude", "Keep the parts that don't overlap", many) {
                    self.boolean(BoolOp::Xor);
                }
                ui.end_row();
                if icon_button(ui, "offset", "Offset shape…", has) {
                    self.offset_open = true;
                }
                if icon_button(ui, "array", "Grid array…", has) {
                    self.array_open = true;
                }
                ui.end_row();
                if icon_button(ui, "carray", "Circular array (Arrange panel)", has) {
                    self.right_tab = RightTab::Arrange;
                }
                ui.end_row();
            });
            ui.separator();
            egui::Grid::new("palette_corners").spacing([4.0, 4.0]).show(ui, |ui| {
                let shift = ui.input(|i| i.modifiers.shift);
                let on = |b: bool| if b { "  (on)" } else { "" };
                if toggle_icon(ui, "round", &format!("Round: the corner tool rounds corners{}. Shift-click: round every corner of the selection", on(self.tool == CanvasTool::Corner && !self.design.chamfer)), self.tool == CanvasTool::Corner && !self.design.chamfer) {
                    if shift {
                        self.round_selected(false);
                    } else {
                        self.design.chamfer = false;
                        self.tool = CanvasTool::Corner;
                    }
                }
                if toggle_icon(ui, "chamfer", &format!("Chamfer: the corner tool cuts corners off{}. Shift-click: chamfer every corner of the selection", on(self.tool == CanvasTool::Corner && self.design.chamfer)), self.tool == CanvasTool::Corner && self.design.chamfer) {
                    if shift {
                        self.round_selected(true);
                    } else {
                        self.design.chamfer = true;
                        self.tool = CanvasTool::Corner;
                    }
                }
                ui.end_row();
            });
            let k = crate::panels::unit_k(self.settings.units);
            ui.label(RichText::new(if self.design.chamfer { if k > 1.0 { "Chamfer (in)" } else { "Chamfer (mm)" } } else if k > 1.0 { "Radius (in)" } else { "Radius (mm)" }).size(10.0));
            let mut r = self.design.corner_r / k;
            if ui.add(egui::DragValue::new(&mut r).speed(0.1 / k).range(0.0..=1000.0).max_decimals(2)).changed() {
                self.design.corner_r = r * k;
            }
        });
    }

    pub fn options_bar(&mut self, ctx: &egui::Context) {
        let frame = egui::Frame::default().fill(tone(ctx, Color32::from_rgb(36, 40, 47))).inner_margin(egui::Margin::symmetric(8, 4));
        egui::TopBottomPanel::top("options").frame(frame).show(ctx, |ui| {
            compact_style(ui);
            ui.horizontal_top(|ui| {
                let units = self.settings.units;
                let sel = self.selection_bbox();
                let (mn, mx) = sel.unwrap_or(([0.0, 0.0], [0.0, 0.0]));
                ui.add_enabled_ui(sel.is_some(), |ui| {
                    ui.horizontal_top(|ui| self.position_groups(ui, mn, mx));
                });
                // rotation
                group_frame(ui, true, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Rotate");
                        ui.add(egui::DragValue::new(&mut self.rotate_by).speed(1.0).suffix("°").max_decimals(1));
                        if ui.button("⟲").on_hover_text("Rotate counter-clockwise").clicked() {
                            self.rotate(self.rotate_by);
                        }
                        if ui.button("⟳").on_hover_text("Rotate clockwise").clicked() {
                            self.rotate(-self.rotate_by);
                        }
                        let ut = if units == Units::Mm { "mm" } else { "in" };
                        if ui.button(ut).on_hover_text("Switch between millimetres and inches").clicked() {
                            self.settings.units = if units == Units::Mm { Units::Inch } else { Units::Mm };
                        }
                    });
                });
                // what dragging the selection may do
                group_frame(ui, false, |ui| {
                    egui::Grid::new("msrs").num_columns(2).spacing([6.0, ROW_GAP]).show(ui, |ui| {
                        ui.checkbox(&mut self.design.allow_move, "Move");
                        ui.checkbox(&mut self.design.allow_size, "Size");
                        ui.end_row();
                        ui.checkbox(&mut self.design.allow_rotate, "Rotate");
                        ui.checkbox(&mut self.design.allow_shear, "Shear");
                        ui.end_row();
                    });
                });
                // tool options
                match self.tool {
                    CanvasTool::Star => group_frame(ui, true, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Points");
                            ui.add(egui::DragValue::new(&mut self.design.star_points).range(3..=100));
                            ui.label("Inner");
                            let mut pct = self.design.star_inner * 100.0;
                            if ui.add(egui::DragValue::new(&mut pct).speed(0.5).range(5.0..=95.0).suffix(" %")).changed() {
                                self.design.star_inner = pct / 100.0;
                            }
                        });
                    }),
                    CanvasTool::Corner => group_frame(ui, true, |ui| {
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut self.design.chamfer, false, "Round");
                            ui.radio_value(&mut self.design.chamfer, true, "Chamfer");
                            ui.label("Radius");
                            len_edit(ui, &mut self.design.corner_r, units, 0.1);
                            self.design.corner_r = self.design.corner_r.max(0.0);
                        });
                    }),
                    CanvasTool::Builder => group_frame(ui, true, |ui| {
                        ui.label("Click or drag across areas to merge them · Alt: delete areas");
                    }),
                    CanvasTool::Knife => group_frame(ui, true, |ui| {
                        ui.label("Drag a line right across shapes to split them");
                    }),
                    CanvasTool::Trim => group_frame(ui, true, |ui| {
                        ui.label("Click a piece of outline to remove it back to the crossing lines");
                    }),
                    CanvasTool::Dimension => group_frame(ui, true, |ui| {
                        ui.label("Drag to add a dimension · right-click one to remove it");
                    }),
                    CanvasTool::Nodes => group_frame(ui, true, |ui| {
                        ui.label("Drag points · double-click an edge to add · Delete removes");
                    }),
                    CanvasTool::Rect => group_frame(ui, true, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Corner radius");
                            len_edit(ui, &mut self.rect_corner, units, 0.2);
                            self.rect_corner = self.rect_corner.max(0.0);
                        });
                    }),
                    CanvasTool::Polygon => group_frame(ui, true, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Sides");
                            ui.add(egui::DragValue::new(&mut self.poly_sides).range(3..=64));
                            ui.checkbox(&mut self.poly_star, "Star");
                        });
                    }),
                    CanvasTool::Measure => group_frame(ui, true, |ui| {
                        if let Some((a, b)) = self.measure {
                            let d = crate::geom::dist(a, b);
                            ui.label(format!("Distance {}  ·  ΔX {}  ΔY {}", self.len_text(d), self.len_text(b[0] - a[0]), self.len_text(b[1] - a[1])));
                        } else {
                            ui.label("Drag to measure");
                        }
                    }),
                    _ => {}
                }
                // font (makes room for the tool's own box when one is showing)
                let tool_box = matches!(self.tool, CanvasTool::Rect | CanvasTool::Polygon | CanvasTool::Measure | CanvasTool::Star | CanvasTool::Corner | CanvasTool::Builder | CanvasTool::Knife | CanvasTool::Trim | CanvasTool::Nodes | CanvasTool::Dimension);
                if !tool_box || self.tool == CanvasTool::Text {
                group_frame(ui, false, |ui| {
                    egui::Grid::new("fontgrid").num_columns(6).spacing([6.0, ROW_GAP]).show(ui, |ui| {
                        ui.label("Font");
                        let fname = self.fonts.get(self.text_font).map(|f| f.0.clone()).unwrap_or_default();
                        egui::ComboBox::from_id_salt("font").width(130.0).selected_text(fname).show_ui(ui, |ui| {
                            for (i, f) in self.fonts.iter().enumerate() {
                                ui.selectable_value(&mut self.text_font, i, &f.0);
                            }
                        });
                        ui.label("Height");
                        len_edit(ui, &mut self.text_height, units, 0.2);
                        self.text_height = self.text_height.max(0.5);
                        ui.label("HSpace");
                        len_edit(ui, &mut self.text_spacing, units, 0.1);
                        ui.end_row();
                        ui.label("Text");
                        ui.add(egui::TextEdit::singleline(&mut self.text_input).desired_width(130.0).margin(egui::Margin::symmetric(4, 2)).min_size(egui::vec2(0.0, ROW_H)).hint_text("Your text"));
                        ui.checkbox(&mut self.design.text_upper, "Upper Case");
                        ui.label("");
                        ui.label("Bend").on_hover_text("Curve the text around a circle of this radius (0 = straight, negative bends downwards)");
                        len_edit(ui, &mut self.design.text_arc, units, 0.5);
                        ui.end_row();
                    });
                });
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    group_frame(ui, false, |ui| {
                        egui::Grid::new("snapgrid").num_columns(2).spacing([6.0, ROW_GAP]).show(ui, |ui| {
                            ui.checkbox(&mut self.settings.snap_grid, "Snap grid").on_hover_text("Snap to the grid");
                            len_edit(ui, &mut self.settings.grid, units, 0.5);
                            self.settings.grid = self.settings.grid.max(0.1);
                            ui.end_row();
                            ui.checkbox(&mut self.design.snap_objects, "Snap objects").on_hover_text("Snap to other shapes' points and centres");
                            ui.end_row();
                        });
                    });
                });
            });
        });
    }

    /// XPos/YPos, lock, Width/Height with percentages, and the 9-point reference picker.
    fn position_groups(&mut self, ui: &mut Ui, mn: P, mx: P) {
        let units = self.settings.units;
        let a = self.design.anchor;
        let fx = (a % 3) as f64 / 2.0;
        let fy = (2 - a / 3) as f64 / 2.0;
        let (w, h) = (mx[0] - mn[0], mx[1] - mn[1]);
        let (mut x, mut y, mut nw, mut nh) = (mn[0] + w * fx, mn[1] + h * fy, w, h);
        let (mut cx, mut cy, mut cw, mut ch) = (false, false, false, false);
        group_frame(ui, false, |ui| {
            egui::Grid::new("posgrid").num_columns(2).spacing([6.0, ROW_GAP]).show(ui, |ui| {
                ui.label("XPos");
                cx = len_edit(ui, &mut x, units, 0.5);
                ui.end_row();
                ui.label("YPos");
                cy = len_edit(ui, &mut y, units, 0.5);
                ui.end_row();
            });
        });
        let (lr, lresp) = ui.allocate_exact_size(egui::vec2(28.0, GROUP_OUTER_H), Sense::click());
        let lbg = if self.lock_aspect { Color32::from_rgb(30, 110, 160) } else { tone(ui.ctx(), Color32::from_rgb(55, 61, 71)) };
        ui.painter().rect_filled(lr, 4.0, lbg);
        let lc = lr.center();
        let lcol = if self.lock_aspect { Color32::WHITE } else { ui.visuals().text_color() };
        ui.painter().rect_stroke(Rect::from_center_size(lc + egui::vec2(0.0, 4.0), egui::vec2(14.0, 10.0)), 2.0, Stroke::new(1.5f32, lcol), egui::StrokeKind::Middle);
        let shackle: Vec<Pos2> = (0..=10)
            .map(|i| {
                let a = std::f32::consts::PI * i as f32 / 10.0;
                let lift = if self.lock_aspect { 0.0 } else { 3.0 };
                Pos2::new(lc.x - 4.5 * a.cos(), lc.y - 1.0 - lift - 5.0 * a.sin())
            })
            .collect();
        ui.painter().add(egui::Shape::line(shackle, Stroke::new(1.5f32, lcol)));
        if lresp.on_hover_text("Keep proportions").clicked() {
            self.lock_aspect = !self.lock_aspect;
        }
        let mut pct_apply = false;
        group_frame(ui, false, |ui| {
            egui::Grid::new("sizegrid").num_columns(3).spacing([6.0, ROW_GAP]).show(ui, |ui| {
                ui.label("Width");
                cw = len_edit(ui, &mut nw, units, 0.5);
                let rw = ui.add(egui::DragValue::new(&mut self.design.pct_w).speed(0.5).range(1.0..=10000.0).suffix(" %").max_decimals(1));
                ui.end_row();
                ui.label("Height");
                ch = len_edit(ui, &mut nh, units, 0.5);
                let rh = ui.add(egui::DragValue::new(&mut self.design.pct_h).speed(0.5).range(1.0..=10000.0).suffix(" %").max_decimals(1));
                ui.end_row();
                if (rw.drag_stopped() || rw.lost_focus()) && (self.design.pct_w - 100.0).abs() > 1e-9 {
                    if self.lock_aspect {
                        self.design.pct_h = self.design.pct_w;
                    }
                    pct_apply = true;
                }
                if (rh.drag_stopped() || rh.lost_focus()) && (self.design.pct_h - 100.0).abs() > 1e-9 {
                    if self.lock_aspect {
                        self.design.pct_w = self.design.pct_h;
                    }
                    pct_apply = true;
                }
            });
        });
        group_frame(ui, false, |ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(40.0, GROUP_H), Sense::hover());
            for i in 0..9 {
                let p = egui::pos2(rect.center().x + ((i % 3) as f32 - 1.0) * 12.5, rect.center().y + ((i / 3) as f32 - 1.0) * 12.5);
                let r = ui.interact(Rect::from_center_size(p, egui::vec2(12.0, 12.0)), ui.id().with(("anchor", i)), Sense::click());
                let on = i == a;
                ui.painter().circle(p, 3.8, if on { ui.visuals().text_color() } else { Color32::TRANSPARENT }, Stroke::new(1.0f32, ui.visuals().text_color()));
                if r.on_hover_text("Position and resizing use this point of the selection").clicked() {
                    self.design.anchor = i;
                }
            }
        });
        if pct_apply {
            nw = w * self.design.pct_w / 100.0;
            nh = h * self.design.pct_h / 100.0;
            self.design.pct_w = 100.0;
            self.design.pct_h = 100.0;
        }
        if self.lock_aspect && w > 1e-6 && h > 1e-6 {
            if cw {
                nh = nw * h / w;
            } else if ch {
                nw = nh * w / h;
            }
        }
        if (cx || cy || cw || ch || pct_apply) && !self.selection.is_empty() {
            let nw = nw.max(0.01);
            let nh = nh.max(0.01);
            let (ax, ay) = if cx || cy { (x, y) } else { (mn[0] + w * fx, mn[1] + h * fy) };
            self.set_selection_rect(ax - nw * fx, ay - nh * fy, nw, nh);
        }
    }

    pub fn right_panel(&mut self, ctx: &egui::Context) {
        self.ribbon(ctx);
        egui::SidePanel::right("right").default_width(400.0).min_width(300.0).resizable(true).show(ctx, |ui| {
            ui.add_space(6.0);
            let (r, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 22.0), Sense::hover());
            ui.painter().rect_filled(r, 0.0, HEADER_BLUE);
            ui.painter().text(r.left_center() + egui::vec2(6.0, 0.0), egui::Align2::LEFT_CENTER, self.right_tab.title(), egui::FontId::proportional(13.5), Color32::WHITE);
            ui.add_space(4.0);
            match self.right_tab {
                RightTab::Art => {
                    egui::ScrollArea::vertical().id_salt("art_scroll").show(ui, |ui| self.art_tab(ui));
                }
                RightTab::Makers => {
                    egui::ScrollArea::vertical().id_salt("makers_scroll").show(ui, |ui| self.makers_tab(ui));
                }
                RightTab::Shape => {
                    egui::ScrollArea::vertical().id_salt("shape_scroll").show(ui, |ui| {
                        self.effects_ui(ui);
                        self.shape_tab(ui, false);
                    });
                }
                RightTab::Arrange => {
                    egui::ScrollArea::vertical().id_salt("arr_scroll").show(ui, |ui| {
                        self.objects_list(ui);
                        self.nest_ui(ui);
                        self.tiles_ui(ui);
                        self.shape_tab(ui, true);
                    });
                }
                RightTab::Toolpaths => {
                    egui::ScrollArea::vertical().id_salt("tp_scroll").show(ui, |ui| self.toolpaths_tab(ui));
                }
                RightTab::Tools => {
                    self.tools_tab(ui);
                }
                RightTab::Stock => {
                    egui::ScrollArea::vertical().id_salt("stock_scroll").show(ui, |ui| self.stock_tab(ui));
                }
                RightTab::Machine => {
                    egui::ScrollArea::vertical().id_salt("mach_scroll").show(ui, |ui| self.machine_tab(ui, false));
                }
                RightTab::Move => {
                    egui::ScrollArea::vertical().id_salt("move_scroll").show(ui, |ui| self.machine_tab(ui, true));
                }
                RightTab::Modify => {
                    egui::ScrollArea::vertical().id_salt("mod_scroll").show(ui, |ui| {
                        self.modify_tab(ui);
                        self.stamp_ui(ui);
                    });
                }
                RightTab::History => {
                    egui::ScrollArea::vertical().id_salt("hist_scroll").show(ui, |ui| self.history_tab(ui));
                }
                RightTab::Console => self.console_tab(ui),
            }
        });
    }

    /// Every shape, front to back: click to select (Shift adds), lock to stop it being picked on the design.
    pub fn objects_list(&mut self, ui: &mut Ui) {
        section(ui, "Objects (front to back)");
        ui.label(RichText::new("Pick shapes here even when they sit behind others. Locked shapes can't be clicked on the design.").small().weak());
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.design.objects_filter).hint_text("Filter").desired_width(120.0));
            if ui.add_enabled(!self.selection.is_empty(), egui::Button::new("Lock selected")).clicked() {
                self.checkpoint();
                let sel = std::mem::take(&mut self.selection);
                for id in sel {
                    if !self.doc.locked.contains(&id) {
                        self.doc.locked.push(id);
                    }
                }
            }
            let sel_keep = !self.selection.is_empty() && self.selection.iter().all(|id| self.doc.keepout.contains(id));
            if ui.add_enabled(!self.selection.is_empty(), egui::Button::new(if sel_keep { "Not a clamp" } else { "Mark as clamp" })).on_hover_text("Clamps (keep-out zones) are never cut; the checks stop any cut that would hit one").clicked() {
                self.checkpoint();
                let sel = self.selection.clone();
                if sel_keep {
                    self.doc.keepout.retain(|id| !sel.contains(id));
                } else {
                    for id in sel {
                        if !self.doc.keepout.contains(&id) {
                            self.doc.keepout.push(id);
                        }
                    }
                }
            }
            if ui.add_enabled(!self.doc.locked.is_empty(), egui::Button::new("Unlock all")).clicked() {
                self.checkpoint();
                self.doc.locked.clear();
            }
        });
        let filter = self.design.objects_filter.to_lowercase();
        let shift = ui.input(|i| i.modifiers.shift);
        let mut click: Option<u64> = None;
        let mut toggle_lock: Option<u64> = None;
        let rows: Vec<(u64, String, bool, bool)> = self
            .doc
            .shapes
            .iter()
            .rev()
            .enumerate()
            .map(|(i, s)| {
                let tp: Vec<String> = self.doc.toolpaths.iter().filter(|t| t.shape_ids.contains(&s.id)).map(|t| t.name.clone()).collect();
                let size = s.bbox().map(|(a, b)| format!("{} × {}", self.len_text(b[0] - a[0]), self.len_text(b[1] - a[1]))).unwrap_or_default();
                let name = match &s.text {
                    Some(t) => format!("Text \"{t}\""),
                    None => s.kind.label().to_string(),
                };
                let label = format!("{}. {}{}  ·  {}{}{}", self.doc.shapes.len() - i, if self.doc.keepout.contains(&s.id) { "[CLAMP] " } else { "" }, name, size, if s.group.is_some() { "  ·  grouped" } else { "" }, if tp.is_empty() { String::new() } else { format!("  ·  {}", tp.join(", ")) });
                (s.id, label, self.selection.contains(&s.id), self.doc.locked.contains(&s.id))
            })
            .filter(|r| filter.is_empty() || r.1.to_lowercase().contains(&filter))
            .collect();
        egui::ScrollArea::vertical().id_salt("objects").max_height(240.0).show(ui, |ui| {
            for (id, label, selected, locked) in &rows {
                ui.horizontal(|ui| {
                    if ui.small_button(if *locked { "🔒" } else { "🔓" }).on_hover_text(if *locked { "Unlock" } else { "Lock" }).clicked() {
                        toggle_lock = Some(*id);
                    }
                    let text = if *locked { RichText::new(label).weak() } else { RichText::new(label) };
                    if ui.selectable_label(*selected, text).clicked() {
                        click = Some(*id);
                    }
                });
            }
        });
        if let Some(id) = toggle_lock {
            self.checkpoint();
            if let Some(i) = self.doc.locked.iter().position(|l| *l == id) {
                self.doc.locked.remove(i);
            } else {
                self.doc.locked.push(id);
                self.selection.retain(|s| *s != id);
            }
        }
        if let Some(id) = click {
            if shift {
                if let Some(i) = self.selection.iter().position(|s| *s == id) {
                    self.selection.remove(i);
                } else {
                    self.selection.push(id);
                }
            } else {
                self.selection = vec![id];
            }
            self.tool = CanvasTool::Select;
        }
    }

    pub fn history_tab(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Click a step to go back (or forward) to it.").weak());
        let summary = |d: &Document| format!("{} shapes · {} toolpaths", d.shapes.len(), d.toolpaths.len());
        let mut go_undo = 0usize;
        let mut go_redo = 0usize;
        let n = self.undo.len();
        if n == 0 && self.redo.is_empty() {
            ui.label("Nothing to undo yet.");
        }
        for (i, d) in self.undo.iter().enumerate() {
            if ui.selectable_label(false, format!("Step {}  ·  {}", i + 1, summary(d))).clicked() {
                go_undo = n - i;
            }
        }
        ui.add(egui::Button::new(RichText::new(format!("▶ Now  ·  {}", summary(&self.doc))).strong().color(Color32::WHITE)).fill(HEADER_BLUE));
        for (j, d) in self.redo.iter().rev().enumerate() {
            if ui.selectable_label(false, RichText::new(format!("Undone {}  ·  {}", j + 1, summary(d))).weak()).clicked() {
                go_redo = j + 1;
            }
        }
        for _ in 0..go_undo {
            self.do_undo();
        }
        for _ in 0..go_redo {
            self.do_redo();
        }
    }
}

#[allow(dead_code)]
fn _accent() -> Color32 {
    ACCENT
}
