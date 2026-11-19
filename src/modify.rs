//! Object-modifying design tools working on CncCraft's shape format (polylines in
//! millimetres, Y up): warps, smoothing, hatch, lattice, knockout,
//! inlay, contour, stencil bridges, shadows/extrude, radial repeat, blend, clip, knife,
//! copy along path, fit into, dock, convex hull, oriented bounding box and stone/hole templates.

use crate::geom::{self, dist, BoolOp};
use crate::model::{bbox_of, Path, Shape, ShapeKind, P};
use geo::{BooleanOps, LineString, MultiLineString};
use std::f64::consts::{PI, TAU};

fn temp_shape(paths: &[Path]) -> Shape {
    Shape { id: 0, kind: ShapeKind::Path, paths: paths.to_vec(), group: None, text: None, fx: None }
}

/// The closed parts of `paths` as one even-odd region.
pub fn region(paths: &[Path]) -> geo::MultiPolygon<f64> {
    geom::region_of(std::iter::once(&temp_shape(paths)))
}

/// Several shapes' regions merged into one.
pub fn merged(parts: &[Vec<Path>]) -> geo::MultiPolygon<f64> {
    let mut acc = geo::MultiPolygon::new(vec![]);
    for p in parts {
        let r = region(p);
        if !r.0.is_empty() {
            acc = if acc.0.is_empty() { r } else { geom::boolean(&acc, &r, BoolOp::Union) };
        }
    }
    acc
}

fn to_paths(mp: &geo::MultiPolygon<f64>) -> Vec<Path> {
    geom::region_to_paths(mp)
}

fn bounds(paths: &[Path]) -> Option<(P, P)> {
    bbox_of(paths.iter().flat_map(|p| p.pts.iter()))
}

fn circle(c: P, r: f64) -> Path {
    geom::ellipse_path(c, r, r)
}

pub fn densify(pts: &[P], closed: bool, max: f64) -> Vec<P> {
    let max = max.max(0.05);
    let n = pts.len();
    let mut out = Vec::with_capacity(n * 2);
    for i in 0..n {
        out.push(pts[i]);
        if i + 1 < n || closed {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            let k = (dist(a, b) / max).ceil() as usize;
            for j in 1..k {
                let t = j as f64 / k as f64;
                out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
            }
        }
    }
    out
}

// ---------------------------------------------------------------- warp

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Warp {
    Arch,
    Bulge,
    Taper,
    Wave,
    Bridge,
    Flag,
    Barrel,
    TunaCan,
    TitleCrawl,
    Rays,
    Rotate3d,
}

