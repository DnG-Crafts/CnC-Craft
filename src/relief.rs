//! 3D reliefs: a greyscale picture (white = high) or a 3D model (STL) turned into a height map that
//! fills the box of a frame shape, ready for the 3D rough and 3D finish toolpaths.

use crate::app::App;
use crate::geom;
use crate::model::*;

/// Reads an STL file (binary or text) into triangles.
pub fn parse_stl(data: &[u8]) -> Result<Vec<[[f64; 3]; 3]>, String> {
    if data.len() >= 84 {
        let n = u32::from_le_bytes([data[80], data[81], data[82], data[83]]) as usize;
        if n > 0 && data.len() == 84 + n * 50 {
            let mut tris = Vec::with_capacity(n);
            for i in 0..n {
                let b = 84 + i * 50 + 12;
                let f = |k: usize| f32::from_le_bytes([data[b + k * 4], data[b + k * 4 + 1], data[b + k * 4 + 2], data[b + k * 4 + 3]]) as f64;
                tris.push([[f(0), f(1), f(2)], [f(3), f(4), f(5)], [f(6), f(7), f(8)]]);
            }
            return Ok(tris);
        }
    }
    let text = String::from_utf8_lossy(data);
    let mut verts: Vec<[f64; 3]> = vec![];
    for line in text.lines() {
        let mut it = line.split_whitespace();
        if it.next() == Some("vertex") {
            let v: Vec<f64> = it.filter_map(|s| s.parse().ok()).collect();
            if v.len() == 3 {
                verts.push([v[0], v[1], v[2]]);
            }
        }
    }
    if verts.len() < 3 {
        return Err("no triangles found - is it an STL file?".into());
    }
    Ok(verts.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect())
}

/// The model seen from above: (heights 0..1 with row 0 at the back, cols, rows, size X, size Y, height).
pub fn stl_heightmap(tris: &[[[f64; 3]; 3]], long_cells: usize) -> Option<(Vec<f64>, usize, usize, f64, f64, f64)> {
    let pts = tris.iter().flatten();
    let (mut mn, mut mx) = ([f64::MAX; 3], [f64::MIN; 3]);
    for p in pts {
        for k in 0..3 {
            mn[k] = mn[k].min(p[k]);
            mx[k] = mx[k].max(p[k]);
        }
    }
    let (sx, sy, sz) = (mx[0] - mn[0], mx[1] - mn[1], mx[2] - mn[2]);
    if !(sx > 1e-9 && sy > 1e-9) {
        return None;
    }
    let cell = sx.max(sy) / long_cells as f64;
    let (w, h) = (((sx / cell).ceil() as usize).max(1), ((sy / cell).ceil() as usize).max(1));
    let mut z = vec![mn[2]; w * h];
    for t in tris {
        let (a, b, c) = (t[0], t[1], t[2]);
        let den = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
        if den.abs() < 1e-12 {
            continue;
        }
        let x0 = (((a[0].min(b[0]).min(c[0]) - mn[0]) / cell).floor().max(0.0)) as usize;
        let x1 = (((a[0].max(b[0]).max(c[0]) - mn[0]) / cell).ceil() as usize).min(w);
        let y0 = (((a[1].min(b[1]).min(c[1]) - mn[1]) / cell).floor().max(0.0)) as usize;
        let y1 = (((a[1].max(b[1]).max(c[1]) - mn[1]) / cell).ceil() as usize).min(h);
        for j in y0..y1 {
            for i in x0..x1 {
                let (px, py) = (mn[0] + (i as f64 + 0.5) * cell, mn[1] + (j as f64 + 0.5) * cell);
                let l1 = ((b[1] - c[1]) * (px - c[0]) + (c[0] - b[0]) * (py - c[1])) / den;
                let l2 = ((c[1] - a[1]) * (px - c[0]) + (a[0] - c[0]) * (py - c[1])) / den;
                let l3 = 1.0 - l1 - l2;
                if l1 < -1e-9 || l2 < -1e-9 || l3 < -1e-9 {
                    continue;
                }
                let zz = l1 * a[2] + l2 * b[2] + l3 * c[2];
                let k = (h - 1 - j) * w + i;
                if zz > z[k] {
                    z[k] = zz;
                }
            }
        }
    }
    let heights = z.iter().map(|v| if sz > 1e-12 { (v - mn[2]) / sz } else { 1.0 }).collect();
    Some((heights, w, h, sx, sy, sz))
}

