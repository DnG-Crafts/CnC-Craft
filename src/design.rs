//! Design generators: shape library, makers (box, gear, hinge, puzzle, sign), hole patterns,
//! corner rounding, path joining and QR codes.

use crate::geom::{self, dist, ellipse_path, polygon_path, rect_path, signed_area};
use crate::model::{Path, P};
use std::f64::consts::{PI, TAU};

// ---------------------------------------------------------------- shape library

pub const ART: [&str; 24] = [
    "Heart", "Star", "Six star", "Burst", "Arrow", "Double arrow", "Cross", "Ring", "Moon", "Lightning", "House", "Speech bubble",
    "Shield", "Leaf", "Teardrop", "Diamond", "Triangle", "Octagon", "Cloud", "Paw", "Check", "Tree", "Bone", "Gear",
];

fn poly(pts: &[[f64; 2]]) -> Path {
    Path { pts: pts.to_vec(), closed: true }
}

fn circle(c: P, r: f64) -> Path {
    ellipse_path(c, r, r)
}

fn union_paths(parts: Vec<Path>) -> Vec<Path> {
    let mut acc = geo::MultiPolygon::new(vec![]);
    for p in parts {
        let s = crate::model::Shape { id: 0, kind: crate::model::ShapeKind::Path, paths: vec![p], group: None, text: None, fx: None };
        acc = geom::boolean(&acc, &geom::region_of(std::iter::once(&s)), geom::BoolOp::Union);
    }
    geom::region_to_paths(&acc)
}

fn subtract(a: Vec<Path>, b: Vec<Path>) -> Vec<Path> {
    let sa = crate::model::Shape { id: 0, kind: crate::model::ShapeKind::Path, paths: a, group: None, text: None, fx: None };
    let sb = crate::model::Shape { id: 0, kind: crate::model::ShapeKind::Path, paths: b, group: None, text: None, fx: None };
    geom::region_to_paths(&geom::boolean(&geom::region_of(std::iter::once(&sa)), &geom::region_of(std::iter::once(&sb)), geom::BoolOp::Difference))
}

