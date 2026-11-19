//! Import and export of drawing files (SVG, DXF).

use crate::geom;
use crate::model::{Path, Shape, P};

/// Reads an SVG file into paths (millimetres, Y up).
pub fn import_svg(data: &[u8]) -> Result<Vec<Vec<Path>>, String> {
    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_data(data, &opt).map_err(|e| format!("Could not read SVG: {e}"))?;
    let k = 25.4 / 96.0;
    let h = tree.size().height() as f64;
    let mut out = vec![];
    walk_group(tree.root(), &mut out, k, h);
    Ok(out)
}

fn walk_group(g: &usvg::Group, out: &mut Vec<Vec<Path>>, k: f64, h: f64) {
    for node in g.children() {
        match node {
            usvg::Node::Group(gr) => walk_group(gr, out, k, h),
            usvg::Node::Path(p) => {
                let t = p.abs_transform();
                let map = |pt: usvg::tiny_skia_path::Point| -> P {
                    let x = t.sx as f64 * pt.x as f64 + t.kx as f64 * pt.y as f64 + t.tx as f64;
                    let y = t.ky as f64 * pt.x as f64 + t.sy as f64 * pt.y as f64 + t.ty as f64;
                    [x * k, (h - y) * k]
                };
                let mut paths = vec![];
                let mut cur: Vec<P> = vec![];
                let finish = |cur: &mut Vec<P>, closed: bool, paths: &mut Vec<Path>| {
                    if cur.len() >= 2 {
                        let mut pts = std::mem::take(cur);
                        if closed && pts.len() > 2 && geom::dist(pts[0], *pts.last().unwrap()) < 1e-6 {
                            pts.pop();
                        }
                        paths.push(Path { pts, closed });
                    }
                    cur.clear();
                };
                for seg in p.data().segments() {
                    use usvg::tiny_skia_path::PathSegment as S;
                    match seg {
                        S::MoveTo(a) => {
                            finish(&mut cur, false, &mut paths);
                            cur.push(map(a));
                        }
                        S::LineTo(a) => cur.push(map(a)),
                        S::QuadTo(a, b) => {
                            let p0 = *cur.last().unwrap_or(&[0.0, 0.0]);
                            geom::quad(&mut cur, p0, map(a), map(b));
                        }
                        S::CubicTo(a, b, c) => {
                            let p0 = *cur.last().unwrap_or(&[0.0, 0.0]);
                            geom::cubic(&mut cur, p0, map(a), map(b), map(c));
                        }
                        S::Close => finish(&mut cur, true, &mut paths),
                    }
                }
                finish(&mut cur, false, &mut paths);
                if !paths.is_empty() {
                    out.push(paths);
                }
            }
            _ => {}
        }
    }
}

