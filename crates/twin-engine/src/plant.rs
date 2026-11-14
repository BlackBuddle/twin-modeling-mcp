//! 설비 모델(plant): 출력 비트에서 장치의 위치를 움직이고 센서 비트를 돌려준다. 운동학만 있고 물리 엔진은 없다.
//! 시계, PLC, 파일에 닿지 않는다. 한 번의 `tick`은 `t`에서 `t + dt`까지 가며, 첫 tick 뒤에는 메모리를 할당하지 않는다
//! (이름과 색인은 `Plant::new`에서 한 번 풀어 둔다; 경보가 새로 생길 때만 할당한다).
//! 시뮬레이션 방식(`Mode::Sim`)은 센서를 쓴다. 추정 방식(`Mode::Estimate`, 실시간 화면용)은 센서를 읽기만 하고
//! 실제 센서가 바뀌면 축을 그 위치로 맞추며, 모델만 따라가는 유령(ghost) 사본을 함께 움직인다.

use super::bind::Addresses;
use super::scene::{Dev, Scene};
use serde_json::{json, Value};

// 입출력 트레이트는 `bind`에 있다(호스트가 자기 상태에 구현한다). 옛 경로 `plant::PlantIo`도 그대로 쓴다.
pub use super::bind::PlantIo;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Sim,
    Estimate,
}

// 밸브 규칙(`Valve`, `drive`)은 `mech`에 한 벌만 있다. 5/3 배기형의 자유 이동은 모델하지 않는다(shortcut).
use super::mech::Valve;