impl Warp {
    pub const ALL: [Warp; 11] =
        [Warp::Arch, Warp::Bulge, Warp::Taper, Warp::Wave, Warp::Bridge, Warp::Flag, Warp::Barrel, Warp::TunaCan, Warp::TitleCrawl, Warp::Rays, Warp::Rotate3d];
    pub fn label(self) -> &'static str {
        match self {
            Warp::Arch => "Arch",
            Warp::Bulge => "Bulge",
            Warp::Taper => "Taper / perspective",
            Warp::Wave => "Wave",
            Warp::Bridge => "Bridge",
            Warp::Flag => "Flag",
            Warp::Barrel => "Barrel (fisheye)",
            Warp::TunaCan => "Tuna can",
            Warp::TitleCrawl => "Title crawl",
            Warp::Rays => "Rays",
            Warp::Rotate3d => "3D rotate",
        }
    }

    /// Where a point goes, in screen-style coordinates (y grows downwards) inside `area`
    /// (x0, y0 = top-left; x1, y1 = bottom-right).
    fn map(self, p: P, a: (f64, f64, f64, f64), amount: f64) -> P {
        let (x0, y0, x1, y1) = a;
        let (w, h) = ((x1 - x0).max(1e-9), (y1 - y0).max(1e-9));
        let (u, v) = ((p[0] - x0) / w, (p[1] - y0) / h);
        let c = [(x0 + x1) / 2.0, (y0 + y1) / 2.0];
        match self {
            Warp::Arch => [p[0], p[1] - amount * h * (1.0 - (2.0 * u - 1.0).powi(2))],
            Warp::Bulge => [c[0] + (p[0] - c[0]) * (1.0 + amount * (1.0 - (2.0 * v - 1.0).powi(2))), p[1]],
            Warp::Taper => [c[0] + (p[0] - c[0]) * (1.0 + amount * (1.0 - 2.0 * v)), p[1]],
            Warp::Wave => [p[0], p[1] - amount * h * 0.5 * (TAU * u).sin()],
            Warp::Bridge => [p[0], p[1] - amount * h * (PI * u).sin() * v],
            Warp::Flag => [p[0], p[1] + amount * h * 0.5 * u * (2.0 * PI * 1.5 * u).sin()],
            Warp::Barrel => {
                let half_diag = w.hypot(h) / 2.0;
                let d = [p[0] - c[0], p[1] - c[1]];
                let r = d[0].hypot(d[1]) / half_diag;
                let k = 1.0 + amount * (r * r - 0.33);
                [c[0] + d[0] * k, c[1] + d[1] * k]
            }
            Warp::TunaCan => {
                let wrap = (amount.abs() * PI * 1.6).clamp(0.05, PI * 1.6);
                let r = (w / 2.0) / (wrap / 2.0).sin();
                let phi = (u - 0.5) * wrap;
                [c[0] + r * phi.sin(), p[1] + amount.signum() * (r - r * phi.cos()) * 0.15]
            }
            Warp::TitleCrawl => {
                let top = (1.0 - amount.abs() * 0.9).clamp(0.05, 1.0);
                let scale = top + (1.0 - top) * v.clamp(0.0, 1.0);
                let nv = v.clamp(0.0, 1.0).powf(1.0 + amount.abs());
                [c[0] + (p[0] - c[0]) * scale, y1 - (1.0 - nv) * h]
            }
            Warp::Rays => {
                let spread = (amount.abs() * PI).max(1e-3);
                let r0 = (w / 2.0) / (spread / 2.0).sin();
                let phi = (u - 0.5) * spread * amount.signum();
                let radius = r0 + (p[1] - y0);
                [c[0] + radius * phi.sin(), y0 - r0 + radius * phi.cos()]
            }
            Warp::Rotate3d => {
                let theta = (amount.clamp(-1.0, 1.0) * 80f64.to_radians()).clamp(-1.3, 1.3);
                let focal = 2.0 * w;
                let lx = p[0] - c[0];
                let scale = focal / (focal + lx * theta.sin()).max(1e-3);
                [c[0] + lx * theta.cos() * scale, c[1] + (p[1] - c[1]) * scale]
            }
        }
    }
}

/// Bends `paths` inside `area` (min, max in CncCraft's Y-up coordinates).
pub fn warp(paths: &[Path], area: (P, P), kind: Warp, amount: f64) -> Vec<Path> {
    let (mn, mx) = area;
    // flip to y-down so the effects read naturally (top is "up")
    let a = (mn[0], -mx[1], mx[0], -mn[1]);
    let step = (mx[0] - mn[0]).max(mx[1] - mn[1]) / 200.0;
    paths
        .iter()
        .map(|p| {
            let pts = densify(&p.pts, p.closed, step)
                .into_iter()
                .map(|q| {
                    let m = kind.map([q[0], -q[1]], a, amount);
                    [m[0], -m[1]]
                })
                .collect();
            Path { pts, closed: p.closed }
        })
        .collect()
}

/// Smooths outlines into gentle curves (corner cutting), `passes` times.
pub fn smooth(paths: &[Path], passes: u32) -> Vec<Path> {
    paths
        .iter()
        .map(|p| {
            let mut pts = p.pts.clone();
            for _ in 0..passes.clamp(1, 6) {
                if pts.len() < 3 {
                    break;
                }
                let n = pts.len();
                let mut out = Vec::with_capacity(n * 2);
                if !p.closed {
                    out.push(pts[0]);
                }
                let segs = if p.closed { n } else { n - 1 };
                for i in 0..segs {
                    let (a, b) = (pts[i], pts[(i + 1) % n]);
                    out.push([0.75 * a[0] + 0.25 * b[0], 0.75 * a[1] + 0.25 * b[1]]);
                    out.push([0.25 * a[0] + 0.75 * b[0], 0.25 * a[1] + 0.75 * b[1]]);
                }
                if !p.closed {
                    out.push(pts[n - 1]);
                }
                pts = out;
            }
            Path { pts, closed: p.closed }
        })
        .collect()
}