/// Library shape scaled to roughly `size` mm, centered on the origin.
pub fn art_shape(name: &str, size: f64) -> Vec<Path> {
    let s = size / 2.0;
    let raw: Vec<Path> = match name {
        "Heart" => {
            let n = 120;
            let pts = (0..n)
                .map(|i| {
                    let t = TAU * i as f64 / n as f64;
                    let x = 16.0 * t.sin().powi(3);
                    let y = 13.0 * t.cos() - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (4.0 * t).cos();
                    [x / 17.0, y / 17.0]
                })
                .collect();
            vec![Path { pts, closed: true }]
        }
        "Star" => vec![polygon_path([0.0, 0.0], 1.0, 5, true, 0.0)],
        "Six star" => vec![polygon_path([0.0, 0.0], 1.0, 6, true, 0.0)],
        "Burst" => {
            let n = 32;
            let pts = (0..n).map(|i| {
                let a = TAU * i as f64 / n as f64;
                let r = if i % 2 == 0 { 1.0 } else { 0.78 };
                [r * a.cos(), r * a.sin()]
            });
            vec![Path { pts: pts.collect(), closed: true }]
        }
        "Arrow" => vec![poly(&[[-1.0, -0.25], [0.3, -0.25], [0.3, -0.6], [1.0, 0.0], [0.3, 0.6], [0.3, 0.25], [-1.0, 0.25]])],
        "Double arrow" => vec![poly(&[[-1.0, 0.0], [-0.45, -0.55], [-0.45, -0.22], [0.45, -0.22], [0.45, -0.55], [1.0, 0.0], [0.45, 0.55], [0.45, 0.22], [-0.45, 0.22], [-0.45, 0.55]])],
        "Cross" => vec![poly(&[[-0.3, -1.0], [0.3, -1.0], [0.3, -0.3], [1.0, -0.3], [1.0, 0.3], [0.3, 0.3], [0.3, 1.0], [-0.3, 1.0], [-0.3, 0.3], [-1.0, 0.3], [-1.0, -0.3], [-0.3, -0.3]])],
        "Ring" => vec![circle([0.0, 0.0], 1.0), circle([0.0, 0.0], 0.65)],
        "Moon" => subtract(vec![circle([0.0, 0.0], 1.0)], vec![circle([0.45, 0.25], 0.85)]),
        "Lightning" => vec![poly(&[[0.15, 1.0], [-0.55, -0.05], [-0.05, -0.05], [-0.25, -1.0], [0.55, 0.15], [0.05, 0.15], [0.35, 1.0]])],
        "House" => vec![poly(&[[-0.75, -1.0], [0.75, -1.0], [0.75, 0.15], [1.0, 0.15], [0.0, 1.0], [-1.0, 0.15], [-0.75, 0.15]]), poly(&[[-0.2, -1.0], [0.2, -1.0], [0.2, -0.4], [-0.2, -0.4]])],
        "Speech bubble" => union_paths(vec![rect_path([-1.0, -0.4], [1.0, 0.9], 0.3), poly(&[[-0.5, -0.3], [-0.1, -0.3], [-0.65, -1.0]])]),
        "Shield" => {
            let mut pts = vec![[-0.85, 0.95], [0.0, 1.0], [0.85, 0.95]];
            for i in 0..=20 {
                let t = i as f64 / 20.0;
                let y = 0.95 - 1.95 * t;
                let w = 0.85 * (1.0 - t.powf(2.2));
                let _ = w;
                pts.push([0.85 * (1.0 - t * t), y]);
            }
            for i in (0..=20).rev() {
                let t = i as f64 / 20.0;
                pts.push([-0.85 * (1.0 - t * t), 0.95 - 1.95 * t]);
            }
            pts.dedup_by(|a, b| dist(*a, *b) < 1e-9);
            vec![Path { pts, closed: true }]
        }
        "Leaf" => {
            let mut pts = vec![];
            for i in 0..=30 {
                let t = PI * i as f64 / 30.0;
                pts.push([-t.cos(), 0.5 * t.sin()]);
            }
            for i in 1..30 {
                let t = PI * i as f64 / 30.0;
                pts.push([t.cos(), -0.5 * t.sin()]);
            }
            vec![Path { pts, closed: true }]
        }
        "Teardrop" => {
            let n = 80;
            let pts = (0..n)
                .map(|i| {
                    let t = TAU * i as f64 / n as f64;
                    [0.7 * t.sin() * (t / 2.0).sin().powi(2) * 1.6, -t.cos()]
                })
                .collect();
            vec![Path { pts, closed: true }]
        }
        "Diamond" => vec![poly(&[[0.0, -1.0], [0.7, 0.0], [0.0, 1.0], [-0.7, 0.0]])],
        "Triangle" => vec![polygon_path([0.0, -0.2], 1.1, 3, false, 0.0)],
        "Octagon" => vec![polygon_path([0.0, 0.0], 1.0, 8, false, PI / 8.0)],
        "Cloud" => union_paths(vec![
            circle([-0.55, -0.15], 0.42),
            circle([-0.1, 0.2], 0.55),
            circle([0.45, 0.05], 0.48),
            circle([0.7, -0.25], 0.3),
            rect_path([-0.55, -0.57], [0.7, -0.15], 0.0),
        ]),
        "Paw" => {
            let mut v = vec![ellipse_path([0.0, -0.4], 0.55, 0.45)];
            for (x, y) in [(-0.75, 0.2), (-0.3, 0.65), (0.3, 0.65), (0.75, 0.2)] {
                v.push(ellipse_path([x, y], 0.22, 0.28));
            }
            v
        }
        "Check" => vec![poly(&[[-1.0, 0.05], [-0.7, 0.35], [-0.3, -0.05], [0.7, 0.95], [1.0, 0.65], [-0.3, -0.65]])],
        "Tree" => vec![poly(&[
            [-0.12, -1.0],
            [0.12, -1.0],
            [0.12, -0.7],
            [0.8, -0.7],
            [0.35, -0.15],
            [0.6, -0.15],
            [0.25, 0.4],
            [0.42, 0.4],
            [0.0, 1.0],
            [-0.42, 0.4],
            [-0.25, 0.4],
            [-0.6, -0.15],
            [-0.35, -0.15],
            [-0.8, -0.7],
            [-0.12, -0.7],
        ])],
        "Bone" => union_paths(vec![
            rect_path([-0.75, -0.18], [0.75, 0.18], 0.0),
            circle([-0.78, 0.2], 0.24),
            circle([-0.78, -0.2], 0.24),
            circle([0.78, 0.2], 0.24),
            circle([0.78, -0.2], 0.24),
        ]),
        "Gear" => {
            let mut g = gear(12, 1.0 / 7.0, 20.0, 0.0);
            g.push(circle([0.0, 0.0], 0.25));
            g
        }
        _ => vec![circle([0.0, 0.0], 1.0)],
    };
    // normalise to the requested size
    let Some((mn, mx)) = crate::model::bbox_of(raw.iter().flat_map(|p| p.pts.iter())) else { return raw };
    let k = size / (mx[0] - mn[0]).max(mx[1] - mn[1]).max(1e-9);
    let c = [(mn[0] + mx[0]) / 2.0, (mn[1] + mx[1]) / 2.0];
    let _ = s;
    raw.into_iter().map(|mut p| {
        for q in &mut p.pts {
            *q = [(q[0] - c[0]) * k, (q[1] - c[1]) * k];
        }
        p
    }).collect()
}

