//! 3D and side views for the cut preview: the simulated height map drawn as a shaded block you can
//! turn and zoom, the toolpaths at their real depths, and Front/Side cut-through slices.

use crate::app::App;
use egui::{Color32, Pos2, Stroke, Ui, Vec2};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PreviewView {
    Top,
    Iso,
    Front,
    Side,
}

pub struct Cam {
    pub view: PreviewView,
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
    pub pan: Vec2,
    pub exag: f32,
    /// where the Front (Y) and Side (X) slices cut through, as a fraction of the stock
    pub section: f32,
    pub img: Option<egui::ColorImage>,
}

impl Default for Cam {
    fn default() -> Self {
        Cam { view: PreviewView::Top, yaw: -0.6, pitch: 0.75, zoom: 1.0, pan: Vec2::ZERO, exag: 1.0, section: 0.5, img: None }
    }
}

impl App {
    /// View buttons above the preview picture.
    pub fn preview_view_bar(&mut self, ui: &mut Ui) {
        let cam = &mut self.design.cam;
        ui.horizontal(|ui| {
            ui.label("View");
            for (v, l, tip) in [
                (PreviewView::Top, "Top", "Looking straight down"),
                (PreviewView::Iso, "3D", "Drag to turn, scroll to zoom, right-drag to pan, double-click to reset"),
                (PreviewView::Front, "Front", "A slice through the material, seen from the front"),
                (PreviewView::Side, "Side", "A slice through the material, seen from the side"),
            ] {
                if ui.selectable_label(cam.view == v, l).on_hover_text(tip).clicked() {
                    cam.view = v;
                }
            }
            ui.separator();
            ui.label("Depth ×");
            ui.add(egui::Slider::new(&mut cam.exag, 1.0..=6.0).max_decimals(1)).on_hover_text("Exaggerate depths so shallow cuts are easier to see");
            if matches!(cam.view, PreviewView::Front | PreviewView::Side) {
                ui.separator();
                ui.label("Cut through at");
                let mut pct = cam.section * 100.0;
                if ui.add(egui::Slider::new(&mut pct, 0.0..=100.0).suffix(" %").max_decimals(0)).changed() {
                    cam.section = pct / 100.0;
                }
            }
            if cam.view == PreviewView::Iso && ui.button("Reset view").clicked() {
                *cam = Cam { view: PreviewView::Iso, exag: cam.exag, img: cam.img.take(), ..Cam::default() };
            }
        });
    }

    /// Draws the 3D or section view into `rect`.
    pub fn paint_preview_3d(&mut self, ui: &mut Ui, rect: egui::Rect, resp: &egui::Response) {
        match self.design.cam.view {
            PreviewView::Iso => self.paint_iso(ui, rect, resp),
            PreviewView::Front | PreviewView::Side => self.paint_section(ui, rect),
            PreviewView::Top => {}
        }
    }

