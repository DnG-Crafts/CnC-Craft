//! Talking to a GRBL-style controller over USB serial (or a built-in simulator).

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

pub const SIMULATOR: &str = "Simulator";

pub enum Cmd {
    Connect(String, u32),
    Disconnect,
    Line(String),
    Realtime(u8),
    Job(Vec<String>),
    StopJob,
    /// how many times faster than real time the simulator runs
    SimSpeed(f64),
}

pub enum Event {
    Log(String, LogKind),
    Status { state: String, mpos: [f64; 3], wpos: [f64; 3], feed: f64, spindle: f64 },
    Connected(String),
    Disconnected(String),
    Progress(usize, usize),
    JobDone(bool),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LogKind {
    Sent,
    Received,
    Info,
    Error,
}

pub fn list_ports() -> Vec<String> {
    let mut v = vec![SIMULATOR.to_string()];
    if let Ok(ports) = serialport::available_ports() {
        v.extend(ports.into_iter().map(|p| p.port_name));
    }
    v
}

pub struct Machine {
    tx: Sender<Cmd>,
    pub rx: Receiver<Event>,
}

impl Machine {
    pub fn start(ctx: egui::Context) -> Machine {
        let (tx, crx) = channel::<Cmd>();
        let (etx, rx) = channel::<Event>();
        std::thread::spawn(move || worker(crx, etx, ctx));
        Machine { tx, rx }
    }
    pub fn send(&self, c: Cmd) {
        let _ = self.tx.send(c);
    }
}

trait Link {
    fn write_bytes(&mut self, b: &[u8]) -> bool;
    fn read_some(&mut self, buf: &mut [u8]) -> usize;
    fn set_speed(&mut self, _s: f64) {}
}

struct SerialLink(Box<dyn serialport::SerialPort>);

impl Link for SerialLink {
    fn write_bytes(&mut self, b: &[u8]) -> bool {
        self.0.write_all(b).is_ok()
    }
    fn read_some(&mut self, buf: &mut [u8]) -> usize {
        match self.0.read(buf) {
            Ok(n) => n,
            Err(_) => 0,
        }
    }
}

/// One queued motion (or pause) in the pretend controller's planner.
struct Block {
    target: [f64; 3],
    /// mm/min; 0 means rapid
    feed: f64,
    dwell: f64,
    jog: bool,
}

const SIM_BUFFER: usize = 15;
const SIM_RAPID: f64 = 5000.0;

/// A pretend GRBL controller so everything can be tried without a machine.
/// Moves are queued in a 15-block planner and played back in order at their feed rates,
/// like a real controller, so jobs, frames and jogs follow the real path.
struct SimLink {
    out: VecDeque<u8>,
    inbuf: String,
    mpos: [f64; 3],
    wco: [f64; 3],
    /// where the machine will be once the queue is empty
    planned: [f64; 3],
    queue: VecDeque<Block>,
    pending_ok: usize,
    relative: bool,
    inch: bool,
    rapid_mode: bool,
    feed: f64,
    cur_feed: f64,
    spindle: f64,
    hold: bool,
    alarm: bool,
    speed: f64,
    last_tick: Instant,
}

impl SimLink {
    fn new() -> SimLink {
        let mut s = SimLink {
            out: VecDeque::new(),
            inbuf: String::new(),
            mpos: [0.0, 0.0, 0.0],
            wco: [0.0, 0.0, 0.0],
            planned: [0.0, 0.0, 0.0],
            queue: VecDeque::new(),
            pending_ok: 0,
            relative: false,
            inch: false,
            rapid_mode: true,
            feed: 1000.0,
            cur_feed: 0.0,
            spindle: 0.0,
            hold: false,
            alarm: false,
            speed: 5.0,
            last_tick: Instant::now(),
        };
        s.reply("\r\nGrbl 1.1h ['$' for help] (CncCraft simulator)");
        s
    }
    fn reply(&mut self, s: &str) {
        self.out.extend(s.bytes());
        self.out.extend(b"\r\n");
    }
    /// Acknowledges a line now, or later when the planner has room (like GRBL's buffer).
    fn ack(&mut self) {
        if self.queue.len() < SIM_BUFFER {
            self.reply("ok");
        } else {
            self.pending_ok += 1;
        }
    }
    fn push(&mut self, b: Block) {
        self.planned = b.target;
        self.queue.push_back(b);
    }
    fn tick(&mut self) {
        let dt = self.last_tick.elapsed().as_secs_f64();
        self.last_tick = Instant::now();
        if self.hold {
            self.cur_feed = 0.0;
            return;
        }
        let mut time_left = dt * self.speed;
        self.cur_feed = 0.0;
        while time_left > 0.0 {
            let Some(front) = self.queue.front_mut() else { break };
            if front.dwell > 0.0 {
                let used = front.dwell.min(time_left);
                front.dwell -= used;
                time_left -= used;
                if front.dwell > 1e-9 {
                    break;
                }
            } else {
                let t = front.target;
                let feed = if front.feed > 0.0 { front.feed } else { SIM_RAPID };
                let d: f64 = (0..3).map(|i| (t[i] - self.mpos[i]).powi(2)).sum::<f64>().sqrt();
                let v = feed / 60.0;
                self.cur_feed = feed;
                let need = d / v;
                if need > time_left {
                    let f = v * time_left / d;
                    for i in 0..3 {
                        self.mpos[i] += (t[i] - self.mpos[i]) * f;
                    }
                    break;
                }
                self.mpos = t;
                time_left -= need;
            }
            self.queue.pop_front();
            if self.pending_ok > 0 && self.queue.len() < SIM_BUFFER {
                self.pending_ok -= 1;
                self.reply("ok");
            }
        }
    }
    fn exec(&mut self, line: &str) {
        let l = line.trim().to_uppercase();
        let l = l.split(';').next().unwrap_or("").trim().to_string();
        let l: String = if let (Some(a), Some(b)) = (l.find('('), l.find(')')) { format!("{}{}", &l[..a], &l[b + 1..]) } else { l };
        if l.trim().is_empty() {
            self.reply("ok");
            return;
        }
        if self.alarm && !(l.starts_with("$X") || l.starts_with("$H")) {
            self.reply("error:9");
            return;
        }
        if l.starts_with("$H") {
            self.alarm = false;
            self.queue.clear();
            self.mpos = [0.0, 0.0, 0.0];
            self.planned = self.mpos;
            self.reply("ok");
            return;
        }
        if l.starts_with("$X") {
            self.alarm = false;
            self.reply("[MSG:Caution: Unlocked]");
            self.reply("ok");
            return;
        }
        if l == "$$" {
            for s in ["$100=80.000", "$101=80.000", "$102=400.000", "$110=5000.000", "$111=5000.000", "$112=1000.000", "$130=600.000", "$131=400.000", "$132=90.000"] {
                self.reply(s);
            }
            self.reply("ok");
            return;
        }
        if let Some(body) = l.strip_prefix("$J=") {
            let body = body.to_string();
            self.run_words(&body, true);
            self.ack();
            return;
        }
        if l.starts_with('$') {
            self.reply("ok");
            return;
        }
        self.run_words(&l, false);
        self.ack();
    }
    fn run_words(&mut self, l: &str, jog: bool) {
        let mut words: Vec<(char, f64)> = vec![];
        let chars: Vec<char> = l.chars().filter(|c| !c.is_whitespace()).collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c.is_ascii_alphabetic() {
                let mut j = i + 1;
                while j < chars.len() && (chars[j].is_ascii_digit() || chars[j] == '.' || chars[j] == '-' || chars[j] == '+') {
                    j += 1;
                }
                let v: f64 = chars[i + 1..j].iter().collect::<String>().parse().unwrap_or(0.0);
                words.push((c, v));
                i = j;
            } else {
                i += 1;
            }
        }
        let mut rel = if jog { false } else { self.relative };
        let mut inch = self.inch;
        let mut g10 = false;
        let mut probe = false;
        let mut dwell: Option<f64> = None;
        let mut rapid = self.rapid_mode;
        let mut arc: Option<bool> = None; // Some(clockwise)
        let (mut ai, mut aj) = (0.0, 0.0);
        for (c, v) in &words {
            match (c, *v as i32, (*v * 10.0).round() as i32) {
                ('G', 0, _) => rapid = true,
                ('G', 1, _) => rapid = false,
                ('G', 2, _) => {
                    rapid = false;
                    arc = Some(true)
                }
                ('G', 3, _) => {
                    rapid = false;
                    arc = Some(false)
                }
                ('G', 4, _) => dwell = Some(0.0),
                ('G', 90, _) => rel = false,
                ('G', 91, _) => rel = true,
                ('G', 20, _) => inch = true,
                ('G', 21, _) => inch = false,
                ('G', 10, _) => g10 = true,
                ('G', 38, _) => probe = true,
                ('M', 3, _) | ('M', 4, _) => {
                    if self.spindle == 0.0 {
                        self.spindle = 10000.0
                    }
                }
                ('M', 5, _) | ('M', 30, _) | ('M', 2, _) => self.spindle = 0.0,
                _ => {}
            }
        }
        if !jog {
            self.relative = rel;
            self.inch = inch;
            self.rapid_mode = rapid;
        }
        let k = if inch { 25.4 } else { 1.0 };
        let mut tgt = self.planned;
        let mut any = false;
        let mut feed = self.feed;
        for (c, v) in &words {
            let idx = match c {
                'X' => 0,
                'Y' => 1,
                'Z' => 2,
                'F' => {
                    feed = v * k;
                    continue;
                }
                'S' => {
                    self.spindle = *v;
                    continue;
                }
                'I' => {
                    ai = v * k;
                    continue;
                }
                'J' => {
                    aj = v * k;
                    continue;
                }
                'P' => {
                    if dwell.is_some() {
                        dwell = Some(*v);
                    }
                    continue;
                }
                _ => continue,
            };
            any = true;
            if g10 {
                // set work coordinate: work = machine - wco
                self.wco[idx] = self.planned[idx] - v * k;
            } else if rel {
                tgt[idx] += v * k;
            } else {
                tgt[idx] = v * k + self.wco[idx];
            }
        }
        if !jog {
            self.feed = feed;
        }
        if let Some(sec) = dwell {
            self.push(Block { target: self.planned, feed: 0.0, dwell: sec.max(0.0) + 1e-6, jog: false });
            return;
        }
        if probe {
            // the plate is touched after a short travel
            let mut t = self.planned;
            t[2] -= 12.0;
            self.push(Block { target: t, feed: feed.max(10.0), dwell: 0.0, jog: false });
            self.reply(&format!("[PRB:{:.3},{:.3},{:.3}:1]", t[0], t[1], t[2]));
            return;
        }
        if any && !g10 {
            let f = if jog { feed } else if rapid { 0.0 } else { feed.max(1.0) };
            if let Some(cw) = arc {
                // follow the arc in short steps round its centre (I, J from the start)
                let s = self.planned;
                let c = [s[0] + ai, s[1] + aj];
                let r = (ai * ai + aj * aj).sqrt().max(1e-9);
                let a0 = (s[1] - c[1]).atan2(s[0] - c[0]);
                let mut a1 = (tgt[1] - c[1]).atan2(tgt[0] - c[0]);
                if cw {
                    while a1 >= a0 - 1e-9 {
                        a1 -= std::f64::consts::TAU;
                    }
                } else {
                    while a1 <= a0 + 1e-9 {
                        a1 += std::f64::consts::TAU;
                    }
                }
                let n = (((a1 - a0).abs() * r / 0.5).ceil() as usize).clamp(2, 720);
                for i in 1..=n {
                    let t = i as f64 / n as f64;
                    let a = a0 + (a1 - a0) * t;
                    let p = if i == n { tgt } else { [c[0] + r * a.cos(), c[1] + r * a.sin(), s[2] + (tgt[2] - s[2]) * t] };
                    self.push(Block { target: p, feed: f, dwell: 0.0, jog: false });
                }
            } else {
                self.push(Block { target: tgt, feed: f, dwell: 0.0, jog });
            }
        }
    }
}