// ---------------------------------------------------------------- makers

/// Involute spur gear outline centered on the origin (plus a bore when `bore` > 0).
pub fn gear(teeth: u32, module: f64, pressure_deg: f64, bore: f64) -> Vec<Path> {
    let n = teeth.max(5) as f64;
    let m = module.max(0.01);
    let alpha = pressure_deg.clamp(10.0, 35.0).to_radians();
    let rp = m * n / 2.0;
    let rb = rp * alpha.cos();
    let ra = rp + m;
    let rf = (rp - 1.25 * m).max(0.1);
    let inv = |a: f64| a.tan() - a;
    let half = PI / (2.0 * n) + inv(alpha);
    let flank_r0 = rb.max(rf);
    let steps = 10;
    let mut pts: Vec<P> = vec![];
    let polar = |r: f64, a: f64| [r * a.cos(), r * a.sin()];
    for k in 0..teeth.max(5) {
        let c = TAU * k as f64 / n;
        // root before the tooth
        let a_root = c - half - if rf < rb { 0.0 } else { 0.0 };
        pts.push(polar(rf, a_root - (PI / n - half) * 0.5));
        if rf < rb {
            pts.push(polar(rf, a_root));
        }
        // rising flank
        for i in 0..=steps {
            let r = flank_r0 + (ra - flank_r0) * i as f64 / steps as f64;
            let t = ((r / rb).powi(2) - 1.0).max(0.0).sqrt();
            let phi = t - t.atan();
            pts.push(polar(r, c - half + phi));
        }
        // falling flank
        for i in (0..=steps).rev() {
            let r = flank_r0 + (ra - flank_r0) * i as f64 / steps as f64;
            let t = ((r / rb).powi(2) - 1.0).max(0.0).sqrt();
            let phi = t - t.atan();
            pts.push(polar(r, c + half - phi));
        }
        if rf < rb {
            pts.push(polar(rf, c + half));
        }
        pts.push(polar(rf, c + half + (PI / n - half) * 0.5));
    }
    pts.dedup_by(|a, b| dist(*a, *b) < 1e-9);
    let mut out = vec![Path { pts, closed: true }];
    if bore > 0.0 {
        out.push(circle([0.0, 0.0], bore / 2.0));
    }
    out
}

#[derive(Clone, Copy, PartialEq)]
enum Edge {
    Straight,
    /// finger joint; `ends_out` = the end fingers stick out
    Fingers(bool),
}