    fn paint_iso(&mut self, ui: &mut Ui, rect: egui::Rect, resp: &egui::Response) {
        // camera controls
        {
            let cam = &mut self.design.cam;
            if resp.dragged_by(egui::PointerButton::Primary) {
                let d = resp.drag_delta();
                cam.yaw -= d.x * 0.01;
                cam.pitch = (cam.pitch + d.y * 0.01).clamp(0.05, std::f32::consts::FRAC_PI_2);
            }
            if resp.dragged_by(egui::PointerButton::Secondary) || resp.dragged_by(egui::PointerButton::Middle) {
                cam.pan += resp.drag_delta();
            }
            if resp.hovered() {
                let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                if scroll != 0.0 {
                    cam.zoom = (cam.zoom * (scroll * 0.003).exp()).clamp(0.2, 20.0);
                }
            }
            if resp.double_clicked() {
                *cam = Cam { view: PreviewView::Iso, exag: cam.exag, img: cam.img.take(), ..Cam::default() };
            }
        }
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, Color32::from_rgb(24, 26, 30));
        let (Some(sim), Some(img)) = (self.sim.as_ref(), self.design.cam.img.as_ref()) else { return };
        let cam = &self.design.cam;
        let (w, h, cell) = (sim.w, sim.h, sim.cell as f32);
        let thick = sim.thickness as f32;
        let exag = cam.exag;
        let (sw, sh) = (w as f32 * cell, h as f32 * cell);
        let (cx, cy, cz) = (sw / 2.0, sh / 2.0, -thick * exag / 2.0);
        let (sy_, cy_) = cam.yaw.sin_cos();
        let (sp, cp) = cam.pitch.sin_cos();
        let diag = (sw * sw + sh * sh + (thick * exag).powi(2)).sqrt();
        let scale = rect.width().min(rect.height()) / diag * 0.95 * cam.zoom;
        let center = rect.center() + cam.pan;
        // world (x, y, z) -> screen position and depth (bigger = nearer)
        let project = |x: f32, y: f32, z: f32| -> (Pos2, f32) {
            let (dx, dy, dz) = (x - cx, y - cy, z * exag - cz);
            let xr = dx * cy_ - dy * sy_;
            let yr = dx * sy_ + dy * cy_;
            let up = yr * sp + dz * cp;
            let depth = -yr * cp + dz * sp;
            (Pos2::new(center.x + xr * scale, center.y - up * scale), depth)
        };
        let step = ((w * h) as f32 / 22_000.0).sqrt().ceil().max(1.0) as usize;
        let gw = w.div_ceil(step);
        let gh = h.div_ceil(step);
        let height = |gi: usize, gj: usize| -> f32 {
            let i = (gi * step).min(w - 1);
            let j = (gj * step).min(h - 1);
            sim.heights[j * w + i]
        };
        let colour = |gi: usize, gj: usize| -> Color32 {
            let i = (gi * step + step / 2).min(w - 1);
            let j = (gj * step + step / 2).min(h - 1);
            img.pixels[(h - 1 - j) * w + i]
        };
        // grid points
        let mut pts = Vec::with_capacity((gw + 1) * (gh + 1));
        for gj in 0..=gh {
            for gi in 0..=gw {
                let x = ((gi * step) as f32 * cell).min(sw);
                let y = ((gj * step) as f32 * cell).min(sh);
                pts.push(project(x, y, height(gi.min(gw - 1), gj.min(gh - 1))));
            }
        }
        let at = |gi: usize, gj: usize| pts[gj * (gw + 1) + gi];
        // quads: (depth, corners, colour)
        let mut quads: Vec<(f32, [Pos2; 4], Color32)> = Vec::with_capacity(gw * gh + 2 * (gw + gh));
        for gj in 0..gh {
            for gi in 0..gw {
                let c = [at(gi, gj), at(gi + 1, gj), at(gi + 1, gj + 1), at(gi, gj + 1)];
                let d = (c[0].1 + c[1].1 + c[2].1 + c[3].1) / 4.0;
                quads.push((d, [c[0].0, c[1].0, c[2].0, c[3].0], colour(gi, gj)));
            }
        }
        // the block's sides, from the top surface down to the bottom
        let bottom = -thick - 0.2;
        let mut wall = |x0: f32, y0: f32, x1: f32, y1: f32, z0: f32, z1: f32, shade: f32| {
            let a = project(x0, y0, z0);
            let b = project(x1, y1, z1);
            let c = project(x1, y1, bottom);
            let d = project(x0, y0, bottom);
            let depth = (a.1 + b.1 + c.1 + d.1) / 4.0;
            let col = Color32::from_rgb((176.0 * shade) as u8, (136.0 * shade) as u8, (88.0 * shade) as u8);
            if z0 > bottom + 1e-3 || z1 > bottom + 1e-3 {
                quads.push((depth, [a.0, b.0, c.0, d.0], col));
            }
        };
        for gi in 0..gw {
            let (x0, x1) = (((gi * step) as f32 * cell).min(sw), (((gi + 1) * step) as f32 * cell).min(sw));
            wall(x0, 0.0, x1, 0.0, height(gi, 0), height(gi, 0), 0.82);
            wall(x0, sh, x1, sh, height(gi, gh - 1), height(gi, gh - 1), 0.62);
        }
        for gj in 0..gh {
            let (y0, y1) = (((gj * step) as f32 * cell).min(sh), (((gj + 1) * step) as f32 * cell).min(sh));
            wall(0.0, y0, 0.0, y1, height(0, gj), height(0, gj), 0.7);
            wall(sw, y0, sw, y1, height(gw - 1, gj), height(gw - 1, gj), 0.74);
        }
        quads.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut mesh = egui::epaint::Mesh::default();
        for (_, c, col) in &quads {
            let base = mesh.vertices.len() as u32;
            for p in c {
                mesh.colored_vertex(*p, *col);
            }
            mesh.add_triangle(base, base + 1, base + 2);
            mesh.add_triangle(base, base + 2, base + 3);
        }
        // shadow under the block
        painter.add(egui::Shape::mesh(mesh));