impl Link for SimLink {
    fn write_bytes(&mut self, b: &[u8]) -> bool {
        for &c in b {
            match c {
                b'?' => {
                    self.tick();
                    let jogging = self.queue.front().map(|b| b.jog).unwrap_or(false);
                    let state = if self.alarm {
                        "Alarm"
                    } else if self.hold {
                        "Hold:0"
                    } else if jogging {
                        "Jog"
                    } else if !self.queue.is_empty() {
                        "Run"
                    } else {
                        "Idle"
                    };
                    let m = self.mpos;
                    let s = format!(
                        "<{}|MPos:{:.3},{:.3},{:.3}|FS:{:.0},{:.0}|WCO:{:.3},{:.3},{:.3}>",
                        state, m[0], m[1], m[2], self.cur_feed, self.spindle, self.wco[0], self.wco[1], self.wco[2]
                    );
                    self.reply(&s);
                }
                b'!' => self.hold = true,
                b'~' => {
                    self.hold = false;
                    self.last_tick = Instant::now();
                }
                0x18 => {
                    self.queue.clear();
                    self.pending_ok = 0;
                    self.planned = self.mpos;
                    self.hold = false;
                    self.spindle = 0.0;
                    self.inbuf.clear();
                    self.reply("Grbl 1.1h ['$' for help] (CncCraft simulator)");
                }
                0x85 => {
                    // jog cancel: drop queued jog moves and stop where we are
                    self.queue.retain(|b| !b.jog);
                    self.planned = self.queue.back().map(|b| b.target).unwrap_or(self.mpos);
                }
                b'\n' => {
                    let line = std::mem::take(&mut self.inbuf);
                    self.exec(&line);
                }
                b'\r' => {}
                c => self.inbuf.push(c as char),
            }
        }
        true
    }
    fn read_some(&mut self, buf: &mut [u8]) -> usize {
        std::thread::sleep(Duration::from_millis(5));
        self.tick();
        let mut n = 0;
        while n < buf.len() {
            match self.out.pop_front() {
                Some(b) => {
                    buf[n] = b;
                    n += 1
                }
                None => break,
            }
        }
        n
    }
    fn set_speed(&mut self, s: f64) {
        self.speed = s.clamp(0.1, 1000.0);
    }
}

