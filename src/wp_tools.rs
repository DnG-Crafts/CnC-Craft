//! Workplane tools: Star, Trim, Knife, Shape Builder, Round-corner tool,
//! node add/delete and snapping to other shapes. Geometry works on CncCraft polylines (mm, Y up).

use crate::app::App;
use crate::geom::{self, dist};
use crate::model::*;
use geo::{Area, BooleanOps, Contains};

// ---------------------------------------------------------------- star

/// A star with `points` tips, outer radius `r` and inner radius `r * inner`.
pub fn star_path(c: P, r: f64, points: u32, inner: f64, rot: f64) -> Path {
    let n = points.max(3) * 2;
    let pts = (0..n)
        .map(|i| {
            let a = rot + std::f64::consts::FRAC_PI_2 + std::f64::consts::TAU * i as f64 / n as f64;
            let rr = if i % 2 == 1 { r * inner.clamp(0.05, 0.95) } else { r };
            [c[0] + rr * a.cos(), c[1] + rr * a.sin()]
        })
        .collect();
    Path { pts, closed: true }
}

// ---------------------------------------------------------------- trim

fn crossing(a0: P, a1: P, b0: P, b1: P) -> Option<f64> {
    let (r, s) = ([a1[0] - a0[0], a1[1] - a0[1]], [b1[0] - b0[0], b1[1] - b0[1]]);
    let cross = |u: P, v: P| u[0] * v[1] - u[1] * v[0];
    let den = cross(r, s);
    if den.abs() < 1e-12 {
        return None;
    }
    let q = [b0[0] - a0[0], b0[1] - a0[1]];
    let t = cross(q, s) / den;
    let u = cross(q, r) / den;
    ((-1e-9..=1.0 + 1e-9).contains(&t) && (-1e-9..=1.0 + 1e-9).contains(&u)).then_some(t.clamp(0.0, 1.0))
}

struct Poly {
    pts: Vec<P>,
    len: Vec<f64>,
    closed: bool,
}