// ---------------------------------------------------------------- fills and patterns

/// Lines clipped to the inside of a region, as open paths.
pub fn clip_lines(lines: Vec<Vec<P>>, inside: &geo::MultiPolygon<f64>) -> Vec<Path> {
    let mls = MultiLineString::new(lines.into_iter().filter(|l| l.len() >= 2).map(|l| LineString::from(l.into_iter().map(|p| (p[0], p[1])).collect::<Vec<_>>())).collect());
    let clipped = inside.clip(&mls, false);
    clipped
        .0
        .into_iter()
        .map(|ls| Path { pts: ls.0.iter().map(|c| [c.x, c.y]).collect(), closed: false })
        .filter(|p| p.pts.len() >= 2)
        .collect()
}

/// Hatch fill: parallel lines `spacing` mm apart at `angle`° across the shapes (and at right
/// angles too when `cross`), clipped to them, running back and forth.
pub fn hatch(parts: &[Vec<Path>], spacing: f64, angle: f64, cross: bool) -> Vec<Path> {
    let shape = merged(parts);
    let Some((mn, mx)) = bounds(&to_paths(&shape)) else { return vec![] };
    let spacing = spacing.max(0.05);
    let c = [(mn[0] + mx[0]) / 2.0, (mn[1] + mx[1]) / 2.0];
    let r = (mx[0] - mn[0]).hypot(mx[1] - mn[1]) / 2.0 + spacing;
    let mut lines = vec![];
    let mut angles = vec![angle];
    if cross {
        angles.push(angle + 90.0);
    }
    for a in angles {
        let (s, co) = a.to_radians().sin_cos();
        let rot = |p: P| [c[0] + (p[0] - c[0]) * co - (p[1] - c[1]) * s, c[1] + (p[0] - c[0]) * s + (p[1] - c[1]) * co];
        let mut y = -r;
        let mut flip = false;
        while y <= r {
            let (mut p0, mut p1) = ([c[0] - r, c[1] + y], [c[0] + r, c[1] + y]);
            if flip {
                std::mem::swap(&mut p0, &mut p1);
            }
            lines.push(vec![rot(p0), rot(p1)]);
            y += spacing;
            flip = !flip;
        }
    }
    clip_lines(lines, &shape)
}