#[derive(Clone, Copy, Debug)]
struct Motion {
    pos: f64,
    /// 스풀 위치: 1 앞, -1 뒤, 0 가운데(섬)
    spool: i8,
    lag_left: f64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Shape {
    Linear,
    Rotary,
    /// 모터: 각도가 계속 돈다
    Spin,
    Belt,
}

struct Sens {
    name: String,
    out: bool,
    acc: f64,
    on_ms: f64,
    off_ms: f64,
    stuck: Option<bool>,
}

struct Axis {
    id: String,
    /// 장면의 장치 번호(공작물 실행이 축 값을 장치 번호로 받는다)
    dev: usize,
    shape: Shape,
    range: f64,
    fwd_ms: f64,
    back_ms: f64,
    lag: f64,
    window: f64,
    /// 연속 운동의 속도(단위/ms)
    rate: f64,
    valve: Valve,
    cf: Option<String>,
    cb: Option<String>,
    sens: [Option<Sens>; 2],
    m: Motion,
    g: Motion,
    /// 두 코일 경보가 걸려 있다(두 코일이 꺼진 채 `NEVER_MS`가 지나야 풀린다)
    both: bool,
    both_off: f64,
    factor: f64,
    air_off: bool,
    dead_f: bool,
    dead_b: bool,
    arrived: [f64; 2],
    /// 놓인 자리(x, z)와 방향(cos, sin of 회전 y): 작업물 규칙이 쓴다
    at: [f64; 2],
    cs: [f64; 2],
    /// 컨베이어 틀의 반 길이, 반 너비(벨트가 아니면 None)
    frame: Option<[f64; 2]>,
    /// 막대로 부품을 미는 축(밸브가 있는 직선 축, 그리퍼 제외)
    push: bool,
    /// 밀기 축: 장치 원점에서 몸통 앞끝까지(몸통 길이의 반, mm). 화면은 막대 판을 이 앞끝에서 행정만큼 더 내서 그린다.
    tip0: f64,
    /// 이번 tick에 표면(벨트)이나 끝(막대)이 간 거리
    delta: f64,
}

/// 한 시뮬레이션이 가진 부품 수의 상한
pub const MAX_PARTS: usize = 50;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Carrier {
    None,
    Axis(usize),
    Hold(usize),
}

struct Part {
    id: u32,
    x: f64,
    y: f64,
    z: f64,
    size: f64,
    carrier: Carrier,
}

struct Source {
    id: String,
    make: Option<String>,
    prev: bool,
    at: [f64; 3],
    size: f64,
}

struct Zone {
    at: [f64; 2],
    r: f64,
    /// 센서 비트(싱크는 None)
    bit: Option<String>,
}

struct Holder {
    id: String,
    bit: Option<String>,
    at: [f64; 3],
    reach: f64,
    /// 잡고 있는 부품 id(0이면 없음)
    held: u32,
}

struct Light {
    id: String,
    coils: Vec<String>,
    on: bool,
}

struct Level {
    id: String,
    fill: Option<String>,
    drain: Option<String>,
    low: Option<String>,
    high: Option<String>,
    word: Option<String>,
    cap: f64,
    fill_rate: f64,
    drain_rate: f64,
    low_at: f64,
    high_at: f64,
    value: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Alarm {
    pub kind: &'static str,
    pub device: String,
    pub sensor: Option<String>,
    /// 기계 모델 엔진 경보의 덧붙임(`joint`, `program`, `signal`)
    pub extra: Option<(&'static str, Value)>,
}

impl Alarm {
    pub fn to_json(&self) -> Value {
        let mut o = match &self.sensor {
            Some(s) => json!({"kind":self.kind,"device":self.device,"sensor":s}),
            None => json!({"kind":self.kind,"device":self.device}),
        };
        if let Some((k, v)) = &self.extra {
            o[*k] = v.clone();
        }
        o
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FaultKind {
    SensorStuckOn,
    SensorStuckOff,
    CoilDead,
    Slow,
    AirOff,
}

/// 시뮬레이션 고장. 장면 파일에 저장하지 않는다.
#[derive(Clone, Debug)]
pub struct Fault {
    pub device: String,
    pub kind: FaultKind,
    /// 센서나 코일 주소(정규화). 없으면 그 장치의 모두.
    pub addr: Option<String>,
    pub factor: f64,
}

const EPS: f64 = 1e-9;
/// 끝에 닿았는데 센서가 이 시간(ms) 안에 안 켜지면 경보
const NEVER_MS: f64 = 500.0;
/// 플랜트 하나가 담는 경보 수의 상한. 넘으면 버리고 세기만 한다.
pub const MAX_ALARMS: usize = 100;

fn push_alarm(v: &mut Vec<Alarm>, dropped: &mut u64, a: Alarm) {
    if v.len() < MAX_ALARMS {
        v.push(a);
    } else {
        *dropped += 1;
    }
}

pub struct Plant {
    mode: Mode,
    axes: Vec<Axis>,
    lights: Vec<Light>,
    levels: Vec<Level>,
    /// 시작할 때 쓰는 스위치 접점의 처음 값(NC는 켜짐)
    switches: Vec<(String, bool)>,
    alarms: Vec<Alarm>,
    /// 상한(`MAX_ALARMS`) 때문에 버린 경보 수
    dropped: u64,
    sources: Vec<Source>,
    sinks: Vec<Zone>,
    zones: Vec<Zone>,
    holders: Vec<Holder>,
    parts: Vec<Part>,
    next_part: u32,
    /// 상한(`MAX_PARTS`) 때문에 만들지 못한 부품 수
    parts_dropped: u64,
    /// 형식 2 장면의 공작물 규칙(시뮬레이션 방식에서만; 장면에 공작물 키가 있을 때)
    work: Option<super::work::Rt>,
    /// `mech` 키가 있는 장치의 엔진(시뮬레이션 방식에서만, id 순서)
    mechs: Vec<MechRt>,
    /// 주소 없는 신호 때문에 돌리지 않은 `mech` 장치의 안내 글(`twin_sim_run`의 오류와 같은 글; 상태의 `notes`에 나온다)
    skipped: Vec<String>,
    /// 장면을 읽은 호스트의 주소 규칙(32비트 읽기)
    addrs: &'static dyn Addresses,
}

/// 엔진 핀 하나가 가리키는 주소(`Plant`만 주소를 안다)
struct BoundPin {
    addr: String,
    low: bool,
    bcd: bool,
}

struct MechRt {
    dev: usize,
    id: String,
    m: super::mech::Machine,
    pins: Vec<BoundPin>,
    /// 짧은 형태 `position`이 만든 엔진(결과의 `joints`에는 나오지 않는다)
    short: bool,
}

/// 엔진의 `Pins`를 `PlantIo` 위에 얹는다: 낮은 활성(비트 뒤집기), BCD 풀기, 32비트(다음 단어)
struct Bind<'a> {
    io: &'a mut dyn PlantIo,
    pins: &'a [BoundPin],
    addrs: &'static dyn Addresses,
}

impl super::mech::Pins for Bind<'_> {
    fn bit(&self, pin: usize) -> bool {
        let b = &self.pins[pin];
        self.io.bit(&b.addr) != b.low
    }
    fn word(&self, pin: usize, width: u8) -> Option<i64> {
        let b = &self.pins[pin];
        if width == 32 {
            // 아래 단어가 먼저, 부호 있는 32비트(구 대리 축과 같은 식): 다음 단어를 잇는 규칙은 호스트의 것이다
            return self.addrs.read32(&*self.io, &b.addr);
        }
        let raw = self.io.word(&b.addr);
        if !b.bcd {
            return Some(raw);
        }
        // BCD 4자리: 9보다 큰 자리가 있으면 수가 아니다
        let w = raw & 0xFFFF;
        (0..4).try_fold(0i64, |acc, k| {
            let d = (w >> (4 * (3 - k))) & 0xF;
            (d <= 9).then_some(acc * 10 + d)
        })
    }
    fn set_bit(&mut self, pin: usize, v: bool) {
        let b = &self.pins[pin];
        self.io.set_bit(&b.addr, v != b.low);
    }
}

/// 종류의 첫 부품(틀) 크기에서 (x, z)의 반을 읽는다. 형식 2 장치는 풀어낸 크기(벨트의 길이와 너비)를 쓴다(다리 값만 바뀐다).
fn frame_half(d: &Dev) -> Option<[f64; 2]> {
    if let Some(g) = &d.geom {
        return g.belt_half();
    }
    let s = super::scene::catalog().pointer(&format!("/layout/{}/parts/0/size", d.kind))?.as_array()?;
    Some([s.first()?.as_f64()? / 2.0, s.get(2)?.as_f64()? / 2.0])
}

/// 막대 끝이 행정 0에서 `pos`에서 +x로 떨어진 거리: 형식 1은 틀 길이의 반, 형식 2는 `out` 틀의 위치(몸 길이의 반 + 막대가 나온 길이)
fn tip0_of(d: &Dev) -> f64 {
    match &d.geom {
        Some(g) => g.rod_tip0(&d.kind),
        None => frame_half(d).map_or(0.0, |h| h[0]),
    }
}

fn sens_of(d: &Dev, slot: &str) -> Option<Sens> {
    d.slot(slot).map(|a| Sens { name: a.to_string(), out: false, acc: 0.0, on_ms: d.num("on_delay_ms"), off_ms: d.num("off_delay_ms"), stuck: None })
}

/// 코일 상태에서 스풀을 정한다. 코일이 둘 다 켜지면 스풀을 그대로 두고 true를 돌려준다.
fn drive(m: &mut Motion, v: Valve, cf: bool, cb: bool, lag: f64) -> bool {
    super::mech::drive(&mut m.spool, &mut m.lag_left, v, cf, cb, lag)
}

/// 지연을 쓰고 일정 속도로 움직인다(가속 없음).
fn advance(m: &mut Motion, a: &Axis, dt: f64) {
    if a.air_off || m.spool == 0 {
        return;
    }
    let mut dt = dt;
    let used = m.lag_left.min(dt);
    m.lag_left -= used;
    dt -= used;
    if dt <= 0.0 {
        return;
    }
    let s = m.spool as f64;
    match a.shape {
        Shape::Spin => m.pos = (m.pos + s * a.rate * dt).rem_euclid(360.0),
        Shape::Belt => m.pos = (m.pos + s * a.rate * dt).rem_euclid(1e9),
        _ => {
            let ms = if m.spool > 0 { a.fwd_ms } else { a.back_ms } * a.factor;
            m.pos = (m.pos + s * a.range / ms * dt).clamp(0.0, a.range);
        }
    }
}

fn in_window(pos: f64, at: f64, win: f64) -> bool {
    (pos - at).abs() <= win + EPS
}

/// 구동기 하나의 지금 상태(쪽의 카드 줄). 글이 아니라 부호만 준다: 한국어 글은 쪽이 만든다
fn driver_state(d: &super::mech::Driver) -> Value {
    use super::mech::Driver;
    match d {
        Driver::Coils { spool, .. } => json!({"type": "coils", "spool": state_name(*spool)}),
        Driver::Follow { .. } => json!({"type": "follow"}),
        Driver::Step { moving, .. } => json!({"type": "step", "moving": moving}),
        Driver::Playback { running, cur, programs, .. } => json!({"type": "playback", "running": running, "program": if *running { programs.get(*cur).map(|p| p.no) } else { None }}),
    }
}

fn state_name(spool: i8) -> &'static str {
    match spool {
        1 => "fwd",
        -1 => "back",
        _ => "none",
    }
}

impl Plant {
    pub fn new(scene: &Scene, mode: Mode) -> Plant {
        let (mut axes, mut lights, mut levels, mut switches) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let (mut sources, mut sinks, mut zones, mut holders) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for (di, d) in scene.devices.iter().enumerate().filter(|(_, d)| d.simulated) {
            let slot =|k: &str| d.slot(k).map(String::from);
            if d.has("linear") || d.has("rotary") || d.has("belt") {
                let shape = if d.has("belt") {
                    Shape::Belt
                } else if d.has("valve") {
                    if d.has("rotary") {
                        Shape::Rotary
                    } else {
                        Shape::Linear
                    }
                } else {
                    Shape::Spin
                };
                let (range, rate) = match shape {
                    Shape::Linear => (d.num("stroke_mm"), 0.0),
                    Shape::Rotary => (d.num("angle_deg"), 0.0),
                    Shape::Spin => (360.0, d.num("speed_deg_s") / 1000.0),
                    Shape::Belt => (0.0, d.num("speed_mm_s") / 1000.0),
                };
                let valve = match d.text("valve") {
                    "5/2-double" | "" if d.has("valve") => Valve::Hold,
                    "5/2-single" | "3/2-single" => Valve::Spring,
                    "5/3-closed" | "5/3-exhaust" => Valve::Center,
                    _ => Valve::Center,
                };
                let lag = if d.has("valve") { d.timing.lag_ms } else { 0.0 };
                let (cf, cb) = if d.has("valve") { (slot("coil_fwd"), slot("coil_back")) } else { (slot("run_fwd"), slot("run_back")) };
                // 처음 스풀: 한 방향 밸브와 양쪽 밸브 모두 뒤쪽(센서 back 위치)에서 시작한다.
                let m = Motion { pos: 0.0, spool: if d.has("valve") { -1 } else { 0 }, lag_left: 0.0 };
                axes.push(Axis {
                    id: d.id.clone(),
                    dev: di,
                    shape,
                    range,
                    fwd_ms: d.timing.fwd_ms,
                    back_ms: d.timing.back_ms,
                    lag,
                    window: d.num("window_mm"),
                    rate,
                    valve,
                    cf,
                    cb,
                    sens: [sens_of(d, "sensor_back"), sens_of(d, "sensor_fwd")],
                    m,
                    g: m,
                    both: false,
                    both_off: 0.0,
                    factor: 1.0,
                    air_off: false,
                    dead_f: false,
                    dead_b: false,
                    arrived: [0.0; 2],
                    at: [d.pos[0], d.pos[2]],
                    cs: [d.rot_y.to_radians().cos(), d.rot_y.to_radians().sin()],
                    frame: if shape == Shape::Belt { frame_half(d) } else { None },
                    push: shape == Shape::Linear && d.has("valve") && !d.has("hold"),
                    tip0: tip0_of(d),
                    delta: 0.0,
                });
            } else if d.has("light") {
                lights.push(Light { id: d.id.clone(), coils: d.slots.iter().filter(|s| s.out).map(|s| s.addr.clone()).collect(), on: false });
            } else if d.has("level") {
                levels.push(Level {
                    id: d.id.clone(),
                    fill: slot("fill"),
                    drain: slot("drain"),
                    low: slot("sensor_low"),
                    high: slot("sensor_high"),
                    word: slot("value_word"),
                    cap: d.num("capacity"),
                    fill_rate: d.num("fill_rate") / 1000.0,
                    drain_rate: d.num("drain_rate") / 1000.0,
                    low_at: d.num("low_at"),
                    high_at: d.num("high_at"),
                    value: d.num("initial").min(d.num("capacity")),
                });
            } else if d.has("switch") {
                let nc = d.text("contact") == "nc";
                switches.extend(d.slots.iter().filter(|s| s.name.starts_with("contact")).map(|s| (s.addr.clone(), nc)));
            }
            // 작업물 규칙(F28): 종류의 동작으로 고른다
            let at = [d.pos[0], d.pos[1], d.pos[2]];
            if d.has("source") {
                sources.push(Source { id: d.id.clone(), make: slot("make"), prev: false, at, size: d.num("part_mm") });
            }
            if d.has("sink") {
                sinks.push(Zone { at: [at[0], at[2]], r: d.num("zone_mm"), bit: None });
            }
            if d.has("zone") {
                zones.push(Zone { at: [at[0], at[2]], r: d.num("zone_mm"), bit: slot("output") });
            }
            if d.has("hold") {
                holders.push(Holder { id: d.id.clone(), bit: slot("coil_fwd").or_else(|| slot("coil")), at, reach: d.num("reach_mm"), held: 0 });
            }
        }
        // 부품이 나올 수 있을 때만 자리를 미리 잡는다(tick은 그 뒤 할당하지 않는다)
        let parts = if sources.is_empty() { Vec::new() } else { Vec::with_capacity(MAX_PARTS) };
        let work = if mode == Mode::Sim { super::work::Rt::new(scene) } else { None };
        // 엔진: 시뮬레이션에서만(추정 방식은 관절 값을 읽지 못한다), 주소 없는 신호를 쓰는 장치는 돌리지 않는다(`twin_sim_run`이 거부한다)
        let mut mechs: Vec<MechRt> = Vec::new();
        let mut skipped = Vec::new();
        if mode == Mode::Sim {
            for (di, d) in scene.devices.iter().enumerate() {
                if let Some(u) = d.mech.as_ref().and_then(|m| m.unbound.as_ref()) {
                    skipped.push(super::mech::unbound_text(&d.id, u));
                    continue;
                }
                let Some(m) = d.mech.as_ref() else { continue };
                let pins = d.slots.iter().chain(&d.signals).map(|s| BoundPin { addr: s.addr.clone(), low: s.low, bcd: s.bcd }).collect();
                mechs.push(MechRt { dev: di, id: d.id.clone(), m: super::mech::Machine::new(m), pins, short: false });
            }
            // 짧은 형태 `position`: 형태 관절과 `follow` 구동기 하나(엔진이 움직인다; 옛 `Carriage`의 자리)
            for p in scene.work.iter().flat_map(|w| &w.positions) {
                let d = &scene.devices[p.dev];
                let Some(range) = d.geom.as_ref().and_then(|g| g.axis.as_ref()).map(|a| a.range) else { continue };
                let sig = |nm: &str| d.signals.iter().find(|s| s.name == nm && s.bound).map(|s| s.addr.clone());
                // 32비트인데 다음 단어 이름이 없으면 16비트로 읽는다(옛 규칙)
                let width = if p.width == 32 && scene.addrs.can_read32(&p.from) { 32 } else { 16 };
                let (en, bu) = (sig("servo_on"), sig("busy"));
                let m = super::mech::short_position(range, width, p.per_mm, p.speed_mm_s, en.is_some(), bu.is_some());
                let pins = std::iter::once(p.from.clone()).chain(en).chain(bu).map(|addr| BoundPin { addr, low: false, bcd: false }).collect();
                mechs.push(MechRt { dev: p.dev, id: d.id.clone(), m: super::mech::Machine::new(&m), pins, short: true });
            }
            mechs.sort_by(|a, b| a.id.cmp(&b.id));
        }
        Plant { mode, axes, lights, levels, switches, alarms: Vec::new(), dropped: 0, sources, sinks, zones, holders, parts, next_part: 1, parts_dropped: 0, work, mechs, skipped, addrs: scene.addrs }
    }