impl Poly {
    fn new(p: &Path) -> Poly {
        let mut pts = p.pts.clone();
        if p.closed && pts.len() > 1 && pts.first() != pts.last() {
            pts.push(pts[0]);
        }
        let mut len = vec![0.0];
        for w in pts.windows(2) {
            len.push(len.last().unwrap() + dist(w[0], w[1]));
        }
        Poly { pts, len, closed: p.closed }
    }
    fn total(&self) -> f64 {
        *self.len.last().unwrap_or(&0.0)
    }
    fn at(&self, s: f64) -> P {
        let i = self.len.partition_point(|&l| l <= s).clamp(1, self.pts.len() - 1);
        let seg = (self.len[i] - self.len[i - 1]).max(1e-12);
        let t = ((s - self.len[i - 1]) / seg).clamp(0.0, 1.0);
        let (a, b) = (self.pts[i - 1], self.pts[i]);
        [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
    }
    fn piece(&self, a: f64, b: f64) -> Vec<P> {
        let mut out = vec![self.at(a)];
        for (p, &l) in self.pts.iter().zip(&self.len) {
            if l > a + 1e-9 && l < b - 1e-9 {
                out.push(*p);
            }
        }
        out.push(self.at(b));
        out
    }
    fn project(&self, p: P) -> (f64, f64) {
        let mut best = (0.0, f64::MAX);
        for (i, w) in self.pts.windows(2).enumerate() {
            let d = [w[1][0] - w[0][0], w[1][1] - w[0][1]];
            let l2 = d[0] * d[0] + d[1] * d[1];
            let t = if l2 > 0.0 { (((p[0] - w[0][0]) * d[0] + (p[1] - w[0][1]) * d[1]) / l2).clamp(0.0, 1.0) } else { 0.0 };
            let q = [w[0][0] + d[0] * t, w[0][1] + d[1] * t];
            let dd = dist(p, q);
            if dd < best.1 {
                best = (self.len[i] + t * l2.sqrt(), dd);
            }
        }
        best
    }
    fn crossings_with(&self, other: &Poly, same: bool) -> Vec<f64> {
        let mut out = vec![];
        let n = self.pts.len().saturating_sub(1);
        for (i, a) in self.pts.windows(2).enumerate() {
            for (j, b) in other.pts.windows(2).enumerate() {
                if same {
                    let adjacent = i == j || i + 1 == j || j + 1 == i || (self.closed && ((i == 0 && j + 1 == n) || (j == 0 && i + 1 == n)));
                    if adjacent {
                        continue;
                    }
                }
                if let Some(t) = crossing(a[0], a[1], b[0], b[1]) {
                    out.push(self.len[i] + t * (self.len[i + 1] - self.len[i]));
                }
            }
        }
        out
    }
}

/// Removes the piece of `target`'s outline under `at`, between its nearest crossings with the
/// cutters (and itself). `None` when nothing is under `at` or a closed outline has nothing to trim to.
pub fn trim(target: &[Path], cutters: &[Vec<Path>], at: P, tol: f64) -> Option<Vec<Path>> {
    let (hit, poly, s_click) = target
        .iter()
        .enumerate()
        .filter(|(_, p)| p.pts.len() >= 2)
        .map(|(i, p)| {
            let pl = Poly::new(p);
            let (s, d) = pl.project(at);
            (i, pl, s, d)
        })
        .filter(|x| x.3 <= tol)
        .min_by(|a, b| a.3.total_cmp(&b.3))
        .map(|(i, p, s, _)| (i, p, s))?;
    let mut cuts: Vec<f64> = vec![];
    for (i, sp) in target.iter().enumerate() {
        if sp.pts.len() >= 2 {
            cuts.extend(poly.crossings_with(&Poly::new(sp), i == hit));
        }
    }
    for c in cutters.iter().flatten() {
        if c.pts.len() >= 2 {
            cuts.extend(poly.crossings_with(&Poly::new(c), false));
        }
    }
    let total = poly.total();
    let eps = 1e-6;
    cuts.retain(|s| (*s > eps && *s < total - eps) || poly.closed);
    cuts.sort_by(|a, b| a.total_cmp(b));
    cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    let below = cuts.iter().copied().rev().find(|&s| s < s_click - eps);
    let above = cuts.iter().copied().find(|&s| s > s_click + eps);
    let mut kept: Vec<Vec<P>> = vec![];
    if poly.closed {
        let (lo, hi) = match (below, above) {
            (Some(lo), Some(hi)) => (lo, hi),
            (None, Some(hi)) if cuts.len() >= 2 => (*cuts.last().unwrap() - total, hi),
            (Some(lo), None) if cuts.len() >= 2 => (lo, cuts[0] + total),
            _ => return None,
        };
        let mut pts = if hi <= total { poly.piece(hi, total) } else { vec![] };
        let wrap_end = if lo < 0.0 { lo + total } else { lo };
        let start = if hi > total { hi - total } else { 0.0 };
        let mut tail = poly.piece(start, wrap_end);
        if !pts.is_empty() {
            tail.remove(0);
        }
        pts.extend(tail);
        kept.push(pts);
    } else {
        let lo = below.unwrap_or(0.0);
        let hi = above.unwrap_or(total);
        if lo > eps {
            kept.push(poly.piece(0.0, lo));
        }
        if hi < total - eps {
            kept.push(poly.piece(hi, total));
        }
    }
    let mut out = vec![];
    for (i, sp) in target.iter().enumerate() {
        if i == hit {
            for k in &kept {
                if k.len() >= 2 {
                    out.push(Path { pts: k.clone(), closed: false });
                }
            }
        } else {
            out.push(sp.clone());
        }
    }
    Some(out)
}

// ---------------------------------------------------------------- corners

/// Indices of a path's sharp corners (turning between 20° and 175°).
pub fn corner_indices(p: &Path) -> Vec<usize> {
    let n = p.pts.len();
    if n < 3 {
        return vec![];
    }
    (0..n)
        .filter(|&i| {
            if !p.closed && (i == 0 || i == n - 1) {
                return false;
            }
            let (a, v, b) = (p.pts[(i + n - 1) % n], p.pts[i], p.pts[(i + 1) % n]);
            let (la, lb) = (dist(a, v), dist(v, b));
            if la < 1e-9 || lb < 1e-9 {
                return false;
            }
            let c = (((v[0] - a[0]) * (b[0] - v[0]) + (v[1] - a[1]) * (b[1] - v[1])) / (la * lb)).clamp(-1.0, 1.0);
            let turn = c.acos();
            turn > 20f64.to_radians() && turn < 175f64.to_radians()
        })
        .collect()
}

/// Rounds (or chamfers) the single corner `i` of a path.
pub fn round_one(p: &Path, i: usize, r: f64, chamfer: bool) -> Path {
    let n = p.pts.len();
    if n < 3 || r <= 0.0 || i >= n {
        return p.clone();
    }
    let (a, v, b) = (p.pts[(i + n - 1) % n], p.pts[i], p.pts[(i + 1) % n]);
    let (la, lb) = (dist(a, v), dist(v, b));
    let d1 = [(v[0] - a[0]) / la, (v[1] - a[1]) / la];
    let d2 = [(b[0] - v[0]) / lb, (b[1] - v[1]) / lb];
    let turn = (d1[0] * d2[0] + d1[1] * d2[1]).clamp(-1.0, 1.0).acos();
    let theta = std::f64::consts::PI - turn;
    let mut t = r / (theta / 2.0).tan();
    let mut rr = r;
    let lim = la.min(lb) * 0.5;
    if t > lim {
        t = lim;
        rr = t * (theta / 2.0).tan();
    }
    let p1 = [v[0] - d1[0] * t, v[1] - d1[1] * t];
    let p2 = [v[0] + d2[0] * t, v[1] + d2[1] * t];
    let mut mid = vec![p1];
    if !chamfer {
        let bis = [-d1[0] + d2[0], -d1[1] + d2[1]];
        let bl = bis[0].hypot(bis[1]).max(1e-12);
        let cd = rr / (theta / 2.0).sin();
        let c = [v[0] + bis[0] / bl * cd, v[1] + bis[1] / bl * cd];
        let a0 = (p1[1] - c[1]).atan2(p1[0] - c[0]);
        let mut a1 = (p2[1] - c[1]).atan2(p2[0] - c[0]);
        let cross = d1[0] * d2[1] - d1[1] * d2[0];
        if cross > 0.0 {
            while a1 < a0 {
                a1 += std::f64::consts::TAU;
            }
        } else {
            while a1 > a0 {
                a1 -= std::f64::consts::TAU;
            }
        }
        let steps = (((a1 - a0).abs() / 10f64.to_radians()).ceil() as usize).max(2);
        for k in 1..steps {
            let ang = a0 + (a1 - a0) * k as f64 / steps as f64;
            mid.push([c[0] + rr * ang.cos(), c[1] + rr * ang.sin()]);
        }
    }
    mid.push(p2);
    let mut pts = p.pts[..i].to_vec();
    pts.extend(mid);
    pts.extend_from_slice(&p.pts[i + 1..]);
    Path { pts, closed: p.closed }
}

// ---------------------------------------------------------------- shape builder

/// Every separate area the outlines of `regions` enclose, overlaps included.
pub fn faces(regions: &[geo::MultiPolygon<f64>]) -> Vec<geo::Polygon<f64>> {
    let mut faces: Vec<geo::MultiPolygon<f64>> = vec![];
    let mut covered = geo::MultiPolygon::<f64>::new(vec![]);
    for r in regions {
        if r.0.is_empty() {
            continue;
        }
        let mut next = vec![];
        for f in &faces {
            let i = f.intersection(r);
            let d = f.difference(r);
            if !i.0.is_empty() {
                next.push(i);
            }
            if !d.0.is_empty() {
                next.push(d);
            }
        }
        let rest = if covered.0.is_empty() { r.clone() } else { r.difference(&covered) };
        if !rest.0.is_empty() {
            next.push(rest);
        }
        covered = if covered.0.is_empty() { r.clone() } else { covered.union(r) };
        faces = next;
    }
    faces.into_iter().flat_map(|m| m.0).filter(|p| p.unsigned_area() > 1e-3).collect()
}

pub fn face_at(faces: &[geo::Polygon<f64>], p: P) -> Option<usize> {
    let pt = geo::Point::new(p[0], p[1]);
    // the smallest face containing the point (faces don't overlap, but be safe)
    faces.iter().enumerate().filter(|(_, f)| f.contains(&pt)).min_by(|a, b| a.1.unsigned_area().total_cmp(&b.1.unsigned_area())).map(|(i, _)| i)
}

impl App {
    /// Shapes the Shape Builder works on: the selection when two or more are selected, else all.
    pub fn builder_shapes(&self) -> Vec<u64> {
        let closed = |s: &&Shape| s.paths.iter().any(|p| p.closed && p.pts.len() > 2) && !self.doc.locked.contains(&s.id);
        let sel: Vec<u64> = self.doc.shapes.iter().filter(|s| self.selection.contains(&s.id)).filter(closed).map(|s| s.id).collect();
        if sel.len() >= 2 {
            sel
        } else {
            self.doc.shapes.iter().filter(closed).map(|s| s.id).collect()
        }
    }

