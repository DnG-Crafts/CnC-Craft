//! Turns text into outline paths using a TrueType font.

use crate::geom;
use crate::model::{Path, P};

pub const BUILTIN: &str = "Built-in sans";

/// Lists fonts available on this computer (file stem, full path).
pub fn system_fonts() -> Vec<(String, String)> {
    let mut out = vec![(BUILTIN.to_string(), String::new())];
    let dirs = [
        std::env::var("WINDIR").map(|w| format!("{w}\\Fonts")).unwrap_or_else(|_| "C:\\Windows\\Fonts".into()),
        std::env::var("LOCALAPPDATA").map(|l| format!("{l}\\Microsoft\\Windows\\Fonts")).unwrap_or_default(),
        "/usr/share/fonts/truetype".into(),
    ];
    for d in dirs {
        if d.is_empty() {
            continue;
        }
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        let mut found: Vec<(String, String)> = rd
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                let ext = p.extension()?.to_string_lossy().to_lowercase();
                if ext == "ttf" || ext == "otf" {
                    Some((p.file_stem()?.to_string_lossy().to_string(), p.to_string_lossy().to_string()))
                } else {
                    None
                }
            })
            .collect();
        found.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
        out.extend(found);
    }
    out
}

struct Builder {
    paths: Vec<Path>,
    cur: Vec<P>,
    scale: f64,
    ox: f64,
    oy: f64,
}

impl Builder {
    fn tp(&self, x: f32, y: f32) -> P {
        [self.ox + x as f64 * self.scale, self.oy + y as f64 * self.scale]
    }
    fn flush(&mut self) {
        if self.cur.len() > 2 {
            let mut pts = std::mem::take(&mut self.cur);
            if geom::dist(pts[0], *pts.last().unwrap()) < 1e-6 {
                pts.pop();
            }
            self.paths.push(Path { pts, closed: true });
        }
        self.cur.clear();
    }
}

impl ttf_parser::OutlineBuilder for Builder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.flush();
        let p = self.tp(x, y);
        self.cur.push(p);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.tp(x, y);
        self.cur.push(p);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let p0 = *self.cur.last().unwrap_or(&[0.0, 0.0]);
        let p1 = self.tp(x1, y1);
        let p2 = self.tp(x, y);
        geom::quad(&mut self.cur, p0, p1, p2);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let p0 = *self.cur.last().unwrap_or(&[0.0, 0.0]);
        let (p1, p2, p3) = (self.tp(x1, y1), self.tp(x2, y2), self.tp(x, y));
        geom::cubic(&mut self.cur, p0, p1, p2, p3);
    }
    fn close(&mut self) {
        self.flush();
    }
}

/// Builds outlines for `text` with its baseline-left corner at `origin` and cap height ~`height` mm.
pub fn text_paths(text: &str, font_path: &str, height: f64, spacing: f64, origin: P) -> Result<Vec<Path>, String> {
    let data: Vec<u8> = if font_path.is_empty() {
        epaint_default_fonts::UBUNTU_LIGHT.to_vec()
    } else {
        std::fs::read(font_path).map_err(|e| format!("Could not read font: {e}"))?
    };
    let face = ttf_parser::Face::parse(&data, 0).map_err(|e| format!("Could not read font: {e}"))?;
    let cap = face.capital_height().map(|c| c as f64).unwrap_or(face.ascender() as f64 * 0.7).max(1.0);
    let scale = height / cap;
    let line_h = (face.ascender() - face.descender() + face.line_gap()) as f64 * scale;
    let mut b = Builder { paths: vec![], cur: vec![], scale, ox: origin[0], oy: origin[1] };
    for (li, line) in text.lines().enumerate() {
        b.ox = origin[0];
        b.oy = origin[1] - li as f64 * line_h;
        for ch in line.chars() {
            let Some(gid) = face.glyph_index(ch) else {
                b.ox += height * 0.5;
                continue;
            };
            face.outline_glyph(gid, &mut b);
            b.flush();
            let adv = face.glyph_hor_advance(gid).unwrap_or(0) as f64 * scale;
            b.ox += adv + spacing;
        }
    }
    Ok(b.paths)
}