/// One panel outline (w × h) with finger joints on its edges, front-left corner at `at`.
fn panel(w: f64, h: f64, t: f64, finger: f64, edges: [Edge; 4], at: P) -> Path {
    // edges: bottom, right, top, left
    let count = |l: f64| {
        let mut n = (l / finger.max(t)).round().max(3.0) as i64;
        if n % 2 == 0 {
            n -= 1;
        }
        n.max(3) as usize
    };
    let inset_at_ends = |e: Edge| match e {
        Edge::Fingers(out) => {
            if out {
                0.0
            } else {
                t
            }
        }
        Edge::Straight => 0.0,
    };
    let depth = |e: Edge, i: usize| match e {
        Edge::Straight => 0.0,
        Edge::Fingers(out) => {
            let outward = (i % 2 == 0) == out;
            if outward {
                0.0
            } else {
                t
            }
        }
    };
    let mut pts: Vec<P> = vec![];
    // generic edge along a local axis: start/end corners clipped by the neighbours' insets
    let mut edge = |len: f64, e: Edge, start_clip: f64, end_clip: f64, map: &dyn Fn(f64, f64) -> P| {
        let n = if let Edge::Fingers(_) = e { count(len) } else { 1 };
        let seg = len / n as f64;
        for i in 0..n {
            let a = (i as f64 * seg).max(start_clip);
            let b = ((i + 1) as f64 * seg).min(len - end_clip);
            let d = depth(e, i);
            pts.push(map(a, d));
            pts.push(map(b, d));
        }
    };
    let [eb, er, et, el] = edges;
    edge(w, eb, inset_at_ends(el), inset_at_ends(er), &|s, d| [s, d]);
    edge(h, er, inset_at_ends(eb), inset_at_ends(et), &|s, d| [w - d, s]);
    edge(w, et, inset_at_ends(er), inset_at_ends(el), &|s, d| [w - s, h - d]);
    edge(h, el, inset_at_ends(et), inset_at_ends(eb), &|s, d| [d, h - s]);
    pts.dedup_by(|a, b| dist(*a, *b) < 1e-9);
    if pts.len() > 1 && dist(pts[0], *pts.last().unwrap()) < 1e-9 {
        pts.pop();
    }
    let pts = pts.into_iter().map(|p| [p[0] + at[0], p[1] + at[1]]).collect();
    Path { pts, closed: true }
}

/// Flat-pack finger-jointed box (outside sizes). Returns one entry per panel.
pub fn finger_box(w: f64, d: f64, h: f64, t: f64, finger: f64, lid: bool) -> Vec<Vec<Path>> {
    use Edge::*;
    let gap = 6.0;
    let top_edge = if lid { Fingers(false) } else { Straight };
    let mut out = vec![];
    out.push(vec![panel(w, d, t, finger, [Fingers(true); 4], [0.0, 0.0])]);
    let front = [Fingers(false), Fingers(true), top_edge, Fingers(true)];
    let side = [Fingers(false), Fingers(false), top_edge, Fingers(false)];
    out.push(vec![panel(w, h, t, finger, front, [0.0, d + gap])]);
    out.push(vec![panel(w, h, t, finger, front, [0.0, d + h + 2.0 * gap])]);
    out.push(vec![panel(d, h, t, finger, side, [w + gap, d + gap])]);
    out.push(vec![panel(d, h, t, finger, side, [w + gap, d + h + 2.0 * gap])]);
    if lid {
        out.push(vec![panel(w, d, t, finger, [Fingers(true); 4], [w + gap, 0.0])]);
    }
    out
}

/// Living hinge: staggered slots inside a w × h area (open cut lines).
pub fn living_hinge(w: f64, h: f64, slot: f64, gap: f64, spacing: f64) -> Vec<Path> {
    let mut out = vec![];
    let slot = slot.max(1.0);
    let gap = gap.max(0.5);
    let spacing = spacing.max(0.5);
    let mut x = 0.0;
    let mut col = 0;
    while x <= w + 1e-9 {
        let mut y = if col % 2 == 0 { -slot / 2.0 } else { 0.0 };
        while y < h {
            let a = y.max(0.0);
            let b = (y + slot).min(h);
            if b - a > 0.5 {
                out.push(Path { pts: vec![[x, a], [x, b]], closed: false });
            }
            y += slot + gap;
        }
        x += spacing;
        col += 1;
    }
    out
}

