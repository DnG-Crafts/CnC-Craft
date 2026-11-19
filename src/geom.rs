//! Geometry helpers: shape builders, transforms, hit tests and polygon maths.

use crate::model::{Path, Shape, P};
use geo::{BooleanOps, Buffer, LineString, MultiPolygon, Polygon};

pub fn dist(a: P, b: P) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

pub fn rect_path(a: P, b: P, corner: f64) -> Path {
    let (x0, x1) = (a[0].min(b[0]), a[0].max(b[0]));
    let (y0, y1) = (a[1].min(b[1]), a[1].max(b[1]));
    let r = corner.min((x1 - x0) / 2.0).min((y1 - y0) / 2.0).max(0.0);
    if r <= 0.001 {
        return Path { pts: vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]], closed: true };
    }
    let mut pts = vec![];
    let seg = 8;
    let corners = [([x1 - r, y0 + r], -90.0), ([x1 - r, y1 - r], 0.0), ([x0 + r, y1 - r], 90.0), ([x0 + r, y0 + r], 180.0)];
    for (c, start) in corners {
        for i in 0..=seg {
            let a = (start + 90.0 * i as f64 / seg as f64).to_radians();
            pts.push([c[0] + r * a.cos(), c[1] + r * a.sin()]);
        }
    }
    Path { pts, closed: true }
}

pub fn ellipse_path(c: P, rx: f64, ry: f64) -> Path {
    let n = (((rx.abs() + ry.abs()) * 1.2) as usize).clamp(24, 180);
    let pts = (0..n)
        .map(|i| {
            let a = std::f64::consts::TAU * i as f64 / n as f64;
            [c[0] + rx * a.cos(), c[1] + ry * a.sin()]
        })
        .collect();
    Path { pts, closed: true }
}

pub fn polygon_path(c: P, r: f64, sides: u32, star: bool, rot: f64) -> Path {
    let sides = sides.max(3);
    let mut pts = vec![];
    let n = if star { sides * 2 } else { sides };
    for i in 0..n {
        let a = rot + std::f64::consts::FRAC_PI_2 + std::f64::consts::TAU * i as f64 / n as f64;
        let rr = if star && i % 2 == 1 { r * 0.45 } else { r };
        pts.push([c[0] + rr * a.cos(), c[1] + rr * a.sin()]);
    }
    Path { pts, closed: true }
}

#[allow(dead_code)]
pub fn path_len(p: &Path) -> f64 {
    let mut l = 0.0;
    for w in p.pts.windows(2) {
        l += dist(w[0], w[1]);
    }
    if p.closed && p.pts.len() > 2 {
        l += dist(*p.pts.last().unwrap(), p.pts[0]);
    }
    l
}

pub fn signed_area(pts: &[P]) -> f64 {
    let mut a = 0.0;
    for i in 0..pts.len() {
        let p = pts[i];
        let q = pts[(i + 1) % pts.len()];
        a += p[0] * q[1] - q[0] * p[1];
    }
    a / 2.0
}

pub fn seg_dist(p: P, a: P, b: P) -> f64 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let l2 = dx * dx + dy * dy;
    let t = if l2 < 1e-12 { 0.0 } else { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) };
    dist(p, [a[0] + t * dx, a[1] + t * dy])
}