    /// 엔진이 있는가(결과의 `joints` 키는 있을 때만 나온다)
    pub fn has_mech(&self) -> bool {
        self.mechs.iter().any(|m| !m.short)
    }

    /// 관절 값 `{"<장치>": {"<관절>": 값}}`과 보고하는 값의 개수
    pub fn joints_json(&self) -> (Value, usize) {
        let mut o = serde_json::Map::new();
        let mut n = 0;
        for mr in self.mechs.iter().filter(|m| !m.short) {
            n += mr.m.joint_count();
            o.insert(mr.id.clone(), Value::Object(mr.m.joints_json()));
        }
        (Value::Object(o), n)
    }

    /// 공작물 규칙(형식 2 장면에 공작물 키가 있고 시뮬레이션 방식일 때)
    pub fn work(&self) -> Option<&super::work::Rt> {
        self.work.as_ref()
    }

    pub fn work_mut(&mut self) -> Option<&mut super::work::Rt> {
        self.work.as_mut()
    }
    fn make_part(&mut self, k: usize) {
        if self.parts.len() >= MAX_PARTS {
            self.parts_dropped += 1;
            return;
        }
        let s = &self.sources[k];
        self.parts.push(Part { id: self.next_part, x: s.at[0], y: s.at[1], z: s.at[2], size: s.size, carrier: Carrier::None });
        self.next_part += 1;
    }

