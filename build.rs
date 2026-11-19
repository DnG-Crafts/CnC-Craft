// Gives the Windows .exe its icon and version details: draws the icon (src/icon.rs) at the usual
// sizes and writes the resources straight into an object file that both the Microsoft and the GNU
// linkers take, so no resource compiler or extra tools are needed.

include!("src/icon.rs");

fn push16(b: &mut Vec<u8>, v: u16) {
    b.extend_from_slice(&v.to_le_bytes());
}
fn push32(b: &mut Vec<u8>, v: u32) {
    b.extend_from_slice(&v.to_le_bytes());
}
fn pad(b: &mut Vec<u8>, to: usize) {
    while b.len() % to != 0 {
        b.push(0);
    }
}

/// The icon image as a 32-bit bitmap (bottom-up BGRA plus an empty mask), as icons store it.
fn dib(size: u32) -> Vec<u8> {
    let px = icon_rgba(size);
    let mask_row = ((size + 31) / 32 * 4) as usize;
    let mut b = vec![];
    push32(&mut b, 40);
    push32(&mut b, size);
    push32(&mut b, size * 2);
    push16(&mut b, 1);
    push16(&mut b, 32);
    push32(&mut b, 0);
    push32(&mut b, size * size * 4 + (mask_row as u32) * size);
    for _ in 0..4 {
        push32(&mut b, 0);
    }
    for y in (0..size).rev() {
        for x in 0..size {
            let i = ((y * size + x) * 4) as usize;
            b.extend_from_slice(&[px[i + 2], px[i + 1], px[i], px[i + 3]]);
        }
    }
    b.extend(std::iter::repeat(0u8).take(mask_row * size as usize));
    b
}

fn utf16z(s: &str) -> Vec<u8> {
    s.encode_utf16().chain(std::iter::once(0)).flat_map(|c| c.to_le_bytes()).collect()
}

/// One block of the version resource: length, value length, type, key, value, children.
fn ver_block(key: &str, value: &[u8], value_len: u16, text: bool, children: &[Vec<u8>]) -> Vec<u8> {
    let mut b = vec![0, 0];
    push16(&mut b, value_len);
    push16(&mut b, if text { 1 } else { 0 });
    b.extend(utf16z(key));
    pad(&mut b, 4);
    b.extend_from_slice(value);
    for c in children {
        pad(&mut b, 4);
        b.extend_from_slice(c);
    }
    let len = b.len() as u16;
    b[0..2].copy_from_slice(&len.to_le_bytes());
    b
}

fn version_info() -> Vec<u8> {
    let ver: Vec<u16> = env!("CARGO_PKG_VERSION").split('.').map(|p| p.parse().unwrap_or(0)).chain(std::iter::repeat(0)).take(4).collect();
    let ms = ((ver[0] as u32) << 16) | ver[1] as u32;
    let ls = ((ver[2] as u32) << 16) | ver[3] as u32;
    let mut fixed = vec![];
    for v in [0xFEEF04BDu32, 0x0001_0000, ms, ls, ms, ls, 0x3F, 0, 0x0004_0004, 1, 0, 0, 0] {
        push32(&mut fixed, v);
    }
    let vs = format!("{}.{}.{}.{}", ver[0], ver[1], ver[2], ver[3]);
    let strings: Vec<Vec<u8>> = [
        ("CompanyName", "CncCraft"),
        ("FileDescription", "CncCraft - CNC router design, toolpaths and machine control"),
        ("FileVersion", vs.as_str()),
        ("InternalName", "cnccraft"),
        ("OriginalFilename", "cnccraft.exe"),
        ("ProductName", "CncCraft"),
        ("ProductVersion", vs.as_str()),
    ]
    .iter()
    .map(|(k, v)| {
        let val = utf16z(v);
        ver_block(k, &val, (val.len() / 2) as u16, true, &[])
    })
    .collect();
    let table = ver_block("040904B0", &[], 0, true, &strings);
    let sfi = ver_block("StringFileInfo", &[], 0, true, &[table]);
    let var = ver_block("Translation", &0x04B0_0409u32.to_le_bytes(), 4, false, &[]);
    let vfi = ver_block("VarFileInfo", &[], 0, true, &[var]);
    ver_block("VS_VERSION_INFO", &fixed, 52, false, &[sfi, vfi])
}