        // toolpaths at their real depths
        if self.sim_show_paths {
            let mut last: Option<Pos2> = None;
            for m in &self.job.moves {
                let (p, _) = project(m.x as f32, m.y as f32, m.z as f32);
                if let Some(a) = last {
                    if m.rapid {
                        painter.line_segment([a, p], Stroke::new(0.8f32, Color32::from_rgba_unmultiplied(255, 110, 110, 90)));
                    } else {
                        let c = self.doc.toolpaths.get(m.tp).map(|t| t.color).unwrap_or([255, 255, 255]);
                        painter.line_segment([a, p], Stroke::new(1.0f32, Color32::from_rgba_unmultiplied(c[0], c[1], c[2], 190)));
                    }
                }
                last = Some(p);
            }
        }
        // the bit
        if sim.done_moves < self.job.moves.len() {
            let t = sim.tool_pos(&self.job);
            let (tip, _) = project(t[0] as f32, t[1] as f32, t[2] as f32);
            let (top, _) = project(t[0] as f32, t[1] as f32, t[2] as f32 + 25.0 / exag);
            painter.line_segment([tip, top], Stroke::new(4.0f32, Color32::from_rgb(200, 210, 225)));
            painter.circle_filled(tip, 3.0, Color32::WHITE);
        }
        painter.text(rect.left_bottom() + egui::vec2(8.0, -6.0), egui::Align2::LEFT_BOTTOM, "Drag to turn · scroll to zoom · right-drag to pan · double-click to reset", egui::FontId::proportional(11.0), Color32::from_gray(150));
    }

    /// A cut-through slice: Front cuts across X at a Y position, Side cuts across Y at an X position.
    fn paint_section(&mut self, ui: &mut Ui, rect: egui::Rect) {
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, Color32::from_rgb(24, 26, 30));
        let Some(sim) = self.sim.as_ref() else { return };
        let cam = &self.design.cam;
        let front = cam.view == PreviewView::Front;
        let (w, h, cell) = (sim.w, sim.h, sim.cell);
        let n = if front { w } else { h };
        let line = if front { ((h - 1) as f32 * cam.section).round() as usize } else { ((w - 1) as f32 * cam.section).round() as usize };
        let length = n as f64 * cell;
        let thick = sim.thickness;
        let exag = cam.exag as f64;
        let margin = 50.0;
        let area = rect.shrink2(egui::vec2(margin, 40.0));
        let sx = area.width() as f64 / length;
        // stretch depths so the slice is readable (at least about a third of the view tall)
        let exag = exag.max((area.height() as f64 * 0.35) / (thick * sx).max(1e-9)).min((area.height() as f64 * 0.8) / (thick * sx).max(1e-9)).max(1.0);
        let sz = sx * exag;
        let base_y = area.center().y + (thick * sz / 2.0) as f32;
        let to = |s: f64, z: f64| Pos2::new(area.left() + (s * sx) as f32, base_y - ((z + thick) * sz) as f32);
        // the material, column by column
        for k in 0..n {
            let z = if front { sim.heights[line * w + k] } else { sim.heights[k * w + line] } as f64;
            let top = z.max(-thick);
            if top <= -thick + 1e-6 {
                continue;
            }
            let a = to(k as f64 * cell, top);
            let b = to((k + 1) as f64 * cell, -thick);
            let depth = (-top / thick).clamp(0.0, 1.0) as f32;
            let col = if top >= -1e-6 { Color32::from_rgb(200, 158, 104) } else { Color32::from_rgb((186.0 - depth * 50.0) as u8, (140.0 - depth * 45.0) as u8, (88.0 - depth * 30.0) as u8) };
            painter.rect_filled(egui::Rect::from_min_max(Pos2::new(a.x, a.y), Pos2::new(b.x + 0.6, b.y)), 0.0, col);
        }
        // outline of the cut surface
        let mut pts = vec![];
        for k in 0..n {
            let z = if front { sim.heights[line * w + k] } else { sim.heights[k * w + line] } as f64;
            let z = z.max(-thick - 0.2);
            pts.push(to(k as f64 * cell, z));
            pts.push(to((k + 1) as f64 * cell, z));
        }
        painter.add(egui::Shape::line(pts, Stroke::new(1.2f32, Color32::from_rgb(255, 215, 150))));
        // scale marks
        let s = Stroke::new(1.0f32, Color32::from_gray(110));
        let txt = Color32::from_gray(170);
        let f = egui::FontId::proportional(10.5);
        for (z, label) in [(0.0, "top 0".to_string()), (-thick, format!("{:.1}", -thick)), (-thick / 2.0, format!("{:.1}", -thick / 2.0))] {
            let p = to(0.0, z);
            painter.line_segment([Pos2::new(p.x - 8.0, p.y), Pos2::new(p.x - 2.0, p.y)], s);
            painter.text(Pos2::new(p.x - 10.0, p.y), egui::Align2::RIGHT_CENTER, label, f.clone(), txt);
        }
        let axis = if front { "X" } else { "Y" };
        let at = (line as f64 + 0.5) * cell;
        painter.text(
            rect.left_top() + egui::vec2(10.0, 8.0),
            egui::Align2::LEFT_TOP,
            format!("{} view · cut through at {} = {}  (depths × {:.1})", if front { "Front" } else { "Side" }, if front { "Y" } else { "X" }, self.len_text(at), exag),
            egui::FontId::proportional(12.0),
            Color32::from_gray(200),
        );
        let end = to(length, -thick);
        painter.text(Pos2::new(end.x, end.y + 14.0), egui::Align2::RIGHT_TOP, format!("{axis} {}", self.len_text(length)), f, txt);
    }
}