    /// 설비 모델이 읽거나 쓰는 센서와 입력 비트 이름(시뮬레이션 상태에 프로그램이 안 쓰는 센서 값도 싣는다).
    pub fn sensor_bits(&self) -> Vec<&str> {
        let ax = self.axes.iter().flat_map(|a| a.sens.iter().flatten().map(|s| s.name.as_str()));
        let sense = self.work.iter().flat_map(|r| r.sense_bits());
        ax.chain(self.zones.iter().filter_map(|z| z.bit.as_deref())).chain(self.sources.iter().filter_map(|s| s.make.as_deref())).chain(self.switches.iter().map(|(a, _)| a.as_str())).chain(sense).collect()
    }

    /// 화면이 시뮬레이션에서 손으로 바꿀 수 있는 장면의 입력 비트인가(원천의 `make`, 스위치 접점, 부품 감지 규칙이 쓰는 비트).
    pub fn takes_input(&self, name: &str) -> bool {
        self.sources.iter().any(|s| s.make.as_deref() == Some(name)) || self.switches.iter().any(|(a, _)| a == name) || self.work.as_ref().is_some_and(|r| r.sense_bit(name))
    }

    pub fn has_source(&self, id: &str) -> bool {
        self.sources.iter().any(|s| s.id == id)
    }