/// A COFF object holding the resources in a .rsrc section (what a resource compiler would make).
fn coff_resources(res: &[(u16, u16, Vec<u8>)], machine: u16, reloc_type: u16) -> Vec<u8> {
    const LANG: u32 = 0x0409;
    let mut types: Vec<u16> = res.iter().map(|r| r.0).collect();
    types.sort();
    types.dedup();
    let names_of = |t: u16| {
        let mut v: Vec<&(u16, u16, Vec<u8>)> = res.iter().filter(|r| r.0 == t).collect();
        v.sort_by_key(|r| r.1);
        v
    };
    // where everything goes in the section
    let dir = |n: usize| 16 + 8 * n;
    let mut off = dir(types.len());
    let mut type_dir = vec![];
    for &t in &types {
        type_dir.push(off);
        off += dir(names_of(t).len());
    }
    let mut lang_dir = vec![];
    for &t in &types {
        for _ in names_of(t) {
            lang_dir.push(off);
            off += dir(1);
        }
    }
    let mut data_entry = vec![];
    for _ in 0..lang_dir.len() {
        data_entry.push(off);
        off += 16;
    }
    let mut data_off = vec![];
    for &t in &types {
        for r in names_of(t) {
            off = (off + 7) / 8 * 8;
            data_off.push(off);
            off += r.2.len();
        }
    }
    let mut s = vec![];
    let header = |s: &mut Vec<u8>, n: usize| {
        push32(s, 0);
        push32(s, 0);
        push16(s, 0);
        push16(s, 0);
        push16(s, 0);
        push16(s, n as u16);
    };
    header(&mut s, types.len());
    for (i, &t) in types.iter().enumerate() {
        push32(&mut s, t as u32);
        push32(&mut s, 0x8000_0000 | type_dir[i] as u32);
    }
    let mut k = 0;
    for &t in &types {
        let names = names_of(t);
        header(&mut s, names.len());
        for (j, r) in names.iter().enumerate() {
            push32(&mut s, r.1 as u32);
            push32(&mut s, 0x8000_0000 | lang_dir[k + j] as u32);
        }
        k += names.len();
    }
    for (i, _) in lang_dir.iter().enumerate() {
        header(&mut s, 1);
        push32(&mut s, LANG);
        push32(&mut s, data_entry[i] as u32);
    }
    let all: Vec<&(u16, u16, Vec<u8>)> = types.iter().flat_map(|&t| names_of(t)).collect();
    let mut relocs = vec![];
    for (i, r) in all.iter().enumerate() {
        relocs.push(s.len() as u32);
        push32(&mut s, data_off[i] as u32); // the linker adds the section's address
        push32(&mut s, r.2.len() as u32);
        push32(&mut s, 0);
        push32(&mut s, 0);
    }
    for (i, r) in all.iter().enumerate() {
        while s.len() < data_off[i] {
            s.push(0);
        }
        s.extend_from_slice(&r.2);
    }
    pad(&mut s, 4);
    // the object file: header, one section, its data, relocations, one symbol
    let raw_at = 20 + 40;
    let reloc_at = raw_at + s.len();
    let sym_at = reloc_at + relocs.len() * 10;
    let mut o = vec![];
    push16(&mut o, machine);
    push16(&mut o, 1);
    push32(&mut o, 0);
    push32(&mut o, sym_at as u32);
    push32(&mut o, 1);
    push16(&mut o, 0);
    push16(&mut o, 0);
    o.extend_from_slice(b".rsrc\0\0\0");
    push32(&mut o, 0);
    push32(&mut o, 0);
    push32(&mut o, s.len() as u32);
    push32(&mut o, raw_at as u32);
    push32(&mut o, reloc_at as u32);
    push32(&mut o, 0);
    push16(&mut o, relocs.len() as u16);
    push16(&mut o, 0);
    push32(&mut o, 0x4030_0040); // initialised data, readable, 4-byte aligned
    o.extend_from_slice(&s);
    for r in relocs {
        push32(&mut o, r);
        push32(&mut o, 0);
        push16(&mut o, reloc_type);
    }
    o.extend_from_slice(b".rsrc\0\0\0");
    push32(&mut o, 0);
    push16(&mut o, 1);
    push16(&mut o, 0);
    o.push(3); // static
    o.push(0);
    push32(&mut o, 4); // empty string table
    o
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/icon.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    if std::env::var("CARGO_CFG_TARGET_OS").map(|t| t != "windows").unwrap_or(true) {
        return;
    }
    let (machine, reloc) = match std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default().as_str() {
        "x86_64" => (0x8664u16, 3u16),
        "x86" => (0x014c, 7),
        "aarch64" => (0xAA64, 2),
        _ => return,
    };
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").map(|t| t == "msvc").unwrap_or(false);
    let sizes = [16u32, 20, 24, 32, 40, 48, 64, 128, 256];
    let mut res: Vec<(u16, u16, Vec<u8>)> = vec![];
    let mut group = vec![];
    push16(&mut group, 0);
    push16(&mut group, 1);
    push16(&mut group, sizes.len() as u16);
    for (i, &s) in sizes.iter().enumerate() {
        let data = dib(s);
        let id = (i + 1) as u16;
        let wb = if s >= 256 { 0 } else { s as u8 };
        group.extend_from_slice(&[wb, wb, 0, 0]);
        push16(&mut group, 1);
        push16(&mut group, 32);
        push32(&mut group, data.len() as u32);
        push16(&mut group, id);
        res.push((3, id, data)); // RT_ICON
    }
    res.push((14, 1, group)); // RT_GROUP_ICON: the first one is the program's icon
    res.push((16, 1, version_info())); // RT_VERSION
    let obj = coff_resources(&res, machine, reloc);
    let path = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join(if msvc { "cnccraft_resources.obj" } else { "cnccraft_resources.o" });
    std::fs::write(&path, obj).expect("write the icon resources");
    println!("cargo:rustc-link-arg-bins={}", path.display());
}