/// A lattice: square holes on a `cell` mm grid inside the shape, keeping `bar` mm of material
/// everywhere (round the edge too). Returns the outline plus the holes.
pub fn lattice(parts: &[Vec<Path>], cell: f64, bar: f64) -> Vec<Path> {
    let shape = merged(parts);
    let outline = to_paths(&shape);
    let Some((mn, mx)) = bounds(&outline) else { return vec![] };
    let cell = cell.max(bar + 0.5);
    let hole = (cell - bar).max(0.2);
    let inner = geom::offset_region(&shape, -bar);
    let inner_rings = geom::rings_of(&inner);
    let inside = |p: P| {
        let mut ins = false;
        for (r, _) in &inner_rings {
            if geom::point_in_poly(p, r) {
                ins = !ins;
            }
        }
        ins
    };
    let (w, h) = (mx[0] - mn[0], mx[1] - mn[1]);
    let cols = ((w - bar) / cell).floor().max(0.0) as usize;
    let rows = ((h - bar) / cell).floor().max(0.0) as usize;
    let x0 = (mn[0] + mx[0]) / 2.0 - (cols as f64 * cell - bar) / 2.0;
    let y0 = (mn[1] + mx[1]) / 2.0 - (rows as f64 * cell - bar) / 2.0;
    let mut out = outline;
    for row in 0..rows {
        for col in 0..cols {
            let (ax, ay) = (x0 + col as f64 * cell, y0 + row as f64 * cell);
            let corners = [[ax, ay], [ax + hole, ay], [ax + hole, ay + hole], [ax, ay + hole]];
            if corners.iter().chain(std::iter::once(&[ax + hole / 2.0, ay + hole / 2.0])).all(|p| inside(*p)) {
                out.push(Path { pts: corners.to_vec(), closed: true });
            }
        }
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StoneLayout {
    Outline,
    DoubleOutline,
    Fill,
}

impl StoneLayout {
    pub const ALL: [StoneLayout; 3] = [StoneLayout::Outline, StoneLayout::DoubleOutline, StoneLayout::Fill];
    pub fn label(self) -> &'static str {
        match self {
            StoneLayout::Outline => "Along the outline",
            StoneLayout::DoubleOutline => "Two rows along the outline",
            StoneLayout::Fill => "Filled (honeycomb)",
        }
    }
}

fn along(pts: &[P], closed: bool, pitch: f64, out: &mut Vec<P>) {
    let mut pts = pts.to_vec();
    if closed && !pts.is_empty() {
        pts.push(pts[0]);
    }
    let len: f64 = pts.windows(2).map(|w| dist(w[0], w[1])).sum();
    if len < pitch * 0.5 {
        return;
    }
    let n = if closed { (len / pitch).floor().max(1.0) as usize } else { (len / pitch).floor() as usize + 1 };
    let step = if closed { len / n as f64 } else { len / (n.max(2) - 1) as f64 };
    let (mut i, mut walked) = (0, 0.0);
    for k in 0..n {
        let target = k as f64 * step;
        while i + 1 < pts.len() && walked + dist(pts[i], pts[i + 1]) < target {
            walked += dist(pts[i], pts[i + 1]);
            i += 1;
        }
        if i + 1 >= pts.len() {
            break;
        }
        let seg = dist(pts[i], pts[i + 1]).max(1e-12);
        let t = ((target - walked) / seg).clamp(0.0, 1.0);
        out.push([pts[i][0] + (pts[i + 1][0] - pts[i][0]) * t, pts[i][1] + (pts[i + 1][1] - pts[i][1]) * t]);
    }
}

/// Dot/hole template (rhinestone-style): holes `diameter` mm across with `gap` mm
/// between, along the outline or filling it. Returns the hole centres.
pub fn stone_centres(parts: &[Vec<Path>], diameter: f64, gap: f64, layout: StoneLayout) -> Vec<P> {
    if diameter <= 0.0 {
        return vec![];
    }
    let pitch = diameter + gap.max(0.0);
    let r = diameter / 2.0;
    let shape = merged(parts);
    let mut centres = vec![];
    match layout {
        StoneLayout::Outline | StoneLayout::DoubleOutline => {
            for (ring, _) in geom::rings_of(&shape) {
                along(&ring, true, pitch, &mut centres);
            }
            if layout == StoneLayout::DoubleOutline {
                for (ring, _) in geom::rings_of(&geom::offset_region(&shape, -pitch)) {
                    along(&ring, true, pitch, &mut centres);
                }
            }
            for p in parts.iter().flatten().filter(|p| !p.closed) {
                along(&p.pts, false, pitch, &mut centres);
            }
        }
        StoneLayout::Fill => {
            let inside = geom::offset_region(&shape, -r);
            let rings = geom::rings_of(&inside);
            let Some((mn, mx)) = bounds(&to_paths(&inside)) else { return vec![] };
            let dy = pitch * 3f64.sqrt() / 2.0;
            let mut row = 0;
            let mut y = mn[1];
            while y <= mx[1] + 1e-9 {
                let mut x = mn[0] + if row % 2 == 1 { pitch / 2.0 } else { 0.0 };
                while x <= mx[0] + 1e-9 {
                    let mut ins = false;
                    for (rg, _) in &rings {
                        if geom::point_in_poly([x, y], rg) {
                            ins = !ins;
                        }
                    }
                    if ins {
                        centres.push([x, y]);
                    }
                    x += pitch;
                }
                y += dy;
                row += 1;
            }
        }
    }
    let mut kept: Vec<P> = vec![];
    for c in centres {
        if kept.iter().all(|k| dist(*k, c) >= pitch * 0.85) {
            kept.push(c);
        }
    }
    kept
}

pub fn holes_at(centres: &[P], diameter: f64, as_points: bool) -> Vec<Vec<Path>> {
    centres.iter().map(|c| vec![if as_points { Path { pts: vec![*c], closed: false } } else { circle(*c, diameter / 2.0) }]).collect()
}

// ---------------------------------------------------------------- recesses, inlays, outlines

/// A knockout: the bottom shapes with a recess where the top shapes sit, grown by `gap`.
pub fn knockout(bottom: &[Vec<Path>], top: &[Vec<Path>], gap: f64) -> Vec<Path> {
    let base = merged(bottom);
    let cutter = merged(top);
    let cutter = if gap > 0.0 { geom::offset_region(&cutter, gap) } else { cutter };
    to_paths(&geom::boolean(&base, &cutter, BoolOp::Difference))
}

/// Inlay pocket and matching insert. For a router the pocket is the shape opened by half the
/// `clearance`, and the insert the shape closed by half of it, placed to the right (mirrored
/// for gluing face down when `mirror`).
pub fn inlay(parts: &[Vec<Path>], clearance: f64, mirror: bool) -> Option<(Vec<Path>, Vec<Path>)> {
    let region = merged(parts);
    let base = to_paths(&region);
    let (mn, mx) = bounds(&base)?;
    let half = clearance / 2.0;
    let pocket = to_paths(&geom::offset_region(&region, half));
    let mut insert = to_paths(&geom::offset_region(&region, -half));
    let cx = (mn[0] + mx[0]) / 2.0;
    let shift = mx[0] - mn[0] + 10.0;
    for p in &mut insert {
        for q in &mut p.pts {
            if mirror {
                q[0] = 2.0 * cx - q[0];
            }
            q[0] += shift;
        }
        if mirror {
            p.pts.reverse();
        }
    }
    (!pocket.is_empty() && !insert.is_empty()).then_some((pocket, insert))
}

/// A cut line around everything (stickers, badges, coasters): merged, grown by `distance`, holes
/// left out.
pub fn contour(parts: &[Vec<Path>], distance: f64) -> Vec<Path> {
    let m = merged(parts);
    let grown = if distance > 0.0 { geom::offset_region(&m, distance) } else { m };
    let mut outer = geo::MultiPolygon::new(vec![]);
    for poly in grown.0 {
        let single = geo::MultiPolygon::new(vec![geo::Polygon::new(poly.exterior().clone(), vec![])]);
        outer = if outer.0.is_empty() { single } else { outer.union(&single) };
    }
    to_paths(&outer)
}

/// Stencil bridges: a strip `width` mm wide across every hole (the middle of an O, A or B),
/// so the island stays attached.
pub fn stencil_bridges(paths: &[Path], width: f64, horizontal: bool) -> Option<Vec<Path>> {
    use geo::{BoundingRect, Centroid};
    let r = region(paths);
    let half = width.max(0.01) / 2.0;
    let mut strips = vec![];
    for poly in &r.0 {
        let Some(outer) = poly.exterior().bounding_rect() else { continue };
        for hole in poly.interiors() {
            let Some(c) = geo::Polygon::new(hole.clone(), vec![]).centroid() else { continue };
            let rect = if horizontal {
                geo::Rect::new((outer.min().x - 1.0, c.y() - half), (outer.max().x + 1.0, c.y() + half))
            } else {
                geo::Rect::new((c.x() - half, outer.min().y - 1.0), (c.x() + half, outer.max().y + 1.0))
            };
            strips.push(rect.to_polygon());
        }
    }
    if strips.is_empty() {
        return None;
    }
    Some(to_paths(&r.difference(&geo::MultiPolygon::new(strips))))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DepthKind {
    DropShadow,
    CastShadow,
    Extrude,
}

impl DepthKind {
    pub const ALL: [DepthKind; 3] = [DepthKind::DropShadow, DepthKind::CastShadow, DepthKind::Extrude];
    pub fn label(self) -> &'static str {
        match self {
            DepthKind::DropShadow => "Drop shadow",
            DepthKind::CastShadow => "Cast shadow",
            DepthKind::Extrude => "3D block (extrude)",
        }
    }
}

/// Shadow or 3D lines for the shapes (dx, dy in mm; for cast shadows dx is the skew per mm of
/// height and dy the squash).
pub fn depth(parts: &[Vec<Path>], kind: DepthKind, dx: f64, dy: f64) -> Vec<Path> {
    let shape = merged(parts);
    let outline = to_paths(&shape);
    match kind {
        DepthKind::DropShadow => outline.into_iter().map(|mut p| {
            p.pts.iter_mut().for_each(|q| *q = [q[0] + dx, q[1] + dy]);
            p
        }).collect(),
        DepthKind::CastShadow => {
            let Some((mn, _)) = bounds(&outline) else { return vec![] };
            let squash = dy.clamp(0.02, 2.0);
            let sheared: Vec<Path> = outline
                .into_iter()
                .map(|p| Path { pts: p.pts.iter().map(|q| {
                    let rise = q[1] - mn[1];
                    [q[0] + rise * dx, mn[1] + rise * squash]
                }).collect(), closed: p.closed })
                .collect();
            let tidy = to_paths(&region(&sheared));
            if tidy.is_empty() { sheared } else { tidy }
        }
        DepthKind::Extrude => {
            let mut out: Vec<Path> = outline.iter().map(|p| Path { pts: p.pts.iter().map(|q| [q[0] + dx, q[1] + dy]).collect(), closed: true }).collect();
            for p in &outline {
                let n = p.pts.len();
                let mut since = f64::INFINITY;
                for i in 0..n {
                    let (prev, here, next) = (p.pts[(i + n - 1) % n], p.pts[i], p.pts[(i + 1) % n]);
                    let (a, b) = ([here[0] - prev[0], here[1] - prev[1]], [next[0] - here[0], next[1] - here[1]]);
                    let (la, lb) = (a[0].hypot(a[1]).max(1e-12), b[0].hypot(b[1]).max(1e-12));
                    let turn = (a[0] * b[0] + a[1] * b[1]) / (la * lb);
                    since += la;
                    if turn < 0.95 || since >= 4.0 {
                        out.push(Path { pts: vec![here, [here[0] + dx, here[1] + dy]], closed: false });
                        since = 0.0;
                    }
                }
            }
            out
        }
    }
}

/// Copies turned round a centre `radius` mm below the shapes' middle (petals); `weld` merges them.
pub fn radial_repeat(paths: &[Path], copies: u32, radius: f64, weld: bool) -> Vec<Path> {
    let Some((mn, mx)) = bounds(paths) else { return vec![] };
    let centre = [(mn[0] + mx[0]) / 2.0, (mn[1] + mx[1]) / 2.0 - radius];
    let n = copies.clamp(1, 360);
    let mut parts = vec![];
    for k in 0..n {
        let mut s = temp_shape(paths);
        geom::rotate_about(&mut s, centre, 360.0 * k as f64 / n as f64);
        parts.push(s.paths);
    }
    if weld {
        let u = to_paths(&merged(&parts));
        if !u.is_empty() {
            return u;
        }
    }
    parts.into_iter().flatten().collect()
}

fn resample(p: &Path, n: usize) -> Vec<P> {
    let mut pts = p.pts.clone();
    if p.closed && !pts.is_empty() {
        pts.push(pts[0]);
    }
    let mut lens = vec![0.0];
    for w in pts.windows(2) {
        lens.push(lens.last().unwrap() + dist(w[0], w[1]));
    }
    let total = *lens.last().unwrap();
    let count = if p.closed { n } else { n + 1 };
    (0..count)
        .map(|k| {
            let s = total * k as f64 / n as f64;
            let i = lens.partition_point(|&x| x < s).clamp(1, pts.len() - 1);
            let span = (lens[i] - lens[i - 1]).max(1e-12);
            let t = ((s - lens[i - 1]) / span).clamp(0.0, 1.0);
            [pts[i - 1][0] + (pts[i][0] - pts[i - 1][0]) * t, pts[i - 1][1] + (pts[i][1] - pts[i - 1][1]) * t]
        })
        .collect()
}

fn align_loop(a: &[P], b: &mut Vec<P>) {
    if (geom::signed_area(a) > 0.0) != (geom::signed_area(b) > 0.0) {
        b.reverse();
    }
    let cen = |v: &[P]| {
        let n = v.len().max(1) as f64;
        [v.iter().map(|p| p[0]).sum::<f64>() / n, v.iter().map(|p| p[1]).sum::<f64>() / n]
    };
    let (ca, cb) = (cen(a), cen(b));
    let n = b.len().min(a.len());
    let cost = |k: usize| {
        (0..n).step_by((n / 32).max(1)).map(|i| {
            let (p, q) = (a[i], b[(i + k) % n]);
            ((p[0] - ca[0]) - (q[0] - cb[0])).powi(2) + ((p[1] - ca[1]) - (q[1] - cb[1])).powi(2)
        }).sum::<f64>()
    };
    if let Some(best) = (0..n).min_by(|&x, &y| cost(x).total_cmp(&cost(y))) {
        b.rotate_left(best);
    }
}

/// `steps` in-between shapes morphing from `start` to `end` (both ends excluded).
pub fn blend(start: &[Path], end: &[Path], steps: u32) -> Vec<Vec<Path>> {
    const N: usize = 256;
    if start.is_empty() || end.is_empty() {
        return vec![];
    }
    let mut pairs = vec![];
    for i in 0..start.len().max(end.len()) {
        let pa = &start[i.min(start.len() - 1)];
        let pb = &end[i.min(end.len() - 1)];
        let a = resample(pa, N);
        let mut b = resample(pb, N);
        let closed = pa.closed && pb.closed;
        if closed {
            align_loop(&a, &mut b);
        }
        pairs.push((a, b, closed));
    }
    let n = steps.clamp(1, 500);
    (1..=n)
        .map(|s| {
            let t = s as f64 / (n + 1) as f64;
            pairs
                .iter()
                .map(|(a, b, closed)| Path {
                    pts: a.iter().zip(b.iter()).map(|(p, q)| [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t]).collect(),
                    closed: *closed,
                })
                .collect()
        })
        .collect()
}

/// What of `paths` lies inside `mask`: closed outlines cut to it, open lines trimmed.
pub fn clip(paths: &[Path], mask: &geo::MultiPolygon<f64>) -> Vec<Path> {
    let closed: Vec<Path> = paths.iter().filter(|p| p.closed).cloned().collect();
    let mut out = if closed.is_empty() { vec![] } else { to_paths(&geom::boolean(&region(&closed), mask, BoolOp::Intersection)) };
    let open: Vec<Vec<P>> = paths.iter().filter(|p| !p.closed).map(|p| p.pts.clone()).collect();
    if !open.is_empty() {
        out.extend(clip_lines(open, mask));
    }
    out
}

/// Splits shapes along the straight line through `a` and `b`. Closed regions crossed right
/// through become two pieces; open lines are cut where it crosses them. `None` if it misses.
pub fn knife(paths: &[Path], a: P, b: P) -> Option<Vec<Vec<Path>>> {
    let d = [b[0] - a[0], b[1] - a[1]];
    let l = d[0].hypot(d[1]);
    if l < 1e-9 {
        return None;
    }
    let dir = [d[0] / l, d[1] / l];
    let n = [-dir[1], dir[0]];
    let far = 1e5;
    let closed = paths.iter().all(|p| p.closed);
    if closed {
        let shape = region(paths);
        let side = |s: f64| {
            let pts = vec![
                [a[0] - dir[0] * far, a[1] - dir[1] * far],
                [a[0] + dir[0] * far, a[1] + dir[1] * far],
                [a[0] + dir[0] * far + n[0] * far * s, a[1] + dir[1] * far + n[1] * far * s],
                [a[0] - dir[0] * far + n[0] * far * s, a[1] - dir[1] * far + n[1] * far * s],
            ];
            region(&[Path { pts, closed: true }])
        };
        let pieces: Vec<Vec<Path>> = [side(1.0), side(-1.0)].iter().map(|h| to_paths(&shape.intersection(h))).filter(|p| !p.is_empty()).collect();
        return (pieces.len() == 2).then_some(pieces);
    }
    // open lines: split at each crossing of the knife's line (between a and b)
    let mut out = vec![];
    let mut any = false;
    for p in paths {
        let mut cur: Vec<P> = vec![p.pts[0]];
        let segs = if p.closed { p.pts.len() } else { p.pts.len().saturating_sub(1) };
        for i in 0..segs {
            let (p0, p1) = (p.pts[i], p.pts[(i + 1) % p.pts.len()]);
            let r = [p1[0] - p0[0], p1[1] - p0[1]];
            let den = r[0] * d[1] - r[1] * d[0];
            if den.abs() > 1e-12 {
                let q = [a[0] - p0[0], a[1] - p0[1]];
                let t = (q[0] * d[1] - q[1] * d[0]) / den;
                let u = (q[0] * r[1] - q[1] * r[0]) / den;
                if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
                    let x = [p0[0] + r[0] * t, p0[1] + r[1] * t];
                    cur.push(x);
                    out.push(vec![Path { pts: std::mem::take(&mut cur), closed: false }]);
                    cur.push(x);
                    any = true;
                }
            }
            cur.push(p1);
        }
        if cur.len() >= 2 {
            out.push(vec![Path { pts: cur, closed: false }]);
        }
    }
    any.then_some(out)
}