    /// 쪽모이 사건 하나: 그 원천이 부품 하나를 만든다(화면 클릭과 `part_events`가 쓴다).
    pub fn spawn(&mut self, source: &str) -> Result<(), String> {
        let k = self.sources.iter().position(|s| s.id == source).ok_or_else(|| format!("source {source} is not a part source of the scene"))?;
        self.make_part(k);
        Ok(())
    }

    /// 부품 규칙 한 걸음(시뮬레이션 방식에서만). 축이 이미 움직인 뒤에 부른다.
    fn tick_parts(&mut self, io: &mut dyn PlantIo) {
        for k in 0..self.sources.len() {
            let on = self.sources[k].make.as_deref().is_some_and(|n| io.bit(n));
            if on && !self.sources[k].prev {
                self.make_part(k);
            }
            self.sources[k].prev = on;
        }
        // 잡힌 부품이 아니면 운반자를 지운다
        for p in self.parts.iter_mut().filter(|p| !matches!(p.carrier, Carrier::Hold(_))) {
            p.carrier = Carrier::None;
        }
        for (i, a) in self.axes.iter().enumerate() {
            let (c, s) = (a.cs[0], a.cs[1]);
            for p in self.parts.iter_mut().filter(|p| !matches!(p.carrier, Carrier::Hold(_))) {
                let (dx, dz) = (p.x - a.at[0], p.z - a.at[1]);
                let (ps, lat) = (dx * c - dz * s, dx * s + dz * c);
                let step = if let Some([hl, hw]) = a.frame {
                    if ps.abs() > hl || lat.abs() > hw {
                        continue;
                    }
                    p.carrier = Carrier::Axis(i);
                    a.delta
                } else if a.push && a.delta > 0.0 {
                    let (h, tip_new) = (p.size / 2.0, a.tip0 + a.m.pos);
                    let (back, tip_old) = (ps - h, tip_new - a.delta);
                    // 접촉 구역: 끝에서 뒤로 2 mm ~ 앞으로 10 mm
                    if lat.abs() > h || back < tip_old - 2.0 || back > tip_old + 10.0 || back > tip_new {
                        continue;
                    }
                    p.carrier = Carrier::Axis(i);
                    tip_new + h - ps
                } else {
                    continue;
                };
                p.x += step * c;
                p.z -= step * s;
            }
        }
        for k in 0..self.holders.len() {
            let on = self.holders[k].bit.as_deref().is_some_and(|n| io.bit(n));
            let h = &mut self.holders[k];
            if !on {
                if h.held != 0 {
                    if let Some(p) = self.parts.iter_mut().find(|p| p.id == h.held) {
                        p.carrier = Carrier::None;
                    }
                    h.held = 0;
                }
                continue;
            }
            if h.held == 0 {
                let r = h.reach;
                let near = self.parts.iter_mut().filter(|p| !matches!(p.carrier, Carrier::Hold(_)) && (p.x - h.at[0]).abs() <= r && (p.z - h.at[2]).abs() <= r);
                let best = near.min_by(|a, b| ((a.x - h.at[0]).powi(2) + (a.z - h.at[2]).powi(2)).total_cmp(&((b.x - h.at[0]).powi(2) + (b.z - h.at[2]).powi(2))));
                if let Some(p) = best {
                    p.carrier = Carrier::Hold(k);
                    h.held = p.id;
                }
            }
            if let Some(p) = self.parts.iter_mut().find(|p| p.id == h.held) {
                (p.x, p.y, p.z) = (h.at[0], h.at[1], h.at[2]);
            }
        }
        let sinks = &self.sinks;
        self.parts.retain(|p| matches!(p.carrier, Carrier::Hold(_)) || !sinks.iter().any(|z| (p.x - z.at[0]).abs() <= z.r && (p.z - z.at[1]).abs() <= z.r));
        for z in &self.zones {
            if let Some(n) = &z.bit {
                let on = self.parts.iter().any(|p| (p.x - z.at[0]).abs() <= z.r && (p.z - z.at[1]).abs() <= z.r);
                io.set_bit(n, on);
            }
        }
    }

    /// 시작할 때 한 번: 센서와 스위치 접점의 처음 값을 입력에 쓴다(첫 스캔이 읽는 값). 추정 방식에서는 아무것도 쓰지 않는다.
    pub fn init(&mut self, io: &mut dyn PlantIo) {
        if self.mode != Mode::Sim {
            return;
        }
        for (a, v) in &self.switches {
            io.set_bit(a, *v);
        }
        for a in &mut self.axes {
            for i in 0..2 {
                let at = if i == 0 { 0.0 } else { a.range };
                if let Some(s) = &mut a.sens[i] {
                    s.out = in_window(a.m.pos, at, a.window);
                    io.set_bit(&s.name, s.stuck.unwrap_or(s.out));
                }
            }
        }
        for l in &mut self.levels {
            Self::level_write(l, io);
        }
        for n in self.zones.iter().filter_map(|z| z.bit.as_deref()) {
            io.set_bit(n, false);
        }
        // 엔진: 센서의 처음 값과 꺼진 busy/done을 쓴다. 형태 관절은 공작물 규칙이 보는 축 값이기도 하다.
        for mr in &mut self.mechs {
            mr.m.init(&mut Bind { io: &mut *io, pins: &mr.pins, addrs: self.addrs });
            if let (Some(r), Some(v)) = (&mut self.work, mr.m.form_value()) {
                r.vals[mr.dev] = v;
            }
            if let (Some(r), false) = (&mut self.work, mr.short) {
                r.set_joints(mr.dev, &mr.m);
            }
        }
        // 공작물: 시작 내용과 처음 떨어짐을 만들고 감지 비트를 쓴다(첫 스캔이 읽는 값)
        if let Some(r) = &mut self.work {
            r.begin(io);
        }
    }