fn parse_status(s: &str) -> Option<(String, Option<[f64; 3]>, Option<[f64; 3]>, Option<[f64; 3]>, Option<(f64, f64)>)> {
    let s = s.trim().trim_start_matches('<').trim_end_matches('>');
    let mut parts = s.split('|');
    let state = parts.next()?.to_string();
    let (mut mpos, mut wpos, mut wco, mut fs) = (None, None, None, None);
    let v3 = |t: &str| -> Option<[f64; 3]> {
        let v: Vec<f64> = t.split(',').filter_map(|x| x.parse().ok()).collect();
        if v.len() >= 3 {
            Some([v[0], v[1], v[2]])
        } else {
            None
        }
    };
    for p in parts {
        if let Some(t) = p.strip_prefix("MPos:") {
            mpos = v3(t);
        } else if let Some(t) = p.strip_prefix("WPos:") {
            wpos = v3(t);
        } else if let Some(t) = p.strip_prefix("WCO:") {
            wco = v3(t);
        } else if let Some(t) = p.strip_prefix("FS:") {
            let v: Vec<f64> = t.split(',').filter_map(|x| x.parse().ok()).collect();
            if v.len() >= 2 {
                fs = Some((v[0], v[1]));
            }
        } else if let Some(t) = p.strip_prefix("F:") {
            fs = t.parse().ok().map(|f| (f, 0.0));
        }
    }
    Some((state, mpos, wpos, wco, fs))
}

