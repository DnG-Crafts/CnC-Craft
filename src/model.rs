//! Data model: shapes, tools, toolpaths, stock and machine settings.

use serde::{Deserialize, Serialize};

pub type P = [f64; 2];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Path {
    pub pts: Vec<P>,
    pub closed: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ShapeKind {
    Rect,
    Ellipse,
    Polygon,
    Line,
    Text,
    Point,
    Imported,
    Path,
}

#[allow(dead_code)]
impl ShapeKind {
    pub fn label(self) -> &'static str {
        match self {
            ShapeKind::Rect => "Rectangle",
            ShapeKind::Ellipse => "Ellipse",
            ShapeKind::Polygon => "Polygon",
            ShapeKind::Line => "Line",
            ShapeKind::Text => "Text",
            ShapeKind::Point => "Drill point",
            ShapeKind::Imported => "Imported",
            ShapeKind::Path => "Path",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Shape {
    pub id: u64,
    pub kind: ShapeKind,
    pub paths: Vec<Path>,
    #[serde(default)]
    pub group: Option<u64>,
    #[serde(default)]
    pub text: Option<String>,
    /// live effects: the original outline and the effects worked out from it (see effects.rs)
    #[serde(default)]
    pub fx: Option<LiveFx>,
}

/// Effects a shape carries that can be changed or removed later. paths on the shape is always
/// the result; cached is that result as last worked out (to notice when another tool changed it).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct LiveFx {
    pub base: Vec<Path>,
    pub effects: Vec<crate::effects::Effect>,
    pub cached: Vec<Path>,
}

impl Shape {
    pub fn bbox(&self) -> Option<(P, P)> {
        bbox_of(self.paths.iter().flat_map(|p| p.pts.iter()))
    }
    #[allow(dead_code)]
    pub fn is_closed(&self) -> bool {
        self.paths.iter().any(|p| p.closed)
    }
}

pub fn bbox_of<'a>(it: impl Iterator<Item = &'a P>) -> Option<(P, P)> {
    let mut mn = [f64::MAX, f64::MAX];
    let mut mx = [f64::MIN, f64::MIN];
    let mut any = false;
    for p in it {
        any = true;
        mn[0] = mn[0].min(p[0]);
        mn[1] = mn[1].min(p[1]);
        mx[0] = mx[0].max(p[0]);
        mx[1] = mx[1].max(p[1]);
    }
    if any {
        Some((mn, mx))
    } else {
        None
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ToolKind {
    FlatEnd,
    BallNose,
    VBit,
    Drill,
    DragKnife,
    ThreadMill,
}

impl ToolKind {
    pub const ALL: [ToolKind; 6] = [ToolKind::FlatEnd, ToolKind::BallNose, ToolKind::VBit, ToolKind::Drill, ToolKind::DragKnife, ToolKind::ThreadMill];
    pub fn label(self) -> &'static str {
        match self {
            ToolKind::FlatEnd => "Flat end mill",
            ToolKind::BallNose => "Ball nose",
            ToolKind::VBit => "V-bit",
            ToolKind::Drill => "Drill",
            ToolKind::DragKnife => "Drag knife",
            ToolKind::ThreadMill => "Thread mill (single point)",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Tool {
    pub id: u64,
    pub number: u32,
    pub name: String,
    pub kind: ToolKind,
    pub diameter: f64,
    /// Included angle for V-bits, degrees.
    pub angle: f64,
    pub flutes: u32,
    pub stepdown: f64,
    pub stepover_pct: f64,
    pub feed: f64,
    pub plunge: f64,
    pub rpm: f64,
    /// how deep the bit can cut (0 = not set)
    #[serde(default)]
    pub flute_length: f64,
}

impl Tool {
    pub fn radius(&self) -> f64 {
        self.diameter / 2.0
    }
}

pub fn default_tools() -> Vec<Tool> {
    vec![
        Tool { id: 1, number: 1, name: "1/4\" Flat end mill".into(), kind: ToolKind::FlatEnd, diameter: 6.35, angle: 90.0, flutes: 2, stepdown: 2.0, stepover_pct: 40.0, feed: 1500.0, plunge: 400.0, rpm: 18000.0, flute_length: 0.0 },
        Tool { id: 2, number: 2, name: "1/8\" Flat end mill".into(), kind: ToolKind::FlatEnd, diameter: 3.175, angle: 90.0, flutes: 2, stepdown: 1.0, stepover_pct: 40.0, feed: 1000.0, plunge: 300.0, rpm: 18000.0, flute_length: 0.0 },
        Tool { id: 3, number: 3, name: "60° V-bit".into(), kind: ToolKind::VBit, diameter: 12.7, angle: 60.0, flutes: 2, stepdown: 1.5, stepover_pct: 20.0, feed: 1000.0, plunge: 300.0, rpm: 18000.0, flute_length: 0.0 },
        Tool { id: 4, number: 4, name: "1/4\" Ball nose".into(), kind: ToolKind::BallNose, diameter: 6.35, angle: 90.0, flutes: 2, stepdown: 1.5, stepover_pct: 15.0, feed: 1200.0, plunge: 300.0, rpm: 18000.0, flute_length: 0.0 },
        Tool { id: 5, number: 5, name: "5 mm Drill".into(), kind: ToolKind::Drill, diameter: 5.0, angle: 118.0, flutes: 2, stepdown: 3.0, stepover_pct: 50.0, feed: 600.0, plunge: 250.0, rpm: 12000.0, flute_length: 0.0 },
        Tool { id: 6, number: 6, name: "Drag knife (0.25 mm offset)".into(), kind: ToolKind::DragKnife, diameter: 0.5, angle: 45.0, flutes: 1, stepdown: 1.0, stepover_pct: 50.0, feed: 1500.0, plunge: 300.0, rpm: 0.0, flute_length: 0.0 },
        Tool { id: 7, number: 7, name: "Thread mill 60° (6 mm)".into(), kind: ToolKind::ThreadMill, diameter: 6.0, angle: 60.0, flutes: 3, stepdown: 1.0, stepover_pct: 50.0, feed: 400.0, plunge: 150.0, rpm: 16000.0, flute_length: 15.0 },
    ]
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum OpKind {
    ProfileOutside,
    ProfileInside,
    ProfileOnLine,
    Pocket,
    Drill,
    VCarve,
    Surface,
    Chamfer,
    Fluting,
    Keyhole,
    Texture,
    Prism,
    DragKnife,
    Thread,
    Moulding,
    Dogbone,
    Bore,
    Rough3D,
    Finish3D,
}

impl OpKind {
    pub const ALL: [OpKind; 19] = [
        OpKind::ProfileOutside,
        OpKind::ProfileInside,
        OpKind::ProfileOnLine,
        OpKind::Pocket,
        OpKind::Drill,
        OpKind::VCarve,
        OpKind::Surface,
        OpKind::Chamfer,
        OpKind::Fluting,
        OpKind::Keyhole,
        OpKind::Texture,
        OpKind::Prism,
        OpKind::DragKnife,
        OpKind::Thread,
        OpKind::Moulding,
        OpKind::Dogbone,
        OpKind::Bore,
        OpKind::Rough3D,
        OpKind::Finish3D,
    ];
    pub fn label(self) -> &'static str {
        match self {
            OpKind::ProfileOutside => "Profile - outside",
            OpKind::ProfileInside => "Profile - inside",
            OpKind::ProfileOnLine => "Engrave - on line",
            OpKind::Pocket => "Pocket",
            OpKind::Drill => "Drill",
            OpKind::VCarve => "V-Carve",
            OpKind::Surface => "Surfacing (flatten)",
            OpKind::Chamfer => "Chamfer (V-bit edge)",
            OpKind::Fluting => "Fluting (ramped grooves)",
            OpKind::Keyhole => "Keyhole (hanging slots)",
            OpKind::Texture => "Texture (carved scallops)",
            OpKind::Prism => "Prism (raised bevel)",
            OpKind::DragKnife => "Drag knife",
            OpKind::Thread => "Thread milling",
            OpKind::Moulding => "Moulding (swept profile)",
            OpKind::Dogbone => "Dogbone slot (square inside corners)",
            OpKind::Bore => "Bore (spiral round holes)",
            OpKind::Rough3D => "3D rough (clear a relief)",
            OpKind::Finish3D => "3D finish (smooth a relief)",
        }
    }
    pub fn short(self) -> &'static str {
        match self {
            OpKind::ProfileOutside => "Outside",
            OpKind::ProfileInside => "Inside",
            OpKind::ProfileOnLine => "On line",
            OpKind::Pocket => "Pocket",
            OpKind::Drill => "Drill",
            OpKind::VCarve => "V-Carve",
            OpKind::Surface => "Surface",
            OpKind::Chamfer => "Chamfer",
            OpKind::Fluting => "Fluting",
            OpKind::Keyhole => "Keyhole",
            OpKind::Texture => "Texture",
            OpKind::Prism => "Prism",
            OpKind::DragKnife => "Knife",
            OpKind::Thread => "Thread",
            OpKind::Moulding => "Moulding",
            OpKind::Dogbone => "Dogbone",
            OpKind::Bore => "Bore",
            OpKind::Rough3D => "3D rough",
            OpKind::Finish3D => "3D finish",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum PocketStyle {
    Offset,
    Raster,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Toolpath {
    pub id: u64,
    pub name: String,
    pub op: OpKind,
    pub tool_id: u64,
    pub shape_ids: Vec<u64>,
    pub enabled: bool,
    pub color: [u8; 3],
    pub start_depth: f64,
    pub cut_depth: f64,
    pub stepdown: f64,
    pub stepover_pct: f64,
    pub feed: f64,
    pub plunge: f64,
    pub rpm: f64,
    pub climb: bool,
    pub allowance: f64,
    pub pocket_style: PocketStyle,
    pub raster_angle: f64,
    pub tabs: bool,
    pub tab_count: u32,
    pub tab_width: f64,
    pub tab_height: f64,
    pub ramp: bool,
    pub peck: f64,
    pub vcarve_max_depth: f64,
    /// Chamfer: width of the bevel on the top edge
    #[serde(default)]
    pub chamfer_width: f64,
    /// Surfacing: rows run along Y instead of X
    #[serde(default)]
    pub surface_along_y: bool,
    /// Profile leads: come in (and go out) from the waste side
    #[serde(default)]
    pub lead: bool,
    #[serde(default)]
    pub lead_arc: bool,
    #[serde(default)]
    pub lead_len: f64,
    /// run past the start of a closed profile before leaving, so no step is left
    #[serde(default)]
    pub overcut: f64,
    /// 3D tabs ramp up and down instead of square steps
    #[serde(default)]
    pub tabs3d: bool,
    /// Fluting: 0 ramp the whole length, 1 ramp at the start, 2 ramp at both ends
    #[serde(default)]
    pub flute_type: u8,
    #[serde(default)]
    pub flute_ramp_pct: f64,
    #[serde(default)]
    pub flute_smooth: bool,
    /// Keyhole: slot length and direction (0 up, 1 down, 2 right, 3 left)
    #[serde(default)]
    pub keyhole_len: f64,
    #[serde(default)]
    pub keyhole_dir: u8,
    /// Texture: longest scallop, shortest as %, shallowest as % of the depth, overlap %, seed
    #[serde(default)]
    pub tex_len: f64,
    #[serde(default)]
    pub tex_len_min: f64,
    #[serde(default)]
    pub tex_depth_min: f64,
    #[serde(default)]
    pub tex_overlap: f64,
    #[serde(default)]
    pub tex_seed: u64,
    /// Drag knife: blade offset (pivot to tip), corners turning more than this are swivelled,
    /// swivel depth as a % of the cut depth
    #[serde(default)]
    pub blade_offset: f64,
    #[serde(default)]
    pub knife_tol: f64,
    #[serde(default)]
    pub swivel_pct: f64,
    /// Thread milling: pitch, major diameter (0 = the shape's size), internal (nut) or external (bolt)
    #[serde(default)]
    pub thread_pitch: f64,
    #[serde(default)]
    pub thread_dia: f64,
    #[serde(default)]
    pub thread_internal: bool,
    /// Moulding: the profile runs outward from closed rails (to the right of open ones) instead of inward
    #[serde(default)]
    pub mould_outward: bool,
    /// Dogbone: clear the inside too (off = only the outline, for cut-through slots)
    #[serde(default)]
    pub dogbone_clear: bool,
    /// Bore: hole diameter (0 = each shape's width)
    #[serde(default)]
    pub bore_dia: f64,
}

pub const PALETTE: [[u8; 3]; 8] = [
    [240, 120, 40],
    [60, 170, 255],
    [90, 210, 110],
    [230, 80, 160],
    [250, 210, 60],
    [160, 110, 255],
    [60, 220, 210],
    [255, 90, 80],
];

impl Toolpath {
    pub fn new(id: u64, op: OpKind, tool: &Tool, shapes: Vec<u64>, index: usize) -> Self {
        Toolpath {
            id,
            name: format!("{} {}", op.short(), index + 1),
            op,
            tool_id: tool.id,
            shape_ids: shapes,
            enabled: true,
            color: PALETTE[index % PALETTE.len()],
            start_depth: 0.0,
            cut_depth: match op {
                OpKind::ProfileOnLine => 1.0,
                OpKind::Pocket => 3.0,
                OpKind::VCarve => 0.0,
                OpKind::Surface => 0.5,
                OpKind::Chamfer => 0.0,
                OpKind::Fluting => 3.0,
                OpKind::Keyhole => 8.0,
                OpKind::Texture => 1.5,
                OpKind::Prism => 3.0,
                OpKind::DragKnife => 1.0,
                OpKind::Thread => 10.0,
                OpKind::Moulding => 6.0,
                OpKind::Dogbone => 6.0,
                OpKind::Bore => 10.0,
                OpKind::Rough3D | OpKind::Finish3D => 6.0,
                _ => 6.0,
            },
            stepdown: tool.stepdown,
            stepover_pct: tool.stepover_pct,
            feed: tool.feed,
            plunge: tool.plunge,
            rpm: tool.rpm,
            climb: true,
            allowance: 0.0,
            pocket_style: PocketStyle::Offset,
            raster_angle: 0.0,
            tabs: false,
            tab_count: 4,
            tab_width: 6.0,
            tab_height: 1.5,
            ramp: false,
            peck: 3.0,
            vcarve_max_depth: 4.0,
            chamfer_width: 2.0,
            surface_along_y: false,
            lead: false,
            lead_arc: true,
            lead_len: 5.0,
            overcut: 0.0,
            tabs3d: false,
            flute_type: 2,
            flute_ramp_pct: 30.0,
            flute_smooth: true,
            keyhole_len: 10.0,
            keyhole_dir: 0,
            tex_len: 20.0,
            tex_len_min: 50.0,
            tex_depth_min: 50.0,
            tex_overlap: 20.0,
            tex_seed: 7,
            blade_offset: 0.25,
            knife_tol: 10.0,
            swivel_pct: 25.0,
            thread_pitch: 1.5,
            thread_dia: 0.0,
            thread_internal: true,
            mould_outward: false,
            dogbone_clear: true,
            bore_dia: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ZZero {
    StockTop,
    Spoilboard,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum XyOrigin {
    FrontLeft,
    FrontRight,
    BackLeft,
    BackRight,
    Center,
}

impl XyOrigin {
    pub const ALL: [XyOrigin; 5] = [XyOrigin::FrontLeft, XyOrigin::FrontRight, XyOrigin::BackLeft, XyOrigin::BackRight, XyOrigin::Center];
    pub fn label(self) -> &'static str {
        match self {
            XyOrigin::FrontLeft => "Front left",
            XyOrigin::FrontRight => "Front right",
            XyOrigin::BackLeft => "Back left",
            XyOrigin::BackRight => "Back right",
            XyOrigin::Center => "Center",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Stock {
    pub width: f64,
    pub height: f64,
    pub thickness: f64,
    pub z_zero: ZZero,
    pub origin: XyOrigin,
    pub material: String,
}

impl Default for Stock {
    fn default() -> Self {
        Stock { width: 300.0, height: 200.0, thickness: 18.0, z_zero: ZZero::StockTop, origin: XyOrigin::FrontLeft, material: "Plywood".into() }
    }
}

impl Stock {
    /// Offset that converts design coordinates (stock front-left = 0,0) into job coordinates.
    pub fn job_offset(&self) -> P {
        match self.origin {
            XyOrigin::FrontLeft => [0.0, 0.0],
            XyOrigin::FrontRight => [-self.width, 0.0],
            XyOrigin::BackLeft => [0.0, -self.height],
            XyOrigin::BackRight => [-self.width, -self.height],
            XyOrigin::Center => [-self.width / 2.0, -self.height / 2.0],
        }
    }
    /// Z value of the stock top in job coordinates.
    pub fn top_z(&self) -> f64 {
        match self.z_zero {
            ZZero::StockTop => 0.0,
            ZZero::Spoilboard => self.thickness,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document {
    pub shapes: Vec<Shape>,
    pub toolpaths: Vec<Toolpath>,
    pub stock: Stock,
    pub next_id: u64,
    #[serde(default)]
    pub locked: Vec<u64>,
    /// ruler guides: (vertical?, position in mm)
    #[serde(default)]
    pub guides: Vec<(bool, f64)>,
    /// dimension lines (never cut)
    #[serde(default)]
    pub dims: Vec<(P, P)>,
    /// shapes marking clamps and other keep-out zones (never cut)
    #[serde(default)]
    pub keepout: Vec<u64>,
    /// 3D reliefs (from a picture or a 3D model), each filling the box of its frame shape
    #[serde(default)]
    pub reliefs: Vec<Relief>,
}

/// A height map laid over the box of its frame shape: 0 = deepest, 65535 = the top surface.
/// Row 0 is the back (largest Y).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Relief {
    pub shape_id: u64,
    pub name: String,
    pub w: usize,
    pub h: usize,
    pub heights: Vec<u16>,
    /// how deep the lowest point is carved, mm
    pub depth: f64,
}

impl Relief {
    /// Height 0..1 at (u, v) in 0..1 across the box (v = 0 at the front), smoothly between cells.
    pub fn sample(&self, u: f64, v: f64) -> f64 {
        if self.w == 0 || self.h == 0 {
            return 1.0;
        }
        let x = (u.clamp(0.0, 1.0) * self.w as f64 - 0.5).clamp(0.0, (self.w - 1) as f64);
        let y = ((1.0 - v.clamp(0.0, 1.0)) * self.h as f64 - 0.5).clamp(0.0, (self.h - 1) as f64);
        let (x0, y0) = (x.floor() as usize, y.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(self.w - 1), (y0 + 1).min(self.h - 1));
        let (fx, fy) = (x - x0 as f64, y - y0 as f64);
        let g = |i: usize, j: usize| self.heights[j * self.w + i] as f64 / 65535.0;
        let a = g(x0, y0) * (1.0 - fx) + g(x1, y0) * fx;
        let b = g(x0, y1) * (1.0 - fx) + g(x1, y1) * fx;
        a * (1.0 - fy) + b * fy
    }
}

impl Default for Document {
    fn default() -> Self {
        Document { shapes: vec![], toolpaths: vec![], stock: Stock::default(), next_id: 100, locked: vec![], guides: vec![], dims: vec![], keepout: vec![], reliefs: vec![] }
    }
}

impl Document {
    pub fn new_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }
    pub fn shape(&self, id: u64) -> Option<&Shape> {
        self.shapes.iter().find(|s| s.id == id)
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Units {
    Mm,
    Inch,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MachineSettings {
    pub name: String,
    pub bed_w: f64,
    pub bed_h: f64,
    pub z_travel: f64,
    pub safe_z: f64,
    pub rapid_rate: f64,
    pub port: String,
    pub baud: u32,
    pub header: String,
    pub footer: String,
    pub spindle_delay: f64,
    pub home_on_start: bool,
    pub return_to_origin: bool,
    pub inch_gcode: bool,
    pub tool_change_mode: bool,
    /// write G2/G3 arcs where moves follow circles (smoother, shorter files)
    #[serde(default = "default_true")]
    pub arcs: bool,
    /// which controller the G-code is written for
    #[serde(default)]
    pub dialect: Dialect,
    /// fastest spindle speed and feed (for the feeds and speeds calculator)
    #[serde(default = "default_max_rpm")]
    pub max_rpm: f64,
    #[serde(default = "default_max_feed")]
    pub max_feed: f64,
}

fn default_max_rpm() -> f64 {
    24000.0
}
fn default_max_feed() -> f64 {
    5000.0
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum Dialect {
    #[default]
    Grbl,
    Mach3,
    LinuxCnc,
    Marlin,
}

impl Dialect {
    pub const ALL: [Dialect; 4] = [Dialect::Grbl, Dialect::Mach3, Dialect::LinuxCnc, Dialect::Marlin];
    pub fn label(self) -> &'static str {
        match self {
            Dialect::Grbl => "GRBL",
            Dialect::Mach3 => "Mach3 / Mach4",
            Dialect::LinuxCnc => "LinuxCNC",
            Dialect::Marlin => "Marlin",
        }
    }
}

fn default_true() -> bool {
    true
}

impl Default for MachineSettings {
    fn default() -> Self {
        MachineSettings {
            name: "My CNC router".into(),
            bed_w: 600.0,
            bed_h: 400.0,
            z_travel: 90.0,
            safe_z: 5.0,
            rapid_rate: 3000.0,
            port: "Simulator".into(),
            baud: 115200,
            header: String::new(),
            footer: String::new(),
            spindle_delay: 3.0,
            home_on_start: false,
            return_to_origin: true,
            inch_gcode: false,
            tool_change_mode: true,
            arcs: true,
            dialect: Dialect::Grbl,
            max_rpm: 24000.0,
            max_feed: 5000.0,
        }
    }
}

/// A machine the person added: all its machine settings plus the clamp height and file splitting.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedMachine {
    pub machine: MachineSettings,
    #[serde(default)]
    pub clamp_height: f64,
    #[serde(default)]
    pub split_by_tool: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    /// the person's own machines, picked from the machine list
    pub my_machines: Vec<SavedMachine>,
    /// which of my_machines the settings were loaded from (None = not one of them)
    pub current_machine: Option<usize>,
    pub machine: MachineSettings,
    pub tools: Vec<Tool>,
    pub units: Units,
    pub snap_grid: bool,
    pub grid: f64,
    pub plate_thickness: f64,
    pub light_theme: bool,
    pub nudge: f64,
    pub recent: Vec<String>,
    pub art_folder: String,
    /// saved toolpath settings to start new toolpaths from
    pub templates: Vec<Toolpath>,
    /// keep-out height of clamps above the stock top (for the checks)
    pub clamp_height: f64,
    /// save one G-code file per tool (for machines without a tool changer)
    pub split_by_tool: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            my_machines: vec![],
            current_machine: None,
            machine: MachineSettings::default(),
            tools: default_tools(),
            units: Units::Mm,
            snap_grid: true,
            grid: 10.0,
            plate_thickness: 15.0,
            light_theme: false,
            nudge: 1.0,
            recent: vec![],
            art_folder: String::new(),
            templates: vec![],
            clamp_height: 15.0,
            split_by_tool: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectFile {
    pub app: String,
    pub version: u32,
    pub document: Document,
    pub tools: Vec<Tool>,
}