    fn level_write(l: &Level, io: &mut dyn PlantIo) {
        if let Some(n) = &l.low {
            io.set_bit(n, l.value >= l.low_at);
        }
        if let Some(n) = &l.high {
            io.set_bit(n, l.value >= l.high_at);
        }
        if let Some(n) = &l.word {
            io.set_word(n, l.value.round() as i64);
        }
    }

    pub fn apply_faults(&mut self, faults: &[Fault]) -> Result<(), String> {
        for f in faults {
            // 부품 감지 규칙을 가진 센서: 비트를 끝까지 켜 두거나 꺼 둔다
            if !self.axes.iter().any(|a| a.id == f.device) {
                let stuck = matches!(f.kind, FaultKind::SensorStuckOn | FaultKind::SensorStuckOff);
                match self.work.as_mut() {
                    Some(r) if r.has_sense(&f.device) => {
                        if !stuck {
                            return Err(format!("fault on sensor {} must be sensor_stuck_on or sensor_stuck_off", f.device));
                        }
                        r.stick(&f.device, f.kind == FaultKind::SensorStuckOn);
                        continue;
                    }
                    _ => return Err(format!("fault device {} is not a moving device or a sensor with sense of the scene", f.device)),
                }
            }
            let a = self.axes.iter_mut().find(|a| a.id == f.device).ok_or_else(|| format!("fault device {} is not a moving device of the scene", f.device))?;
            match f.kind {
                FaultKind::Slow => a.factor = f.factor,
                FaultKind::AirOff => a.air_off = true,
                FaultKind::CoilDead => {
                    let hit = |n: &Option<String>| n.is_some() && (f.addr.is_none() || *n == f.addr);
                    (a.dead_f, a.dead_b) = (a.dead_f || hit(&a.cf), a.dead_b || hit(&a.cb));
                    if !(a.dead_f || a.dead_b) {
                        return Err(format!("fault coil of {} is not a coil of that device", f.device));
                    }
                }
                FaultKind::SensorStuckOn | FaultKind::SensorStuckOff => {
                    let mut hit = false;
                    for s in a.sens.iter_mut().flatten().filter(|s| f.addr.is_none() || f.addr.as_deref() == Some(s.name.as_str())) {
                        s.stuck = Some(f.kind == FaultKind::SensorStuckOn);
                        hit = true;
                    }
                    if !hit {
                        return Err(format!("fault sensor of {} is not a sensor of that device", f.device));
                    }
                }
            }
        }
        Ok(())
    }