/// Points and directions (radians) for `count` copies evenly along a guide outline.
pub fn along_path(guide: &Path, count: usize) -> Vec<(P, f64)> {
    let mut pts = guide.pts.clone();
    if guide.closed && !pts.is_empty() {
        pts.push(pts[0]);
    }
    if pts.len() < 2 || count == 0 {
        return vec![];
    }
    let mut lens = vec![0.0];
    for w in pts.windows(2) {
        lens.push(lens.last().unwrap() + dist(w[0], w[1]));
    }
    let total = *lens.last().unwrap();
    if total <= 0.0 {
        return vec![];
    }
    (0..count)
        .map(|k| {
            let s = match (guide.closed, count) {
                (true, _) => total * k as f64 / count as f64,
                (false, 1) => total / 2.0,
                (false, _) => total * k as f64 / (count - 1) as f64,
            };
            let i = lens.partition_point(|&l| l <= s).clamp(1, pts.len() - 1);
            let seg = (lens[i] - lens[i - 1]).max(1e-12);
            let t = ((s - lens[i - 1]) / seg).clamp(0.0, 1.0);
            let (a, b) = (pts[i - 1], pts[i]);
            ([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t], (b[1] - a[1]).atan2(b[0] - a[0]))
        })
        .collect()
}

/// Convex hull (Andrew's monotone chain), anticlockwise.
pub fn convex_hull(points: &[P]) -> Vec<P> {
    let mut p: Vec<P> = points.to_vec();
    p.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    p.dedup();
    if p.len() < 3 {
        return p;
    }
    let cross = |o: P, a: P, b: P| (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
    let mut hull: Vec<P> = Vec::with_capacity(p.len() * 2);
    for pass in 0..2 {
        let start = hull.len();
        let seq: Vec<P> = if pass == 0 { p.clone() } else { p.iter().rev().copied().collect() };
        for q in seq {
            while hull.len() >= start + 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], q) <= 0.0 {
                hull.pop();
            }
            hull.push(q);
        }
        hull.pop();
    }
    hull
}