    /// Recomputes the Shape Builder's areas when the shapes change.
    pub fn refresh_builder(&mut self) {
        let ids = self.builder_shapes();
        let mut key: f64 = ids.iter().map(|i| *i as f64).sum();
        for s in self.doc.shapes.iter().filter(|s| ids.contains(&s.id)) {
            for p in &s.paths {
                for (k, q) in p.pts.iter().enumerate() {
                    key += q[0] * 1.37 + q[1] * 0.71 + k as f64 * 1e-3;
                }
            }
        }
        let key_bits = key.to_bits() ^ (ids.len() as u64);
        if self.design.builder.as_ref().map(|b| b.0) == Some(key_bits) {
            return;
        }
        let regions: Vec<geo::MultiPolygon<f64>> = self.doc.shapes.iter().filter(|s| ids.contains(&s.id)).map(|s| geom::region_of(std::iter::once(s))).collect();
        let f = faces(&regions);
        self.design.builder = Some((key_bits, ids, f));
    }

    /// Merges (or deletes) the picked areas.
    pub fn builder_apply(&mut self, picked: &[usize], delete: bool) {
        let Some((_, ids, faces)) = self.design.builder.clone() else { return };
        if picked.is_empty() {
            return;
        }
        let mut uni = geo::MultiPolygon::<f64>::new(vec![]);
        for &i in picked {
            if let Some(f) = faces.get(i) {
                let m = geo::MultiPolygon::new(vec![f.clone()]);
                uni = if uni.0.is_empty() { m } else { uni.union(&m) };
            }
        }
        if uni.0.is_empty() {
            return;
        }
        self.checkpoint();
        let mut removed = vec![];
        let mut heir_from = None;
        for id in &ids {
            let Some(s) = self.doc.shape(*id) else { continue };
            let reg = geom::region_of(std::iter::once(s));
            let rest = reg.difference(&uni);
            let touched = reg.intersection(&uni).unsigned_area() > 1e-6;
            if !touched {
                continue;
            }
            if heir_from.is_none() {
                heir_from = Some(*id);
            }
            let paths = geom::region_to_paths(&rest);
            if paths.is_empty() {
                removed.push(*id);
            } else if let Some(s) = self.doc.shapes.iter_mut().find(|s| s.id == *id) {
                s.paths = paths;
                s.kind = ShapeKind::Path;
            }
        }
        let mut new_id = None;
        if !delete {
            let ids_new = self.add_parts(vec![geom::region_to_paths(&uni)], "Shape Builder merge");
            new_id = ids_new.first().copied();
            // the merged shape cuts like the first shape it came from
            if let (Some(n), Some(h)) = (new_id, heir_from) {
                for tp in &mut self.doc.toolpaths {
                    if tp.shape_ids.contains(&h) && !tp.shape_ids.contains(&n) {
                        tp.shape_ids.push(n);
                    }
                }
            }
        }
        self.remove_into(&removed, new_id);
        self.selection = new_id.into_iter().collect();
        self.design.builder = None;
        self.status = if delete { "Areas deleted".into() } else { "Areas merged".into() };
        self.job_dirty = true;
    }

