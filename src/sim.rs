//! Material-removal preview: carves a height map with the job's moves.

use crate::cam::{Job, Move};
use crate::model::{Stock, Tool, ToolKind};

pub struct Sim {
    pub w: usize,
    pub h: usize,
    pub cell: f64,
    pub heights: Vec<f32>,
    pub thickness: f64,
    pub done_moves: usize,
    pub done_frac: f64,
    last: [f64; 3],
    pub dirty: bool,
}

impl Sim {
    pub fn new(stock: &Stock, max_cells: usize) -> Sim {
        let cell = ((stock.width * stock.height) / max_cells as f64).sqrt().max(0.1);
        let w = ((stock.width / cell).ceil() as usize).max(2);
        let h = ((stock.height / cell).ceil() as usize).max(2);
        Sim { w, h, cell, heights: vec![0.0; w * h], thickness: stock.thickness, done_moves: 0, done_frac: 0.0, last: [0.0, 0.0, 10.0], dirty: true }
    }

    fn stamp(&mut self, x: f64, y: f64, z: f64, tool: &Tool) {
        if z >= 0.0 {
            return;
        }
        let r = tool.radius().max(self.cell * 0.5);
        let ci = (x / self.cell) as isize;
        let cj = (y / self.cell) as isize;
        let rc = (r / self.cell).ceil() as isize;
        let half = (tool.angle.clamp(10.0, 170.0) / 2.0).to_radians();
        let floor = -self.thickness as f32 - 0.2;
        for j in (cj - rc).max(0)..=(cj + rc).min(self.h as isize - 1) {
            for i in (ci - rc).max(0)..=(ci + rc).min(self.w as isize - 1) {
                let px = (i as f64 + 0.5) * self.cell;
                let py = (j as f64 + 0.5) * self.cell;
                let d = ((px - x).powi(2) + (py - y).powi(2)).sqrt();
                if d > r {
                    continue;
                }
                let tz = match tool.kind {
                    ToolKind::FlatEnd => z,
                    ToolKind::BallNose => z + r - (r * r - d * d).max(0.0).sqrt(),
                    ToolKind::VBit | ToolKind::Drill => z + d / half.tan(),
                    ToolKind::DragKnife | ToolKind::ThreadMill => z,
                };
                if tz >= 0.0 {
                    continue;
                }
                let idx = j as usize * self.w + i as usize;
                let v = (tz as f32).max(floor);
                if v < self.heights[idx] {
                    self.heights[idx] = v;
                }
            }
        }
    }

    fn carve(&mut self, m: &Move, tool: &Tool, frac: f64) {
        let a = self.last;
        let b = [a[0] + (m.x - a[0]) * frac, a[1] + (m.y - a[1]) * frac, a[2] + (m.z - a[2]) * frac];
        if a[2] < 0.0 || b[2] < 0.0 {
            let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
            let n = ((len / (self.cell * 0.7)).ceil() as usize).max(1);
            for k in 0..=n {
                let t = k as f64 / n as f64;
                self.stamp(a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t, tool);
            }
            self.dirty = true;
        }
    }

    /// Advances the simulation by up to `budget` millimetres of travel; returns true when finished.
    pub fn advance(&mut self, job: &Job, mut budget: f64) -> bool {
        while self.done_moves < job.moves.len() {
            let m = job.moves[self.done_moves];
            let tool = job.tools.get(m.tp).and_then(|t| t.clone());
            let len = ((m.x - self.last[0]).powi(2) + (m.y - self.last[1]).powi(2) + (m.z - self.last[2]).powi(2)).sqrt();
            let remaining = len * (1.0 - self.done_frac);
            if remaining > budget && len > 0.0 {
                let f = self.done_frac + budget / len;
                if let Some(t) = &tool {
                    let start = self.last;
                    self.carve(&m, t, f);
                    self.last = start;
                }
                self.done_frac = f;
                return false;
            }
            budget -= remaining;
            if let Some(t) = &tool {
                self.carve(&m, t, 1.0);
            }
            self.last = [m.x, m.y, m.z];
            self.done_frac = 0.0;
            self.done_moves += 1;
        }
        true
    }

    /// Current tool tip position.
    pub fn tool_pos(&self, job: &Job) -> [f64; 3] {
        if let Some(m) = job.moves.get(self.done_moves) {
            let a = self.last;
            let f = self.done_frac;
            [a[0] + (m.x - a[0]) * f, a[1] + (m.y - a[1]) * f, a[2] + (m.z - a[2]) * f]
        } else {
            self.last
        }
    }

    /// Renders the height map as a shaded wood-coloured picture.
    pub fn render(&self) -> egui::ColorImage {
        let mut px = vec![0u8; self.w * self.h * 4];
        let t = self.thickness.max(0.1) as f32;
        let light = {
            let l = [-0.5f32, 0.6, 0.65];
            let n = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
            [l[0] / n, l[1] / n, l[2] / n]
        };
        let cell = self.cell as f32;
        for row in 0..self.h {
            let j = self.h - 1 - row; // image rows go top-down, Y goes up
            for i in 0..self.w {
                let hgt = |ii: usize, jj: usize| self.heights[jj.min(self.h - 1) * self.w + ii.min(self.w - 1)];
                let z = hgt(i, j);
                let dzdx = (hgt(i + 1, j) - hgt(i.saturating_sub(1), j)) / (2.0 * cell);
                let dzdy = (hgt(i, j + 1) - hgt(i, j.saturating_sub(1))) / (2.0 * cell);
                let n = [-dzdx, -dzdy, 1.0];
                let nl = (n[0] * n[0] + n[1] * n[1] + 1.0).sqrt();
                let shade = ((n[0] * light[0] + n[1] * light[1] + n[2] * light[2]) / nl).clamp(0.0, 1.0);
                let depth = (-z / t).clamp(0.0, 1.0);
                let through = z <= -t;
                let (r, g, b) = if through {
                    (40.0, 44.0, 52.0)
                } else if z >= 0.0 {
                    let grain = (((i as f32 * 0.05 + (j as f32 * 0.9).sin() * 0.6).sin()) * 0.5 + 0.5) * 14.0;
                    (214.0 - grain, 172.0 - grain, 118.0 - grain * 0.6)
                } else {
                    (200.0 - depth * 70.0, 150.0 - depth * 60.0, 96.0 - depth * 40.0)
                };
                let s = 0.45 + 0.75 * shade;
                let k = (row * self.w + i) * 4;
                px[k] = (r * s).clamp(0.0, 255.0) as u8;
                px[k + 1] = (g * s).clamp(0.0, 255.0) as u8;
                px[k + 2] = (b * s).clamp(0.0, 255.0) as u8;
                px[k + 3] = 255;
            }
        }
        egui::ColorImage::from_rgba_unmultiplied([self.w, self.h], &px)
    }
}

/// Total travel of a job in millimetres (for the playback slider).
pub fn travel(job: &Job) -> Vec<f64> {
    let mut cum = Vec::with_capacity(job.moves.len());
    let mut last = [0.0, 0.0, 10.0];
    let mut acc = 0.0;
    for m in &job.moves {
        acc += ((m.x - last[0]).powi(2) + (m.y - last[1]).powi(2) + (m.z - last[2]).powi(2)).sqrt();
        cum.push(acc);
        last = [m.x, m.y, m.z];
    }
    cum
}