/// The smallest-area rectangle around the points (corners), or `None`.
pub fn min_area_rect(points: &[P]) -> Option<Vec<P>> {
    let hull = convex_hull(points);
    if hull.len() < 2 {
        return None;
    }
    let mut best: Option<(f64, Vec<P>)> = None;
    for i in 0..hull.len() {
        let e = [hull[(i + 1) % hull.len()][0] - hull[i][0], hull[(i + 1) % hull.len()][1] - hull[i][1]];
        let el = e[0].hypot(e[1]);
        if el < 1e-12 {
            continue;
        }
        let u = [e[0] / el, e[1] / el];
        let v = [-u[1], u[0]];
        let (mut a0, mut a1, mut b0, mut b1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for p in &hull {
            let (a, b) = (p[0] * u[0] + p[1] * u[1], p[0] * v[0] + p[1] * v[1]);
            a0 = a0.min(a);
            a1 = a1.max(a);
            b0 = b0.min(b);
            b1 = b1.max(b);
        }
        let area = (a1 - a0) * (b1 - b0);
        if best.as_ref().map(|(ba, _)| area < *ba - 1e-9).unwrap_or(true) {
            let pt = |a: f64, b: f64| [u[0] * a + v[0] * b, u[1] * a + v[1] * b];
            best = Some((area, vec![pt(a0, b0), pt(a1, b0), pt(a1, b1), pt(a0, b1)]));
        }
    }
    best.map(|b| b.1)
}