    // ------------------------------------------------------------ knife, trim, corner

    pub fn knife_cut(&mut self, a: P, b: P) {
        if dist(a, b) < 0.5 {
            return;
        }
        let targets: Vec<(u64, Vec<Path>)> = self
            .doc
            .shapes
            .iter()
            .filter(|s| !self.doc.locked.contains(&s.id) && (self.selection.is_empty() || self.selection.contains(&s.id)))
            .map(|s| (s.id, s.paths.clone()))
            .collect();
        self.checkpoint();
        let mut cut = 0;
        let mut new_sel = vec![];
        for (id, paths) in targets {
            if let Some(pieces) = crate::modify::knife(&paths, a, b) {
                let ids = self.add_parts(pieces, "Knife");
                for tp in &mut self.doc.toolpaths {
                    if tp.shape_ids.contains(&id) {
                        for n in &ids {
                            if !tp.shape_ids.contains(n) {
                                tp.shape_ids.push(*n);
                            }
                        }
                    }
                }
                self.remove_into(&[id], None);
                new_sel.extend(ids);
                cut += 1;
            }
        }
        if cut == 0 {
            self.undo.pop();
            self.status = "Knife: drag the line right across a shape to split it".into();
        } else {
            self.selection = new_sel;
            self.status = format!("Knife split {cut} shape(s)");
            self.job_dirty = true;
        }
    }