pub fn point_in_poly(p: P, pts: &[P]) -> bool {
    let mut inside = false;
    let n = pts.len();
    if n < 3 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (pts[i], pts[j]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Distance from a point to the outline of a shape; zero when inside a closed shape.
#[allow(dead_code)]
pub fn shape_hit(shape: &Shape, p: P, tol: f64) -> bool {
    let mut inside = false;
    for path in &shape.paths {
        if path.pts.len() == 1 && dist(path.pts[0], p) < tol * 2.0 {
            return true;
        }
        for w in path.pts.windows(2) {
            if seg_dist(p, w[0], w[1]) < tol {
                return true;
            }
        }
        if path.closed && path.pts.len() > 2 {
            if seg_dist(p, *path.pts.last().unwrap(), path.pts[0]) < tol {
                return true;
            }
            if point_in_poly(p, &path.pts) {
                inside = !inside;
            }
        }
    }
    inside
}

/// Shortest distance from a point to any outline (or single point) of a shape.
pub fn outline_dist(shape: &Shape, p: P) -> f64 {
    let mut best = f64::MAX;
    for path in &shape.paths {
        if path.pts.len() == 1 {
            best = best.min(dist(path.pts[0], p));
        }
        for w in path.pts.windows(2) {
            best = best.min(seg_dist(p, w[0], w[1]));
        }
        if path.closed && path.pts.len() > 2 {
            best = best.min(seg_dist(p, *path.pts.last().unwrap(), path.pts[0]));
        }
    }
    best
}

/// True when the point is inside the shape's closed outlines (holes count as outside).
pub fn inside_shape(shape: &Shape, p: P) -> bool {
    let mut inside = false;
    for path in &shape.paths {
        if path.closed && path.pts.len() > 2 && point_in_poly(p, &path.pts) {
            inside = !inside;
        }
    }
    inside
}

pub fn transform_shape(s: &mut Shape, f: impl Fn(P) -> P) {
    for path in &mut s.paths {
        for p in &mut path.pts {
            *p = f(*p);
        }
    }
    // live effects move, turn and scale with the shape
    if let Some(fx) = s.fx.as_mut() {
        for path in fx.base.iter_mut().chain(fx.cached.iter_mut()) {
            for p in &mut path.pts {
                *p = f(*p);
            }
        }
    }
}

pub fn translate(s: &mut Shape, d: P) {
    transform_shape(s, |p| [p[0] + d[0], p[1] + d[1]]);
}

pub fn rotate_about(s: &mut Shape, c: P, deg: f64) {
    let (sn, cs) = deg.to_radians().sin_cos();
    transform_shape(s, |p| {
        let x = p[0] - c[0];
        let y = p[1] - c[1];
        [c[0] + x * cs - y * sn, c[1] + x * sn + y * cs]
    });
}

pub fn scale_about(s: &mut Shape, c: P, sx: f64, sy: f64) {
    transform_shape(s, |p| [c[0] + (p[0] - c[0]) * sx, c[1] + (p[1] - c[1]) * sy]);
}

// ---------- geo conversions ----------

fn ring(pts: &[P]) -> LineString<f64> {
    let mut v: Vec<(f64, f64)> = pts.iter().map(|p| (p[0], p[1])).collect();
    if let (Some(f), Some(l)) = (v.first().copied(), v.last().copied()) {
        if f != l {
            v.push(f);
        }
    }
    LineString::from(v)
}

/// Builds an even-odd region from every closed path of the given shapes.
pub fn region_of<'a>(shapes: impl Iterator<Item = &'a Shape>) -> MultiPolygon<f64> {
    let mut mp = MultiPolygon::<f64>::new(vec![]);
    for s in shapes {
        for p in &s.paths {
            if !p.closed || p.pts.len() < 3 || signed_area(&p.pts).abs() < 1e-6 {
                continue;
            }
            let poly = Polygon::new(ring(&p.pts), vec![]);
            let single = MultiPolygon::new(vec![poly]);
            mp = if mp.0.is_empty() { single } else { mp.xor(&single) };
        }
    }
    mp
}

pub fn offset_region(mp: &MultiPolygon<f64>, d: f64) -> MultiPolygon<f64> {
    if d.abs() < 1e-9 {
        return mp.clone();
    }
    mp.buffer(d)
}

/// Rings of a region: (points without closing duplicate, is_exterior).
pub fn rings_of(mp: &MultiPolygon<f64>) -> Vec<(Vec<P>, bool)> {
    let mut out = vec![];
    for poly in &mp.0 {
        let conv = |ls: &LineString<f64>| {
            let mut v: Vec<P> = ls.0.iter().map(|c| [c.x, c.y]).collect();
            if v.len() > 1 && v.first() == v.last() {
                v.pop();
            }
            v
        };
        let e = conv(poly.exterior());
        if e.len() >= 3 {
            out.push((e, true));
        }
        for i in poly.interiors() {
            let v = conv(i);
            if v.len() >= 3 {
                out.push((v, false));
            }
        }
    }
    out
}

pub fn region_to_paths(mp: &MultiPolygon<f64>) -> Vec<Path> {
    rings_of(mp).into_iter().map(|(pts, _)| Path { pts, closed: true }).collect()
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BoolOp {
    Union,
    Difference,
    Intersection,
    Xor,
}

pub fn boolean(a: &MultiPolygon<f64>, b: &MultiPolygon<f64>, op: BoolOp) -> MultiPolygon<f64> {
    match op {
        BoolOp::Union => a.union(b),
        BoolOp::Difference => a.difference(b),
        BoolOp::Intersection => a.intersection(b),
        BoolOp::Xor => a.xor(b),
    }
}

/// Simplifies a polyline by dropping points closer than `tol` to the line between neighbours.
#[allow(dead_code)]
pub fn simplify(pts: &[P], tol: f64) -> Vec<P> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let mut out = vec![pts[0]];
    for i in 1..pts.len() - 1 {
        let a = *out.last().unwrap();
        if seg_dist(pts[i], a, pts[i + 1]) > tol || dist(a, pts[i]) > 50.0 {
            out.push(pts[i]);
        }
    }
    out.push(*pts.last().unwrap());
    out
}

/// Flattens a cubic Bezier into points (excluding the start point).
pub fn cubic(out: &mut Vec<P>, p0: P, p1: P, p2: P, p3: P) {
    let len = dist(p0, p1) + dist(p1, p2) + dist(p2, p3);
    let n = ((len / 0.5) as usize).clamp(4, 64);
    for i in 1..=n {
        let t = i as f64 / n as f64;
        let u = 1.0 - t;
        let x = u * u * u * p0[0] + 3.0 * u * u * t * p1[0] + 3.0 * u * t * t * p2[0] + t * t * t * p3[0];
        let y = u * u * u * p0[1] + 3.0 * u * u * t * p1[1] + 3.0 * u * t * t * p2[1] + t * t * t * p3[1];
        out.push([x, y]);
    }
}

pub fn quad(out: &mut Vec<P>, p0: P, p1: P, p2: P) {
    let len = dist(p0, p1) + dist(p1, p2);
    let n = ((len / 0.5) as usize).clamp(3, 48);
    for i in 1..=n {
        let t = i as f64 / n as f64;
        let u = 1.0 - t;
        out.push([u * u * p0[0] + 2.0 * u * t * p1[0] + t * t * p2[0], u * u * p0[1] + 2.0 * u * t * p1[1] + t * t * p2[1]]);
    }
}