    /// `t`에서 `t + dt`(ms)까지 한 걸음. 입력은 출력 비트, 쓰는 것은 센서 비트(시뮬레이션)다.
    pub fn tick(&mut self, io: &mut dyn PlantIo, dt: f64) {
        let mode = self.mode;
        for a in &mut self.axes {
            let cf = !a.dead_f && a.cf.as_deref().is_some_and(|n| io.bit(n));
            let cb = !a.dead_b && a.cb.as_deref().is_some_and(|n| io.bit(n));
            let (v, lag) = (a.valve, a.lag);
            let mut m = a.m;
            let both = drive(&mut m, v, cf, cb, lag);
            advance(&mut m, a, dt);
            // 장치마다 경보 하나: 두 코일이 모두 꺼졌다가(NEVER_MS) 다시 켜질 때만 새로 낸다
            if both {
                if !a.both {
                    push_alarm(&mut self.alarms, &mut self.dropped, Alarm { kind: "both_coils", device: a.id.clone(), sensor: None, extra: None });
                    a.both = true;
                }
                a.both_off = 0.0;
            } else if !cf && !cb {
                a.both_off += dt;
                if a.both_off >= NEVER_MS {
                    a.both = false;
                }
            } else {
                a.both_off = 0.0;
            }
            if mode == Mode::Estimate {
                let mut g = a.g;
                drive(&mut g, v, cf, cb, lag);
                advance(&mut g, a, dt);
                a.g = g;
                // 실제 센서가 말하는 위치로 맞춘다: 켜져 있으면 그 자리, 꺼져 있으면 창 밖 가장자리에서 기다린다.
                for i in 0..2 {
                    let at = if i == 0 { 0.0 } else { a.range };
                    if let Some(s) = &a.sens[i] {
                        let real = io.bit(&s.name);
                        let inside = in_window(m.pos, at, a.window);
                        if real && !inside {
                            m.pos = at;
                        } else if !real && inside {
                            // 창이 행정보다 크면 끝 밖으로 나가지 않게 0 ..= range로 자른다
                            m.pos = (if i == 0 { at + a.window } else { at - a.window }).clamp(0.0, a.range);
                        }
                    }
                }
                a.m = m;
                continue;
            }
            a.delta = match a.shape {
                Shape::Belt if !a.air_off => m.spool as f64 * a.rate * dt,
                Shape::Linear => m.pos - a.m.pos,
                _ => 0.0,
            };
            a.m = m;
            if a.shape != Shape::Linear && a.shape != Shape::Rotary {
                continue;
            }
            for i in 0..2 {
                let at = if i == 0 { 0.0 } else { a.range };
                let raw = in_window(m.pos, at, a.window);
                if let Some(s) = &mut a.sens[i] {
                    if raw != s.out {
                        s.acc += dt;
                        if s.acc >= if raw { s.on_ms } else { s.off_ms } {
                            (s.out, s.acc) = (raw, 0.0);
                        }
                    } else {
                        s.acc = 0.0;
                    }
                    io.set_bit(&s.name, s.stuck.unwrap_or(s.out));
                    // 끝에 닿았는데 센서가 오지 않는다
                    let at_end = if i == 0 { m.pos <= 0.0 } else { m.pos >= a.range };
                    if at_end && !s.stuck.unwrap_or(s.out) {
                        a.arrived[i] += dt;
                        if a.arrived[i] >= NEVER_MS && !self.alarms.iter().any(|x| x.kind == "sensor_never" && x.device == a.id && x.sensor.as_deref() == Some(s.name.as_str())) {
                            push_alarm(&mut self.alarms, &mut self.dropped, Alarm { kind: "sensor_never", device: a.id.clone(), sensor: Some(s.name.clone()), extra: None });
                        }
                    } else {
                        a.arrived[i] = 0.0;
                    }
                }
            }
        }
        // 엔진(설계 2.8의 2): 오래된 축 다음, 공작물 규칙 앞. 장치는 id 순서이다.
        let mut form_deltas: Vec<(usize, f64, f64)> = Vec::new();
        if mode == Mode::Sim {
            let (alarms, dropped) = (&mut self.alarms, &mut self.dropped);
            for mr in &mut self.mechs {
                let before = mr.m.form_value();
                let id = &mr.id;
                mr.m.tick(&mut Bind { io: &mut *io, pins: &mr.pins, addrs: self.addrs }, dt, &mut |a| {
                    let extra = a.key.clone();
                    push_alarm(alarms, dropped, Alarm { kind: a.kind, device: id.clone(), sensor: None, extra });
                });
                if let (Some(b), Some(v)) = (before, mr.m.form_value()) {
                    form_deltas.push((mr.dev, v, v - b));
                }
            }
        }
        if mode == Mode::Sim {
            if let Some(r) = &mut self.work {
                // 축 값과 이번 tick의 변화를 장치 번호로 넘긴다(대리 축은 규칙이 직접 움직인다)
                for a in &self.axes {
                    r.vals[a.dev] = a.m.pos;
                    r.deltas[a.dev] = a.delta;
                }
                for (dev, v, d) in form_deltas {
                    r.vals[dev] = v;
                    r.deltas[dev] = d;
                }
                // 관절 틀의 지금 자리: 관절 틀 위의 점, 패드, 부품을 규칙이 따라가게 한다(작업 2.2, Q10)
                for mr in self.mechs.iter().filter(|m| !m.short) {
                    r.set_joints(mr.dev, &mr.m);
                }
                r.tick(io, dt);
            }
            self.tick_parts(io);
        }
        for l in &mut self.lights {
            l.on = l.coils.iter().any(|c| io.bit(c));
        }
        for l in &mut self.levels {
            let up = l.fill.as_deref().is_some_and(|n| io.bit(n)) as i32 as f64 * l.fill_rate;
            let down = l.drain.as_deref().is_some_and(|n| io.bit(n)) as i32 as f64 * l.drain_rate;
            l.value = (l.value + (up - down) * dt).clamp(0.0, l.cap);
            if mode == Mode::Sim {
                Self::level_write(l, io);
            }
        }
    }

    pub fn alarms(&self) -> &[Alarm] {
        &self.alarms
    }

    /// 상한 때문에 버린 경보 수
    pub fn alarms_dropped(&self) -> u64 {
        self.dropped
    }

    /// 움직이는 축의 (id, 위치). 선형은 mm, 회전은 도.
    pub fn axes(&self) -> impl Iterator<Item = (&str, f64)> {
        self.axes.iter().map(|a| (a.id.as_str(), a.m.pos)).chain(self.mechs.iter().filter_map(|m| m.m.form_value().map(|v| (m.id.as_str(), v))))
    }

    /// 모델만 따라가는 유령 위치(추정 방식에서만 의미가 있다). 작업 2.2a(F24 유령)가 쓸 때까지 서버 본체는 부르지 않는다.
    #[allow(dead_code)]
    pub fn ghost_axes(&self) -> impl Iterator<Item = (&str, f64)> {
        self.axes.iter().map(|a| (a.id.as_str(), a.g.pos))
    }