    pub fn trim_at(&mut self, p: P, tol: f64) {
        let Some(id) = self.hits_at(p, tol).into_iter().find(|id| self.doc.shape(*id).map(|s| crate::geom::outline_dist(s, p) <= tol).unwrap_or(false)) else {
            self.status = "Trim: click right on the piece of outline to remove".into();
            return;
        };
        let target = self.doc.shape(id).map(|s| s.paths.clone()).unwrap_or_default();
        let cutters: Vec<Vec<Path>> = self.doc.shapes.iter().filter(|s| s.id != id).map(|s| s.paths.clone()).collect();
        match trim(&target, &cutters, p, tol) {
            Some(new) => {
                self.checkpoint();
                if new.is_empty() {
                    self.remove_into(&[id], None);
                } else if let Some(s) = self.doc.shapes.iter_mut().find(|s| s.id == id) {
                    s.paths = new;
                    s.kind = ShapeKind::Path;
                }
                self.status = "Trimmed".into();
                self.job_dirty = true;
            }
            None => self.status = "Trim: nothing crosses that outline to trim back to".into(),
        }
    }

    /// The corner marker nearest `p` within `tol`: (shape, path, vertex index, position).
    pub fn corner_near(&self, p: P, tol: f64) -> Option<(u64, usize, usize, P)> {
        let mut best: Option<(f64, (u64, usize, usize, P))> = None;
        for s in self.doc.shapes.iter().filter(|s| !self.doc.locked.contains(&s.id) && (self.selection.is_empty() || self.selection.contains(&s.id))) {
            for (pi, path) in s.paths.iter().enumerate() {
                for i in corner_indices(path) {
                    let d = dist(path.pts[i], p);
                    if d <= tol && best.map(|b| d < b.0).unwrap_or(true) {
                        best = Some((d, (s.id, pi, i, path.pts[i])));
                    }
                }
            }
        }
        best.map(|b| b.1)
    }

    pub fn round_corner_at(&mut self, p: P, tol: f64) {
        let Some((id, pi, i, _)) = self.corner_near(p, tol) else {
            self.status = "Round corner: click one of the corner markers".into();
            return;
        };
        let (r, ch) = (self.design.corner_r, self.design.chamfer);
        if r <= 0.0 {
            self.status = "Set a radius above zero first".into();
            return;
        }
        self.checkpoint();
        if let Some(s) = self.doc.shapes.iter_mut().find(|s| s.id == id) {
            s.paths[pi] = round_one(&s.paths[pi], i, r, ch);
            s.kind = ShapeKind::Path;
        }
        self.status = if ch { "Corner chamfered".into() } else { "Corner rounded".into() };
        self.job_dirty = true;
    }