fn worker(rx: Receiver<Cmd>, tx: Sender<Event>, ctx: egui::Context) {
    let mut link: Option<Box<dyn Link>> = None;
    let mut port_name = String::new();
    let mut buf = String::new();
    let mut last_poll = Instant::now();
    let mut job: VecDeque<String> = VecDeque::new();
    let mut job_total = 0usize;
    let mut job_sent = 0usize;
    let mut waiting_ok = 0usize;
    let mut queue: VecDeque<String> = VecDeque::new();
    let mut wco = [0.0; 3];
    let mut job_errors = false;
    let mut draining = false;
    let mut sim_speed = 5.0;
    let send_ev = |e: Event| {
        let _ = tx.send(e);
        ctx.request_repaint();
    };
    loop {
        // commands from the UI
        loop {
            let c = match rx.try_recv() {
                Ok(c) => c,
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(_) => return,
            };
            match c {
                Cmd::Connect(port, baud) => {
                    link = None;
                    if port == SIMULATOR {
                        let mut s = SimLink::new();
                        s.set_speed(sim_speed);
                        link = Some(Box::new(s));
                        port_name = port.clone();
                        send_ev(Event::Connected(port));
                    } else {
                        match serialport::new(&port, baud).timeout(Duration::from_millis(20)).open() {
                            Ok(mut p) => {
                                let _ = p.write_data_terminal_ready(true);
                                link = Some(Box::new(SerialLink(p)));
                                port_name = port.clone();
                                send_ev(Event::Connected(port));
                            }
                            Err(e) => send_ev(Event::Log(format!("Could not open {port}: {e}"), LogKind::Error)),
                        }
                    }
                    waiting_ok = 0;
                    job.clear();
                    queue.clear();
                    if let Some(l) = link.as_mut() {
                        l.write_bytes(b"\r\n\r\n");
                    }
                }
                Cmd::Disconnect => {
                    if link.take().is_some() {
                        send_ev(Event::Disconnected(port_name.clone()));
                    }
                    job.clear();
                }
                Cmd::Line(l) => queue.push_back(l),
                Cmd::Realtime(b) => {
                    if let Some(l) = link.as_mut() {
                        l.write_bytes(&[b]);
                    }
                    if b == 0x18 {
                        job.clear();
                        queue.clear();
                        waiting_ok = 0;
                        if job_total > 0 || draining {
                            job_total = 0;
                            draining = false;
                            send_ev(Event::JobDone(false));
                        }
                    }
                }
                Cmd::Job(lines) => {
                    job = lines.into_iter().map(|l| l.split(';').next().unwrap_or("").trim().to_string()).filter(|l| !l.is_empty()).collect();
                    job_total = job.len();
                    job_sent = 0;
                    job_errors = false;
                    draining = false;
                    send_ev(Event::Log(format!("Job started: {job_total} lines"), LogKind::Info));
                }
                Cmd::SimSpeed(s) => {
                    sim_speed = s;
                    if let Some(l) = link.as_mut() {
                        l.set_speed(s);
                    }
                }
                Cmd::StopJob => {
                    if !job.is_empty() || job_total > 0 || draining {
                        job.clear();
                        job_total = 0;
                        draining = false;
                        if let Some(l) = link.as_mut() {
                            l.write_bytes(&[b'!']);
                            std::thread::sleep(Duration::from_millis(100));
                            l.write_bytes(&[0x18]);
                        }
                        waiting_ok = 0;
                        send_ev(Event::JobDone(false));
                    }
                }
            }
        }
        let Some(l) = link.as_mut() else {
            std::thread::sleep(Duration::from_millis(30));
            continue;
        };
        // status poll
        if last_poll.elapsed() > Duration::from_millis(200) {
            if !l.write_bytes(b"?") {
                link = None;
                send_ev(Event::Disconnected(port_name.clone()));
                continue;
            }
            last_poll = Instant::now();
        }
        // send manual lines first, then job lines (one at a time, waiting for ok)
        if waiting_ok == 0 {
            if let Some(line) = queue.pop_front() {
                send_ev(Event::Log(line.clone(), LogKind::Sent));
                l.write_bytes(format!("{line}\n").as_bytes());
                waiting_ok += 1;
            } else if let Some(line) = job.pop_front() {
                l.write_bytes(format!("{line}\n").as_bytes());
                waiting_ok += 1;
                job_sent += 1;
                if job_sent % 10 == 0 || job.is_empty() {
                    send_ev(Event::Progress(job_sent, job_total));
                }
            }
        }
        // read replies
        let mut rb = [0u8; 512];
        let n = l.read_some(&mut rb);
        if n > 0 {
            buf.push_str(&String::from_utf8_lossy(&rb[..n]));
            while let Some(pos) = buf.find('\n') {
                let line: String = buf[..pos].trim().to_string();
                buf.drain(..=pos);
                if line.is_empty() {
                    continue;
                }
                if line.starts_with('<') {
                    if let Some((state, mpos, wpos, w, fs)) = parse_status(&line) {
                        if let Some(w) = w {
                            wco = w;
                        }
                        let (mpos, wpos) = match (mpos, wpos) {
                            (Some(m), _) => (m, [m[0] - wco[0], m[1] - wco[1], m[2] - wco[2]]),
                            (None, Some(wp)) => ([wp[0] + wco[0], wp[1] + wco[1], wp[2] + wco[2]], wp),
                            _ => ([0.0; 3], [0.0; 3]),
                        };
                        let (feed, spindle) = fs.unwrap_or((0.0, 0.0));
                        // the job is only finished once the machine has stopped moving
                        if draining && state.starts_with("Idle") {
                            draining = false;
                            job_total = 0;
                            send_ev(Event::Progress(job_sent, job_sent));
                            send_ev(Event::JobDone(!job_errors));
                        }
                        send_ev(Event::Status { state, mpos, wpos, feed, spindle });
                    }
                    continue;
                }
                if line == "ok" {
                    waiting_ok = waiting_ok.saturating_sub(1);
                    if job_total > 0 && job.is_empty() && waiting_ok == 0 && !draining {
                        // every line is in the controller; wait for it to finish moving
                        draining = true;
                        send_ev(Event::Log("All lines sent - waiting for the machine to finish".into(), LogKind::Info));
                    }
                    continue;
                }
                if line.starts_with("error") {
                    waiting_ok = waiting_ok.saturating_sub(1);
                    if job_total > 0 {
                        job_errors = true;
                    }
                    send_ev(Event::Log(format!("{line} {}", grbl_error(&line)), LogKind::Error));
                    continue;
                }
                if line.starts_with("ALARM") {
                    job.clear();
                    if job_total > 0 || draining {
                        draining = false;
                        job_total = 0;
                        send_ev(Event::JobDone(false));
                    }
                    send_ev(Event::Log(format!("{line} - the machine stopped. Home it or unlock to continue."), LogKind::Error));
                    continue;
                }
                if line.starts_with("Grbl") {
                    waiting_ok = 0;
                }
                send_ev(Event::Log(line, LogKind::Received));
            }
        }
    }
}

fn grbl_error(line: &str) -> &'static str {
    match line.trim_start_matches("error:").parse::<i32>().unwrap_or(0) {
        1 => "(G-code word missing a letter)",
        2 => "(bad number)",
        3 => "(unsupported $ command)",
        8 => "(not idle)",
        9 => "(locked by alarm - home or unlock first)",
        15 => "(jog target exceeds machine travel)",
        20 => "(unsupported G-code command)",
        22 => "(feed rate not set)",
        _ => "",
    }
}