fn rng(seed: &mut u64) -> f64 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    (*seed % 10_000) as f64 / 10_000.0
}

pub fn puzzle_edge(a: P, b: P, flip: bool, seed: &mut u64) -> Vec<P> {
    // knob profile in local coordinates (u along, v across), as cubic Bezier segments
    let j = (rng(seed) - 0.5) * 0.06;
    let s = if flip { -1.0 } else { 1.0 };
    let ctrl: [[f64; 2]; 19] = [
        [0.0, 0.0],
        [0.2, 0.0],
        [0.38 + j, 0.02],
        [0.40 + j, 0.0],
        [0.42 + j, -0.02],
        [0.32 + j, 0.12],
        [0.38 + j, 0.2],
        [0.44 + j, 0.28],
        [0.56 + j, 0.28],
        [0.62 + j, 0.2],
        [0.68 + j, 0.12],
        [0.58 + j, -0.02],
        [0.60 + j, 0.0],
        [0.62 + j, 0.02],
        [0.8, 0.0],
        [1.0, 0.0],
        [1.0, 0.0],
        [1.0, 0.0],
        [1.0, 0.0],
    ];
    let d = [b[0] - a[0], b[1] - a[1]];
    let n = [-d[1], d[0]];
    let map = |q: [f64; 2]| [a[0] + d[0] * q[0] + n[0] * q[1] * s, a[1] + d[1] * q[0] + n[1] * q[1] * s];
    let mut pts = vec![map(ctrl[0])];
    let mut i = 0;
    while i + 3 < 16 {
        let p0 = *pts.last().unwrap();
        geom::cubic(&mut pts, p0, map(ctrl[i + 1]), map(ctrl[i + 2]), map(ctrl[i + 3]));
        i += 3;
    }
    pts.push(b);
    pts.dedup_by(|x, y| dist(*x, *y) < 1e-9);
    pts
}

/// Jigsaw puzzle: an outer frame plus interlocking inner cut lines.
pub fn puzzle(w: f64, h: f64, cols: u32, rows: u32, seed: u64) -> Vec<Path> {
    let (cols, rows) = (cols.max(1), rows.max(1));
    let mut sd = seed.max(1) * 2654435761 + 1;
    let mut out = vec![rect_path([0.0, 0.0], [w, h], 0.0)];
    let cw = w / cols as f64;
    let rh = h / rows as f64;
    for c in 1..cols {
        let x = c as f64 * cw;
        let mut pts: Vec<P> = vec![];
        for r in 0..rows {
            let flip = rng(&mut sd) > 0.5;
            let seg = puzzle_edge([x, r as f64 * rh], [x, (r + 1) as f64 * rh], flip, &mut sd);
            if pts.is_empty() {
                pts.extend(seg);
            } else {
                pts.extend(seg.into_iter().skip(1));
            }
        }
        out.push(Path { pts, closed: false });
    }
    for r in 1..rows {
        let y = r as f64 * rh;
        let mut pts: Vec<P> = vec![];
        for c in 0..cols {
            let flip = rng(&mut sd) > 0.5;
            let seg = puzzle_edge([c as f64 * cw, y], [(c + 1) as f64 * cw, y], flip, &mut sd);
            if pts.is_empty() {
                pts.extend(seg);
            } else {
                pts.extend(seg.into_iter().skip(1));
            }
        }
        out.push(Path { pts, closed: false });
    }
    out
}

/// Sign blank: rounded plate with a mounting hole in each corner (holes as circles).
pub fn sign_blank(w: f64, h: f64, corner: f64, hole_d: f64, inset: f64) -> (Vec<Path>, Vec<Path>) {
    let plate = vec![rect_path([0.0, 0.0], [w, h], corner)];
    let mut holes = vec![];
    if hole_d > 0.0 {
        for c in [[inset, inset], [w - inset, inset], [w - inset, h - inset], [inset, h - inset]] {
            holes.push(circle(c, hole_d / 2.0));
        }
    }
    (plate, holes)
}