    // ------------------------------------------------------------ nodes

    /// Double-click on an edge of a selected shape adds a point there.
    pub fn insert_node(&mut self, p: P, tol: f64) -> bool {
        let mut best: Option<(f64, u64, usize, usize, P)> = None;
        for s in self.selected_shapes() {
            for (pi, path) in s.paths.iter().enumerate() {
                let n = path.pts.len();
                let segs = if path.closed { n } else { n.saturating_sub(1) };
                for i in 0..segs {
                    let (a, b) = (path.pts[i], path.pts[(i + 1) % n]);
                    let d = geom::seg_dist(p, a, b);
                    if d <= tol && best.map(|x| d < x.0).unwrap_or(true) {
                        let ab = [b[0] - a[0], b[1] - a[1]];
                        let l2 = (ab[0] * ab[0] + ab[1] * ab[1]).max(1e-12);
                        let t = (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / l2).clamp(0.0, 1.0);
                        best = Some((d, s.id, pi, i + 1, [a[0] + ab[0] * t, a[1] + ab[1] * t]));
                    }
                }
            }
        }
        let Some((_, id, pi, at, q)) = best else { return false };
        self.checkpoint();
        if let Some(s) = self.doc.shapes.iter_mut().find(|s| s.id == id) {
            s.paths[pi].pts.insert(at, q);
            s.kind = ShapeKind::Path;
        }
        self.design.node_sel = Some((id, pi, at));
        self.status = "Point added".into();
        true
    }

    pub fn delete_node(&mut self) {
        let Some((id, pi, i)) = self.design.node_sel.take() else { return };
        let Some(s) = self.doc.shape(id) else { return };
        let Some(path) = s.paths.get(pi) else { return };
        let min = if path.closed { 3 } else { 2 };
        if path.pts.len() <= min {
            self.status = "That outline needs its remaining points".into();
            return;
        }
        self.checkpoint();
        if let Some(s) = self.doc.shapes.iter_mut().find(|s| s.id == id) {
            if i < s.paths[pi].pts.len() {
                s.paths[pi].pts.remove(i);
            }
            s.kind = ShapeKind::Path;
        }
        self.status = "Point deleted".into();
        self.job_dirty = true;
    }