impl App {
    pub fn import_relief_dialog(&mut self) {
        let Some(p) = rfd::FileDialog::new().add_filter("Picture or 3D model", &["png", "jpg", "jpeg", "stl"]).pick_file() else { return };
        let name = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Relief".into());
        let is_stl = p.extension().map(|e| e.to_string_lossy().eq_ignore_ascii_case("stl")).unwrap_or(false);
        let res = if is_stl {
            std::fs::read(&p).map_err(|e| e.to_string()).and_then(|d| parse_stl(&d)).and_then(|t| stl_heightmap(&t, 400).ok_or_else(|| "the model is flat".to_string())).map(|(hgt, w, h, sx, sy, sz)| (hgt, w, h, Some((sx, sy, sz))))
        } else {
            image::open(&p).map_err(|e| e.to_string()).map(|img| {
                let g = img.to_luma16();
                let (w0, h0) = (g.width().max(1), g.height().max(1));
                let k = (500.0 / w0.max(h0) as f64).min(1.0);
                let (w, h) = (((w0 as f64 * k).round() as u32).max(1), ((h0 as f64 * k).round() as u32).max(1));
                let g = image::imageops::resize(&g, w, h, image::imageops::FilterType::Triangle);
                (g.pixels().map(|px| px.0[0] as f64 / 65535.0).collect(), w as usize, h as usize, None)
            })
        };
        match res {
            Ok((hgt, w, h, size)) => self.add_relief(&name, hgt, w, h, size),
            Err(e) => self.status = format!("Could not import {name}: {e}"),
        }
    }

    /// Adds a relief centred on the stock: a 3D model keeps its own size (shrunk to fit if needed),
    /// a picture fills most of the stock.
    pub fn add_relief(&mut self, name: &str, heights: Vec<f64>, w: usize, h: usize, size: Option<(f64, f64, f64)>) {
        let st = &self.doc.stock;
        let (sw, sh, th) = (st.width, st.height, st.thickness);
        let (bw, bh, depth) = match size {
            Some((sx, sy, sz)) => {
                let k = (sw * 0.95 / sx).min(sh * 0.95 / sy).min(1.0);
                (sx * k, sy * k, (sz * k).min(th))
            }
            None => {
                let k = (sw * 0.8 / w as f64).min(sh * 0.8 / h as f64);
                (w as f64 * k, h as f64 * k, (th * 0.5).min(6.0))
            }
        };
        let mn = [(sw - bw) / 2.0, (sh - bh) / 2.0];
        self.checkpoint();
        let id = self.doc.new_id();
        self.doc.shapes.push(Shape { id, kind: ShapeKind::Rect, paths: vec![geom::rect_path(mn, [mn[0] + bw, mn[1] + bh], 0.0)], group: None, text: None, fx: None });
        let heights = heights.iter().map(|v| (v.clamp(0.0, 1.0) * 65535.0).round() as u16).collect();
        self.doc.reliefs.push(Relief { shape_id: id, name: name.to_string(), w, h, heights, depth });
        self.selection = vec![id];
        self.job_dirty = true;
        self.status = format!("3D relief \"{name}\" added ({:.0} × {:.0} mm, {:.1} mm deep) - now add a 3D rough and a 3D finish toolpath", bw, bh, depth);
    }

    /// A shaded picture of a relief for the design view (cached until the relief changes).
    pub fn relief_texture(&self, ctx: &egui::Context, rl: &Relief) -> egui::TextureHandle {
        let key = rl.w * 1_000_003 + rl.h * 7919 + rl.heights.first().copied().unwrap_or(0) as usize + rl.heights.get(rl.heights.len() / 2).copied().unwrap_or(0) as usize * 3;
        if let Some((k, t)) = self.design.relief_tex.borrow().get(&rl.shape_id) {
            if *k == key {
                return t.clone();
            }
        }
        let s = (256.0 / rl.w.max(rl.h) as f64).min(1.0);
        let (w, h) = (((rl.w as f64 * s) as usize).max(1), ((rl.h as f64 * s) as usize).max(1));
        let mut px = Vec::with_capacity(w * h * 4);
        let at = |i: usize, j: usize| rl.sample((i as f64 + 0.5) / w as f64, 1.0 - (j as f64 + 0.5) / h as f64);
        for j in 0..h {
            for i in 0..w {
                let v = at(i, j);
                let dx = at((i + 1).min(w - 1), j) - at(i.saturating_sub(1), j);
                let dy = at(i, (j + 1).min(h - 1)) - at(i, j.saturating_sub(1));
                let shade = (0.75 + (dy - dx) * 6.0).clamp(0.35, 1.25);
                let base = [70.0 + 150.0 * v, 52.0 + 120.0 * v, 36.0 + 80.0 * v];
                px.extend_from_slice(&[(base[0] * shade).min(255.0) as u8, (base[1] * shade).min(255.0) as u8, (base[2] * shade).min(255.0) as u8, 235]);
            }
        }
        let img = egui::ColorImage::from_rgba_unmultiplied([w, h], &px);
        let t = ctx.load_texture(format!("relief{}", rl.shape_id), img, egui::TextureOptions::LINEAR);
        self.design.relief_tex.borrow_mut().insert(rl.shape_id, (key, t.clone()));
        t
    }

    pub fn paint_reliefs(&self, painter: &egui::Painter, rect: egui::Rect) {
        for rl in &self.doc.reliefs {
            let Some((mn, mx)) = self.doc.shape(rl.shape_id).and_then(|s| s.bbox()) else { continue };
            let t = self.relief_texture(painter.ctx(), rl);
            let r = egui::Rect::from_two_pos(self.to_screen(rect, [mn[0], mx[1]]), self.to_screen(rect, [mx[0], mn[1]]));
            painter.image(t.id(), r, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
        }
    }
}