    /// 화면과 `sim/state`가 쓰는 상태. 형식은 `docs/twin-api.md`.
    pub fn state_json(&self) -> Value {
        let r = |x: f64| (x * 1000.0).round() / 1000.0;
        let axes: serde_json::Map<String, Value> = self
            .axes
            .iter()
            .map(|a| {
                let (ty, unit) = match a.shape {
                    Shape::Linear => ("linear", "mm"),
                    Shape::Rotary => ("rotary", "deg"),
                    Shape::Spin => ("motor", "deg"),
                    Shape::Belt => ("belt", "mm"),
                };
                let cont = matches!(a.shape, Shape::Spin | Shape::Belt);
                let m = &a.m;
                let moving = !a.air_off && m.spool != 0 && m.lag_left <= 0.0 && (cont || (m.spool > 0 && m.pos < a.range) || (m.spool < 0 && m.pos > 0.0));
                let mut o = json!({"type":ty,"unit":unit,"pos":r(m.pos),"spool":state_name(m.spool),"moving":moving});
                if a.shape == Shape::Belt {
                    o["speed"] = json!(r(m.spool as f64 * a.rate * 1000.0));
                }
                if self.mode == Mode::Estimate {
                    o["ghost"] = json!(r(a.g.pos));
                }
                (a.id.clone(), o)
            })
            .collect();
        let lights: serde_json::Map<String, Value> = self.lights.iter().map(|l| (l.id.clone(), json!(l.on))).collect();
        let levels: serde_json::Map<String, Value> = self.levels.iter().map(|l| (l.id.clone(), json!(r(l.value)))).collect();
        let mut axes = axes;
        for mr in &self.mechs {
            if let Some(v) = mr.m.form_value() {
                axes.insert(mr.id.clone(), json!({"type":"linear","unit":"mm","pos":r(v),"spool":"none","moving":mr.m.moving()}));
            }
        }
        let parts: Vec<Value> = match &self.work {
            Some(w) => w.parts_json(),
            None => self.parts.iter().map(|p| json!({"id":p.id,"x":r(p.x),"y":r(p.y),"z":r(p.z),"holder":self.holder_id(p.carrier)})).collect(),
        };
        let mut out = json!({"axes":axes,"lights":lights,"levels":levels,"parts":parts,"parts_dropped":self.parts_dropped(),"alarms":self.alarms.iter().map(Alarm::to_json).collect::<Vec<_>>(),"alarms_dropped":self.dropped});
        if let (true, Some(o)) = (self.has_mech(), out.as_object_mut()) {
            o.insert("joints".into(), self.joints_json().0);
            // 쪽 상태에만 있는 키(RR3, RR10): 장치마다 관절이 움직이는지와 구동기의 상태. `twin_sim_run` 결과는 이 함수를 쓰지 않는다
            let eng = || self.mechs.iter().filter(|m| !m.short);
            o.insert("joints_moving".into(), Value::Object(eng().map(|m| (m.id.clone(), json!(m.m.moving()))).collect()));
            o.insert("drivers".into(), Value::Object(eng().map(|m| (m.id.clone(), Value::Array(m.m.m.drivers.iter().map(driver_state).collect()))).collect()));
        }
        // 돌리지 않은 `mech` 장치는 조용히 빠지지 않는다: 까닭을 `notes`에 적는다
        if let (false, Some(o)) = (self.skipped.is_empty(), out.as_object_mut()) {
            o.insert("notes".into(), json!(self.skipped));
        }
        // 형식 2 공작물: 매거진의 남은 수, 규칙이 쓰지 않는 비트, 부품 기록의 끝 20개
        if let (Some(w), Some(o)) = (&self.work, out.as_object_mut()) {
            o.insert("stacks".into(), w.stacks_json());
            o.insert("forced".into(), json!(w.forced_names()));
            o.insert("part_log".into(), json!(w.log[w.log.len().saturating_sub(20)..]));
            o.insert("part_log_truncated".into(), json!(w.log_truncated)); // 기록이 500줄로 차면 더 쓰지 않는다 (B8 BR2)
        }
        out
    }

    fn holder_id(&self, c: Carrier) -> Option<&str> {
        match c {
            Carrier::None => None,
            Carrier::Axis(i) => self.axes.get(i).map(|a| a.id.as_str()),
            Carrier::Hold(k) => self.holders.get(k).map(|h| h.id.as_str()),
        }
    }

    /// 부품의 (id, x, z). `twin_sim_run`의 위치 표가 쓴다.
    pub fn parts(&self) -> impl Iterator<Item = (u32, f64, f64)> + '_ {
        self.parts.iter().map(|p| (p.id, p.x, p.z)).chain(self.work.iter().flat_map(|r| r.parts.iter().map(|p| (p.id, p.p[0], p.p[2]))))
    }

    pub fn parts_dropped(&self) -> u64 {
        self.parts_dropped + self.work.as_ref().map_or(0, |r| r.parts_dropped)
    }
}

/// 고장 목록(JSON 배열)을 읽는다. 항목: `{device, fault, sensor?, coil?, factor?}`.
pub fn parse_faults(v: &Value, addrs: &dyn Addresses) -> Result<Vec<Fault>, String> {
    let a = v.as_array().filter(|a| a.len() <= 32).ok_or("faults must be an array of at most 32")?;
    a.iter()
        .map(|f| {
            let o = f.as_object().ok_or("each fault must be an object")?;
            if let Some(k) = o.keys().find(|k| !matches!(k.as_str(), "device" | "fault" | "sensor" | "coil" | "factor")) {
                return Err(format!("fault field {} is unknown", k.chars().take(20).collect::<String>()));
            }
            let device = o.get("device").and_then(Value::as_str).filter(|s| s.len() <= 32).ok_or("each fault needs device (string)")?.to_string();
            let kind = match o.get("fault").and_then(Value::as_str) {
                Some("sensor_stuck_on") => FaultKind::SensorStuckOn,
                Some("sensor_stuck_off") => FaultKind::SensorStuckOff,
                Some("coil_dead") => FaultKind::CoilDead,
                Some("slow") => FaultKind::Slow,
                Some("air_off") => FaultKind::AirOff,
                _ => return Err("fault must be sensor_stuck_on, sensor_stuck_off, coil_dead, slow or air_off".into()),
            };
            let addr = match o.get("sensor").or_else(|| o.get("coil")).and_then(Value::as_str) {
                None => None,
                Some(t) => Some(addrs.bit(t).ok_or("fault sensor or coil must be an X, Y or M address")?),
            };
            let factor = match (kind, o.get("factor")) {
                (FaultKind::Slow, Some(x)) => x.as_f64().filter(|x| (1.0..=10.0).contains(x)).ok_or("slow factor must be 1 to 10")?,
                (FaultKind::Slow, None) => return Err("slow needs factor (1 to 10)".into()),
                _ => 1.0,
            };
            Ok(Fault { device, kind, addr, factor })
        })
        .collect()
}