    /// Nearest corner, point or centre of another shape within `tol`, for Snap to Objects.
    pub fn object_snap(&self, p: P, tol: f64, exclude: Option<u64>) -> Option<P> {
        let mut best: Option<(f64, P)> = None;
        let mut consider = |q: P| {
            let d = dist(q, p);
            if d <= tol && best.map(|b| d < b.0).unwrap_or(true) {
                best = Some((d, q));
            }
        };
        for s in &self.doc.shapes {
            if Some(s.id) == exclude {
                continue;
            }
            let Some((a, b)) = s.bbox() else { continue };
            if p[0] < a[0] - tol || p[0] > b[0] + tol || p[1] < a[1] - tol || p[1] > b[1] + tol {
                continue;
            }
            consider([(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]);
            for path in &s.paths {
                if path.pts.len() > 400 {
                    for i in corner_indices(path) {
                        consider(path.pts[i]);
                    }
                } else {
                    for q in &path.pts {
                        consider(*q);
                    }
                }
            }
        }
        best.map(|b| b.1)
    }
}

impl App {
    /// Drawing for the workplane tools: knife line, Shape Builder areas, corner markers,
    /// the selected point and the snap marker.
    pub fn paint_tool_overlays(&self, painter: &egui::Painter, rect: egui::Rect, pointer: Option<P>) {
        use crate::app::{CanvasTool, Drag};
        use egui::{Color32, Pos2, Stroke};
        let ts = |p: P| self.to_screen(rect, p);
        let poly_pts = |r: &geo::LineString<f64>| -> Vec<Pos2> { r.0.iter().map(|c| ts([c.x, c.y])).collect() };
        match &self.drag {
            Drag::Knife(s) => {
                if let Some(p) = pointer {
                    painter.extend(egui::Shape::dashed_line(&[ts(*s), ts(p)], Stroke::new(1.6f32, Color32::from_rgb(255, 90, 90)), 6.0, 3.0));
                    painter.circle_filled(ts(*s), 3.0, Color32::from_rgb(255, 90, 90));
                }
            }
            _ => {}
        }
        if self.tool == CanvasTool::Builder {
            if let Some((_, _, faces)) = &self.design.builder {
                let hover = pointer.and_then(|p| face_at(faces, p));
                let (picked, delete): (Vec<usize>, bool) = match &self.drag {
                    Drag::Build { picked, delete } => (picked.clone(), *delete),
                    _ => (hover.into_iter().collect(), false),
                };
                let alt_hover = matches!(self.drag, Drag::None) && pointer.is_some() && painter.ctx().input(|i| i.modifiers.alt);
                for (i, f) in faces.iter().enumerate() {
                    let on = picked.contains(&i);
                    let col = if on {
                        if delete || alt_hover {
                            Color32::from_rgb(240, 80, 80)
                        } else {
                            Color32::from_rgb(80, 210, 120)
                        }
                    } else {
                        Color32::from_rgb(150, 175, 230)
                    };
                    let w = if on { 2.2f32 } else { 1.0f32 };
                    painter.add(egui::Shape::closed_line(poly_pts(f.exterior()), Stroke::new(w, col)));
                    for h in f.interiors() {
                        painter.add(egui::Shape::closed_line(poly_pts(h), Stroke::new(w, col)));
                    }
                }
            }
        }
        if self.tool == CanvasTool::Corner {
            let hot = pointer.and_then(|p| self.corner_near(p, 9.0 / self.view.zoom)).map(|c| c.3);
            for s in self.doc.shapes.iter().filter(|s| !self.doc.locked.contains(&s.id) && (self.selection.is_empty() || self.selection.contains(&s.id))) {
                for path in &s.paths {
                    for i in corner_indices(path) {
                        let q = path.pts[i];
                        let is_hot = hot.map(|h| dist(h, q) < 1e-9).unwrap_or(false);
                        painter.circle(ts(q), if is_hot { 5.5 } else { 3.5 }, if is_hot { Color32::from_rgb(255, 200, 80) } else { Color32::TRANSPARENT }, Stroke::new(1.2f32, Color32::from_rgb(255, 200, 80)));
                    }
                }
            }
        }
        if self.tool == CanvasTool::Nodes {
            if let Some((id, pi, i)) = self.design.node_sel {
                if let Some(q) = self.doc.shape(id).and_then(|s| s.paths.get(pi)).and_then(|p| p.pts.get(i)) {
                    let c = ts(*q);
                    painter.rect_filled(egui::Rect::from_center_size(c, egui::vec2(8.0, 8.0)), 0.0, Color32::from_rgb(255, 160, 60));
                }
            }
        }
        // snap marker for drawing tools
        let drawing = matches!(self.tool, CanvasTool::Line | CanvasTool::Rect | CanvasTool::Ellipse | CanvasTool::Polygon | CanvasTool::Star | CanvasTool::Measure | CanvasTool::Point | CanvasTool::Nodes);
        if drawing && self.design.snap_objects {
            if let Some(p) = pointer {
                let exclude = match self.drag {
                    Drag::Node { shape, .. } => Some(shape),
                    _ => None,
                };
                if let Some(q) = self.object_snap(p, 8.0 / self.view.zoom, exclude) {
                    let c = ts(q);
                    painter.rect_stroke(egui::Rect::from_center_size(c, egui::vec2(10.0, 10.0)), 0.0, Stroke::new(1.5f32, Color32::from_rgb(255, 80, 220)), egui::StrokeKind::Middle);
                }
            }
        }
    }
}