/// Holes evenly spaced on a circle.
pub fn bolt_circle(center: P, radius: f64, count: u32, hole_d: f64, start_deg: f64) -> Vec<Path> {
    (0..count.max(1))
        .map(|i| {
            let a = (start_deg + 360.0 * i as f64 / count.max(1) as f64).to_radians();
            let c = [center[0] + radius * a.cos(), center[1] + radius * a.sin()];
            if hole_d > 0.0 {
                circle(c, hole_d / 2.0)
            } else {
                Path { pts: vec![c], closed: false }
            }
        })
        .collect()
}

/// A grid of holes.
pub fn hole_grid(origin: P, cols: u32, rows: u32, dx: f64, dy: f64, hole_d: f64) -> Vec<Path> {
    let mut v = vec![];
    for r in 0..rows.max(1) {
        for c in 0..cols.max(1) {
            let p = [origin[0] + c as f64 * dx, origin[1] + r as f64 * dy];
            v.push(if hole_d > 0.0 { circle(p, hole_d / 2.0) } else { Path { pts: vec![p], closed: false } });
        }
    }
    v
}

// ---------------------------------------------------------------- path tools

/// Rounds (or chamfers) every sharp corner of a path.
pub fn round_corners(p: &Path, r: f64, chamfer: bool) -> Path {
    let n = p.pts.len();
    if n < 3 || r <= 0.0 {
        return p.clone();
    }
    let mut out: Vec<P> = vec![];
    for i in 0..n {
        let v = p.pts[i];
        if !p.closed && (i == 0 || i == n - 1) {
            out.push(v);
            continue;
        }
        let a = p.pts[(i + n - 1) % n];
        let b = p.pts[(i + 1) % n];
        let (la, lb) = (dist(a, v), dist(v, b));
        if la < 1e-9 || lb < 1e-9 {
            out.push(v);
            continue;
        }
        let d1 = [(v[0] - a[0]) / la, (v[1] - a[1]) / la];
        let d2 = [(b[0] - v[0]) / lb, (b[1] - v[1]) / lb];
        let cosang = (d1[0] * d2[0] + d1[1] * d2[1]).clamp(-1.0, 1.0);
        let turn = cosang.acos();
        if turn < 20f64.to_radians() || turn > 175f64.to_radians() {
            out.push(v);
            continue;
        }
        let theta = PI - turn; // interior angle
        let mut tdist = r / (theta / 2.0).tan();
        let lim = la.min(lb) * 0.5;
        let mut rr = r;
        if tdist > lim {
            tdist = lim;
            rr = tdist * (theta / 2.0).tan();
        }
        let p1 = [v[0] - d1[0] * tdist, v[1] - d1[1] * tdist];
        let p2 = [v[0] + d2[0] * tdist, v[1] + d2[1] * tdist];
        if chamfer {
            out.push(p1);
            out.push(p2);
            continue;
        }
        let bis = [-d1[0] + d2[0], -d1[1] + d2[1]];
        let bl = (bis[0] * bis[0] + bis[1] * bis[1]).sqrt().max(1e-12);
        let cd = rr / (theta / 2.0).sin();
        let c = [v[0] + bis[0] / bl * cd, v[1] + bis[1] / bl * cd];
        let a0 = (p1[1] - c[1]).atan2(p1[0] - c[0]);
        let mut a1 = (p2[1] - c[1]).atan2(p2[0] - c[0]);
        let cross = d1[0] * d2[1] - d1[1] * d2[0];
        if cross > 0.0 {
            while a1 < a0 {
                a1 += TAU;
            }
        } else {
            while a1 > a0 {
                a1 -= TAU;
            }
        }
        let steps = (((a1 - a0).abs() / 10f64.to_radians()).ceil() as usize).max(2);
        for k in 0..=steps {
            let ang = a0 + (a1 - a0) * k as f64 / steps as f64;
            out.push([c[0] + rr * ang.cos(), c[1] + rr * ang.sin()]);
        }
    }
    Path { pts: out, closed: p.closed }
}