/// Reads LINE, CIRCLE, ARC, LWPOLYLINE and POLYLINE entities from an ASCII DXF file.
pub fn import_dxf(text: &str) -> Result<Vec<Vec<Path>>, String> {
    let lines: Vec<&str> = text.lines().map(|l| l.trim()).collect();
    let mut pairs = vec![];
    let mut i = 0;
    while i + 1 < lines.len() {
        if let Ok(code) = lines[i].parse::<i32>() {
            pairs.push((code, lines[i + 1].to_string()));
        }
        i += 2;
    }
    if pairs.is_empty() {
        return Err("This DXF file is empty or not in text format.".into());
    }
    // Split into entities
    let mut in_entities = false;
    let mut ents: Vec<(String, Vec<(i32, String)>)> = vec![];
    let mut prev_was_section = false;
    for (c, v) in pairs {
        if c == 2 && prev_was_section {
            in_entities = v == "ENTITIES" || v == "BLOCKS";
        }
        prev_was_section = c == 0 && v == "SECTION";
        if c == 0 {
            if v == "ENDSEC" {
                in_entities = false;
            }
            if in_entities {
                ents.push((v, vec![]));
            }
        } else if in_entities {
            if let Some(e) = ents.last_mut() {
                e.1.push((c, v));
            }
        }
    }
    let f = |d: &Vec<(i32, String)>, code: i32| -> f64 { d.iter().find(|(c, _)| *c == code).and_then(|(_, v)| v.parse().ok()).unwrap_or(0.0) };
    let mut out = vec![];
    let mut poly: Option<(Vec<P>, Vec<f64>, bool)> = None;
    for (kind, d) in &ents {
        match kind.as_str() {
            "LINE" => out.push(vec![Path { pts: vec![[f(d, 10), f(d, 20)], [f(d, 11), f(d, 21)]], closed: false }]),
            "CIRCLE" => {
                let r = f(d, 40);
                out.push(vec![geom::ellipse_path([f(d, 10), f(d, 20)], r, r)]);
            }
            "ARC" => {
                let (c, r) = ([f(d, 10), f(d, 20)], f(d, 40));
                let (a0, mut a1) = (f(d, 50), f(d, 51));
                if a1 < a0 {
                    a1 += 360.0;
                }
                let n = (((a1 - a0) / 5.0).ceil() as usize).max(2);
                let pts = (0..=n)
                    .map(|i| {
                        let a = (a0 + (a1 - a0) * i as f64 / n as f64).to_radians();
                        [c[0] + r * a.cos(), c[1] + r * a.sin()]
                    })
                    .collect();
                out.push(vec![Path { pts, closed: false }]);
            }
            "LWPOLYLINE" => {
                let closed = (f(d, 70) as i32) & 1 == 1;
                let mut pts = vec![];
                let mut bulges = vec![];
                for (c, v) in d {
                    let val: f64 = v.parse().unwrap_or(0.0);
                    match c {
                        10 => {
                            pts.push([val, 0.0]);
                            bulges.push(0.0);
                        }
                        20 => {
                            if let Some(p) = pts.last_mut() {
                                p[1] = val
                            }
                        }
                        42 => {
                            if let Some(b) = bulges.last_mut() {
                                *b = val
                            }
                        }
                        _ => {}
                    }
                }
                out.push(vec![bulge_path(&pts, &bulges, closed)]);
            }
            "POLYLINE" => poly = Some((vec![], vec![], (f(d, 70) as i32) & 1 == 1)),
            "VERTEX" => {
                if let Some(p) = poly.as_mut() {
                    p.0.push([f(d, 10), f(d, 20)]);
                    p.1.push(f(d, 42));
                }
            }
            "SEQEND" => {
                if let Some((pts, b, closed)) = poly.take() {
                    out.push(vec![bulge_path(&pts, &b, closed)]);
                }
            }
            _ => {}
        }
    }
    out.retain(|s| s.iter().any(|p| p.pts.len() >= 2));
    Ok(out)
}

fn bulge_path(pts: &[P], bulges: &[f64], closed: bool) -> Path {
    let n = pts.len();
    let mut out = vec![];
    for i in 0..n {
        let a = pts[i];
        out.push(a);
        if !closed && i == n - 1 {
            break;
        }
        let b = pts[(i + 1) % n];
        let bulge = bulges.get(i).copied().unwrap_or(0.0);
        if bulge.abs() > 1e-9 {
            let theta = 4.0 * bulge.atan();
            let chord = geom::dist(a, b);
            if chord < 1e-9 {
                continue;
            }
            let r = chord / (2.0 * (theta / 2.0).sin());
            let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
            let h = r * (theta / 2.0).cos();
            let dir = [(b[0] - a[0]) / chord, (b[1] - a[1]) / chord];
            let c = [mid[0] - dir[1] * h, mid[1] + dir[0] * h];
            let a0 = (a[1] - c[1]).atan2(a[0] - c[0]);
            let steps = ((theta.abs().to_degrees() / 5.0).ceil() as usize).max(2);
            for s in 1..steps {
                let ang = a0 + theta * s as f64 / steps as f64;
                out.push([c[0] + r.abs() * ang.cos(), c[1] + r.abs() * ang.sin()]);
            }
        }
    }
    Path { pts: out, closed }
}

pub fn export_svg(shapes: &[Shape], w: f64, h: f64) -> String {
    let mut s = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"0 0 {w} {h}\">\n"
    );
    for sh in shapes {
        let mut d = String::new();
        for p in &sh.paths {
            for (i, q) in p.pts.iter().enumerate() {
                d.push_str(&format!("{}{:.3},{:.3} ", if i == 0 { "M" } else { "L" }, q[0], h - q[1]));
            }
            if p.closed {
                d.push_str("Z ");
            }
        }
        s.push_str(&format!("  <path d=\"{}\" fill=\"none\" stroke=\"black\" stroke-width=\"0.2\"/>\n", d.trim()));
    }
    s.push_str("</svg>\n");
    s
}

pub fn export_dxf(shapes: &[Shape]) -> String {
    let mut s = String::from("0\nSECTION\n2\nENTITIES\n");
    for sh in shapes {
        for p in &sh.paths {
            s.push_str(&format!("0\nLWPOLYLINE\n8\n0\n90\n{}\n70\n{}\n", p.pts.len(), if p.closed { 1 } else { 0 }));
            for q in &p.pts {
                s.push_str(&format!("10\n{:.4}\n20\n{:.4}\n", q[0], q[1]));
            }
        }
    }
    s.push_str("0\nENDSEC\n0\nEOF\n");
    s
}
