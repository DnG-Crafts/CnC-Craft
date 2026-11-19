// The CncCraft icon, drawn from simple shapes: a router bit carving a groove into a wooden board
// on a dark rounded tile. Shared by build.rs (the .exe icon) and the app (the window icon).

/// Colour (straight RGBA, 0..1) of the icon at (u, v) in 0..1, v going down; alpha 0 outside.
fn icon_paint(u: f32, v: f32) -> [f32; 4] {
    fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
        let t = t.clamp(0.0, 1.0);
        [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
    }
    fn hex(c: u32) -> [f32; 3] {
        [((c >> 16) & 255) as f32 / 255.0, ((c >> 8) & 255) as f32 / 255.0, (c & 255) as f32 / 255.0]
    }
    // rounded tile
    let (lo, hi, rad) = (0.03f32, 0.97f32, 0.2f32);
    let qx = (u - 0.5).abs() - (0.5 - lo - rad);
    let qy = (v - 0.5).abs() - (hi - 0.5 - rad);
    let edge = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - rad;
    if edge > 0.0 {
        return [0.0; 4];
    }
    let mut c = mix(hex(0x2e3845), hex(0x1a2027), (v - lo) / (hi - lo));
    // wooden board with grain
    let board = 0.70;
    if v > board {
        let grain = ((v * 70.0 + (u * 6.0).sin() * 2.2).sin() * 0.5 + 0.5) * 0.18;
        c = mix(hex(0xd29d5e), hex(0x8e5f30), grain + (v - board) * 0.9);
        if v < board + 0.018 {
            c = hex(0xeec48c);
        }
        // the carved groove, from the left up to the bit
        let gy = board + 0.045;
        if u > 0.12 && u < 0.5 && (v - gy).abs() < 0.03 {
            c = mix(hex(0x4a2e16), hex(0x6e4622), (v - (gy - 0.03)) / 0.06);
        }
        let d = ((u - 0.12).powi(2) + (v - gy).powi(2)).sqrt();
        if d < 0.03 {
            c = mix(hex(0x4a2e16), hex(0x6e4622), (v - (gy - 0.03)) / 0.06);
        }
    }
    // warm glow where the bit cuts
    let gd = ((u - 0.5).powi(2) + ((v - 0.72) * 1.6).powi(2)).sqrt();
    if gd < 0.2 {
        c = mix(c, hex(0xffa040), (1.0 - gd / 0.2).powi(2) * 0.45);
    }
    // steel shank
    let steel = |du: f32| mix(hex(0xeef1f5), hex(0x7d8794), (du / 0.07).abs().powf(1.4));
    if (u - 0.5).abs() < 0.055 && v > 0.12 && v < 0.38 {
        c = steel((u - 0.5) * 1.27);
    }
    // orange collet nut
    if (u - 0.5).abs() < 0.13 && v > 0.07 && v < 0.2 {
        let t = (u - 0.5).abs() / 0.13;
        c = mix(hex(0xf7a04a), hex(0xc4621a), t.powf(1.5));
        if v < 0.09 {
            c = hex(0xffc27a);
        }
    }
    // fluted cutter with a spiral
    if (u - 0.5).abs() < 0.07 && v >= 0.38 && v < 0.75 {
        let du = u - 0.5;
        c = steel(du);
        let s = (v * 10.0 + du * 9.0).fract();
        if s < 0.38 {
            c = mix(c, hex(0x3d4550), 0.75);
        }
    }
    // flying chips
    for (x, y, r) in [(0.66f32, 0.62f32, 0.03f32), (0.75, 0.53, 0.022), (0.64, 0.49, 0.016), (0.32, 0.6, 0.018)] {
        if ((u - x).powi(2) + (v - y).powi(2)).sqrt() < r {
            c = hex(0xf0c890);
        }
    }
    // thin light rim
    if edge > -0.012 {
        c = mix(c, hex(0x56636f), 0.7);
    }
    [c[0], c[1], c[2], 1.0]
}

/// The icon at `size` × `size` pixels as straight RGBA bytes, smoothed by 4 × 4 samples a pixel.
pub fn icon_rgba(size: u32) -> Vec<u8> {
    let n = 4;
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let mut acc = [0.0f32; 4];
            for sy in 0..n {
                for sx in 0..n {
                    let u = (x as f32 + (sx as f32 + 0.5) / n as f32) / size as f32;
                    let v = (y as f32 + (sy as f32 + 0.5) / n as f32) / size as f32;
                    let p = icon_paint(u, v);
                    acc[0] += p[0] * p[3];
                    acc[1] += p[1] * p[3];
                    acc[2] += p[2] * p[3];
                    acc[3] += p[3];
                }
            }
            let a = acc[3] / (n * n) as f32;
            let k = if acc[3] > 0.0 { 1.0 / acc[3] } else { 0.0 };
            out.extend_from_slice(&[(acc[0] * k * 255.0).round() as u8, (acc[1] * k * 255.0).round() as u8, (acc[2] * k * 255.0).round() as u8, (a * 255.0).round() as u8]);
        }
    }
    out
}