/// Joins open paths whose ends meet (within `tol`) into longer paths, closing loops.
pub fn join_paths(paths: Vec<Path>, tol: f64) -> Vec<Path> {
    let (mut open, mut done): (Vec<Path>, Vec<Path>) = paths.into_iter().partition(|p| !p.closed && p.pts.len() >= 2);
    while let Some(mut cur) = open.pop() {
        loop {
            let end = *cur.pts.last().unwrap();
            let start = cur.pts[0];
            let mut found = None;
            for (i, o) in open.iter().enumerate() {
                let (os, oe) = (o.pts[0], *o.pts.last().unwrap());
                if dist(end, os) < tol {
                    found = Some((i, false, false));
                } else if dist(end, oe) < tol {
                    found = Some((i, true, false));
                } else if dist(start, oe) < tol {
                    found = Some((i, false, true));
                } else if dist(start, os) < tol {
                    found = Some((i, true, true));
                }
                if found.is_some() {
                    break;
                }
            }
            let Some((i, rev, prepend)) = found else { break };
            let mut o = open.remove(i);
            if rev {
                o.pts.reverse();
            }
            if prepend {
                o.pts.pop();
                o.pts.extend(cur.pts);
                cur.pts = o.pts;
            } else {
                cur.pts.extend(o.pts.into_iter().skip(1));
            }
        }
        if cur.pts.len() > 2 && dist(cur.pts[0], *cur.pts.last().unwrap()) < tol {
            cur.pts.pop();
            cur.closed = true;
        }
        done.push(cur);
    }
    done
}

pub fn path_key(paths: &[Path]) -> String {
    let mut s = String::new();
    for p in paths {
        for q in &p.pts {
            s.push_str(&format!("{:.3},{:.3};", q[0], q[1]));
        }
        s.push('|');
    }
    s
}

/// Bends a set of paths so their horizontal baseline follows a circle of radius `r`
/// (positive: text arcs over the top; negative: arcs underneath).
pub fn bend_on_circle(paths: &mut [Path], r: f64) {
    let Some((mn, mx)) = crate::model::bbox_of(paths.iter().flat_map(|p| p.pts.iter())) else { return };
    if r.abs() < 1.0 {
        return;
    }
    let cx = (mn[0] + mx[0]) / 2.0;
    let base = mn[1];
    let center = [cx, base - r];
    for p in paths.iter_mut() {
        // densify long straight segments so they bend smoothly
        let mut dense = vec![];
        let n = p.pts.len();
        for i in 0..n {
            let a = p.pts[i];
            dense.push(a);
            if i + 1 < n || p.closed {
                let b = p.pts[(i + 1) % n];
                let k = (dist(a, b) / 1.0).floor() as usize;
                for j in 1..k {
                    let t = j as f64 / k as f64;
                    dense.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
                }
            }
        }
        p.pts = dense
            .into_iter()
            .map(|q| {
                let ang = (q[0] - cx) / r;
                let rad = r + (q[1] - base);
                [center[0] + rad * ang.sin(), center[1] + rad * ang.cos()]
            })
            .collect();
        if r < 0.0 {
            // keep winding consistent after the mirror-like bend
            if signed_area(&p.pts).abs() > 0.0 {}
        }
    }
}

/// QR code as filled squares (one closed square per dark module, merged).
pub fn qr_code(text: &str, size: f64) -> Result<Vec<Path>, String> {
    let code = qrcode::QrCode::new(text.as_bytes()).map_err(|e| format!("Could not make a QR code: {e}"))?;
    let n = code.width();
    let colors = code.to_colors();
    let cell = size / n as f64;
    let mut squares = vec![];
    for y in 0..n {
        let mut x = 0;
        while x < n {
            if colors[y * n + x] == qrcode::Color::Dark {
                let x0 = x;
                while x < n && colors[y * n + x] == qrcode::Color::Dark {
                    x += 1;
                }
                let yy = (n - 1 - y) as f64 * cell;
                squares.push(rect_path([x0 as f64 * cell, yy], [x as f64 * cell, yy + cell], 0.0));
            } else {
                x += 1;
            }
        }
    }
    Ok(union_paths(squares))
}
