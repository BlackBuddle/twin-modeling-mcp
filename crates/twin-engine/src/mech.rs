//! 설비 모델 엔진(reusable-parts 2.1a, 설계 45): 관절, 결합, 구동기, 창 센서와 그 한 걸음(tick).
//! 신호의 이름과 핀 번호만 안다. PLC 주소, 시계, 난수가 없고 순서가 고정이라 같은 입력이면 같은 결과가 나온다.
//! 다른 모듈을 쓰지 않는다(`serde_json`과 `std`만). 관절 변환은 평범한 배열로 계산해 `geom`이 틀에 곱한다.

use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

pub type V3 = [f64; 3];

pub const MAX_BYTES: usize = 16384;
const MAX_JOINTS: usize = 8;
const MAX_ITEMS: usize = 8;
const MAX_SENSORS: usize = 16;
/// 장면 전체의 상한(`scene.rs`가 센다)
pub const MAX_DEV_SCENE: usize = 32;
pub const MAX_JOINTS_SCENE: usize = 64;
pub const MAX_COUPLINGS_SCENE: usize = 64;
pub const MAX_DRIVERS_SCENE: usize = 64;
pub const MAX_SENSORS_SCENE: usize = 128;
pub const MAX_POINTS_SCENE: usize = 512;
pub const MAX_SEGMENTS_SCENE: usize = 1024;
/// 두 코일 경보가 풀리는 시간(ms; 설비 모델의 오래된 규칙과 같다)
const NEVER_MS: f64 = 500.0;
/// 한계 밖으로 나간 값을 경보로 볼 오차
const LIMIT_TOL: f64 = 1e-6;
const EPS: f64 = 1e-9;
const BASIS_VALUES: &[&str] = &["user", "label", "measured", "ref", "guess"];
/// `basis`의 열쇠로 쓰는 필드 이름(닫힌 목록)
const BASIS_KEYS: &[&str] = &[
    "lead_mm", "pinion_d_mm", "teeth", "k", "points", "wheel_d_mm", "track_mm", "count", "step", "move_ms", "lag_ms", "fwd_ms", "back_ms", "speed", "per_unit", "programs", "pos", "min", "max", "at", "window",
];
/// `ask`가 묻는 필드(이 순서)
pub const ASK_KEYS: &[&str] = &["lead_mm", "pinion_d_mm", "teeth", "points", "wheel_d_mm", "track_mm", "move_ms", "fwd_ms", "back_ms", "speed", "programs"];

// ───────────── 입출력(핀) ─────────────

/// 엔진이 신호를 읽고 쓰는 길. 핀 번호는 `Ctx::sigs`의 번호이다.
pub trait Pins {
    /// PLC가 내보내는 비트
    fn bit(&self, pin: usize) -> bool;
    /// PLC가 내보내는 워드(`width` 32는 이 워드와 다음 워드, 아래 단어가 먼저). BCD에 9보다 큰 자리가 있으면 None.
    fn word(&self, pin: usize, width: u8) -> Option<i64>;
    /// 설비가 쓰는 비트
    fn set_bit(&mut self, pin: usize, v: bool);
}

/// 파서가 보는 신호 하나(방향은 PLC에서 본 것: `out`이면 PLC가 내보낸다)
#[derive(Clone, Debug)]
pub struct Sig {
    pub name: String,
    pub out: bool,
    pub word: bool,
    pub bound: bool,
    /// `type: bcd`(32비트 읽기는 BCD로 풀지 않으므로 `follow`의 `width: 32`가 거부한다)
    pub bcd: bool,
}

pub struct Ctx<'a> {
    /// 장치가 이미 가진 틀 이름(관절 이름과 `on`이 본다)
    pub frames: &'a [String],
    pub params: &'a Map<String, Value>,
    pub sigs: &'a [Sig],
    /// 형태 축(선형 서보 축의 `carriage`)의 값 범위. 없으면 `form` 관절을 못 쓴다.
    pub form_range: Option<[f64; 2]>,
    /// 장치의 종류 이름과 카탈로그의 `presets`(프리셋 펼치기가 본다; 이 단위는 `catalog.json`을 읽지 않는다)
    pub kind: &'a str,
    pub presets: &'a Value,
}

// ───────────── 자료 ─────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum JType {
    Fixed,
    Prismatic,
    Revolute,
    Planar,
    Form,
}

impl JType {
    /// 장면 JSON의 `type` 글자
    pub fn name(self) -> &'static str {
        match self {
            JType::Fixed => "fixed",
            JType::Prismatic => "prismatic",
            JType::Revolute => "revolute",
            JType::Planar => "planar",
            JType::Form => "form",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Joint {
    pub name: String,
    pub ty: JType,
    /// 앞 관절 번호(없으면 장치의 틀)
    pub on: Option<usize>,
    /// 부모 틀 이름: 장치의 틀이거나 앞 관절의 이름
    pub on_frame: String,
    pub pos: V3,
    pub rot: V3,
    /// 부호가 들어간 단위 벡터
    pub axis: V3,
    pub lim: Option<[f64; 2]>,
    pub wrap: bool,
    pub zero: f64,
    pub start: V3,
    pub area: [f64; 4],
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Valve {
    /// 5/2 양쪽 코일: 코일이 모두 꺼지면 마지막 위치를 지킨다.
    Hold,
    /// 5/2 한쪽 코일, 3/2: 코일이 꺼지면 스프링으로 돌아간다.
    Spring,
    /// 5/3 닫힘·배기, 밸브 없음: 코일이 모두 꺼지면 선다.
    Center,
}

#[derive(Clone, Debug)]
pub enum Coupling {
    Ratio { from: usize, to: usize, k: f64 },
    Table { from: usize, to: usize, pts: Vec<[f64; 2]>, period: Option<f64> },
    Diff { left: usize, right: usize, base: usize, wheel_d: f64, track: f64, last: [f64; 2] },
}

#[derive(Clone, Debug)]
pub struct Segment {
    pub ms: f64,
    /// `to`: 목표 자세, `rate`: 단위/s
    pub to: Option<Vec<f64>>,
    pub rate: Option<Vec<f64>>,
}

#[derive(Clone, Debug)]
pub struct Program {
    pub no: i64,
    pub segs: Vec<Segment>,
}

#[derive(Clone, Debug)]
pub enum Driver {
    Coils { joint: usize, fwd: usize, back: Option<usize>, valve: Option<Valve>, lag: f64, fwd_ms: f64, back_ms: f64, rate: f64, spool: i8, lag_left: f64, both: bool, both_off: f64 },
    Follow { joint: usize, word: usize, width: u8, per_unit: f64, speed: Option<f64>, enable: Option<usize>, busy: Option<usize>, busy_now: bool },
    Step { joint: usize, pulse: usize, step: f64, move_ms: f64, lag: f64, busy: Option<usize>, prev: bool, moving: bool, target: f64, lag_left: f64, warned: bool, busy_now: bool },
    Playback { joints: Vec<usize>, select: usize, select_name: String, start: usize, busy: Option<usize>, done: Option<usize>, programs: Vec<Program>, prev: bool, running: bool, cur: usize, seg: usize, e: f64, v0: Vec<f64>, done_now: bool, warned_busy: bool, warned_bcd: bool, warned_no: Vec<i64> },
}

#[derive(Clone, Debug)]
pub struct Sensor {
    pub joint: usize,
    pub at: f64,
    pub window: f64,
    pub period: Option<f64>,
    pub out: usize,
    pub on_ms: f64,
    pub off_ms: f64,
    pub state: bool,
    pub acc: f64,
}

#[derive(Clone, Debug)]
pub struct Mech {
    pub joints: Vec<Joint>,
    pub couplings: Vec<Coupling>,
    pub drivers: Vec<Driver>,
    pub sensors: Vec<Sensor>,
    pub basis: BTreeMap<String, String>,
    /// `ask`가 보는 필드 이름(`ASK_KEYS`) 중 장면이 준 것
    pub given: BTreeSet<String>,
    /// 구동기나 센서가 쓰는 신호 중 주소가 없는 첫 번째 이름
    pub unbound: Option<String>,
    /// 설비가 쓰는 핀과 그 자리(`drivers[0].busy`): 장면 전체의 단일 쓰기 검사가 쓴다
    pub writes: Vec<(usize, String)>,
    pub n_points: usize,
    pub n_segments: usize,
    /// 프리셋 이름과 펼친 명시 형태(`facts.mech[].expanded`; `param`은 풀지 않은 채이다)
    pub preset: Option<String>,
    pub expanded: Option<Value>,
}

/// 짧은 형태 `position`(서보 축의 지령 단어 하나)의 엔진: 형태 관절 `carriage`와 `follow` 구동기 하나(설계 45의 MM9).
/// 핀 0은 지령 단어, 있는 경우 그다음이 `enable`(`servo_on`), 그다음이 `busy`이다. 결과의 `joints`와 `facts.mech`에는 나오지 않는다(플랜트의 `MechRt.short`).
pub fn short_position(range: [f64; 2], width: u8, per_unit: f64, speed: Option<f64>, enable: bool, busy: bool) -> Mech {
    let joint = Joint { name: "carriage".into(), ty: JType::Form, on: None, on_frame: "base".into(), pos: [0.0; 3], rot: [0.0; 3], axis: [0.0; 3], lim: Some(range), wrap: false, zero: 0.0, start: [range[0], 0.0, 0.0], area: [-100000.0, 100000.0, -100000.0, 100000.0] };
    let driver = Driver::Follow { joint: 0, word: 0, width, per_unit, speed, enable: enable.then_some(1), busy: busy.then_some(1 + usize::from(enable)), busy_now: false };
    Mech { joints: vec![joint], couplings: Vec::new(), drivers: vec![driver], sensors: Vec::new(), basis: BTreeMap::new(), given: BTreeSet::new(), unbound: None, writes: Vec::new(), n_points: 0, n_segments: 0, preset: None, expanded: None }
}

/// 엔진 경보 한 건(플랜트가 장치 id를 붙여 경보 목록에 넣는다)
#[derive(Clone, Debug, PartialEq)]
pub struct MAlarm {
    pub kind: &'static str,
    pub key: Option<(&'static str, Value)>,
}

// ───────────── 글과 오류 ─────────────

fn f(x: f64) -> String {
    format!("{x}")
}

fn err(id: &str, path: &str, msg: &str) -> String {
    if path.is_empty() {
        format!("device {id}: mech: {msg}")
    } else {
        format!("device {id}: mech.{path}: {msg}")
    }
}

/// 주소 없는 신호를 쓰는 `mech`를 돌릴 수 없다는 글(`twin_sim_run`의 오류와 시뮬레이션 상태의 `notes`가 같은 글을 쓴다)
pub fn unbound_text(id: &str, sig: &str) -> String {
    format!("device {id}: mech: signal {sig} has no device; bind it before a simulation (step 8 of guide.modeling)")
}

fn p(base: &str, key: &str) -> String {
    if base.is_empty() {
        key.to_string()
    } else {
        format!("{base}.{key}")
    }
}

fn round3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0 + 0.0
}

/// 보고하는 관절 값: 먼저 0.001로 반올림하고 그다음 감는다(359.9999999가 360으로 나가지 않게)
pub fn report(j: &Joint, v: f64) -> f64 {
    let r = round3(v);
    if j.wrap {
        r.rem_euclid(360.0) + 0.0
    } else {
        r
    }
}

/// `{"param": 이름, "mul": m}`을 장치의 매개변수 값으로 바꾼다. `{"arg": ...}`는 프리셋 것이라 거부한다.
fn subst(id: &str, path: &str, v: &Value, params: &Map<String, Value>) -> Result<Value, String> {
    match v {
        Value::Object(m) if m.contains_key("param") || m.contains_key("arg") => {
            if m.contains_key("arg") {
                return Err(err(id, path, "arg is only for a preset"));
            }
            if let Some(k) = m.keys().find(|k| *k != "param" && *k != "mul") {
                return Err(err(id, &p(path, k), "unknown field"));
            }
            let name = m["param"].as_str().ok_or_else(|| err(id, path, "param must be a parameter name"))?;
            let base = params.get(name).and_then(Value::as_f64).ok_or_else(|| err(id, path, &format!("params.{name} is not given")))?;
            let mul = match m.get("mul") {
                None => 1.0,
                Some(x) => x.as_f64().filter(|n| n.is_finite()).ok_or_else(|| err(id, &p(path, "mul"), "must be a number"))?,
            };
            let x = base * mul;
            if !x.is_finite() {
                return Err(err(id, path, "param times mul is not a finite number"));
            }
            Ok(json!(x))
        }
        Value::Object(m) => {
            let mut o = Map::new();
            for (k, x) in m {
                o.insert(k.clone(), subst(id, &p(path, k), x, params)?);
            }
            Ok(Value::Object(o))
        }
        Value::Array(a) => a.iter().enumerate().map(|(i, x)| subst(id, &format!("{path}[{i}]"), x, params)).collect::<Result<Vec<_>, _>>().map(Value::Array),
        _ => Ok(v.clone()),
    }
}

fn walk_keys(v: &Value, out: &mut BTreeSet<String>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                if ASK_KEYS.contains(&k.as_str()) {
                    out.insert(k.clone());
                }
                walk_keys(x, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| walk_keys(x, out)),
        _ => {}
    }
}

fn only(id: &str, o: &Map<String, Value>, base: &str, keys: &[&str]) -> Result<(), String> {
    match o.keys().find(|k| !keys.contains(&k.as_str())) {
        Some(k) => Err(err(id, &p(base, &k.chars().take(24).collect::<String>()), "unknown field")),
        None => Ok(()),
    }
}

fn num(id: &str, o: &Map<String, Value>, base: &str, key: &str, lo: f64, hi: f64) -> Result<Option<f64>, String> {
    match o.get(key).filter(|v| !v.is_null()) {
        None => Ok(None),
        Some(v) => v.as_f64().filter(|n| n.is_finite() && *n >= lo && *n <= hi).map(Some).ok_or_else(|| err(id, &p(base, key), &format!("must be {} to {}", f(lo), f(hi)))),
    }
}

fn need_num(id: &str, o: &Map<String, Value>, base: &str, key: &str, lo: f64, hi: f64) -> Result<f64, String> {
    num(id, o, base, key, lo, hi)?.ok_or_else(|| err(id, &p(base, key), &format!("is needed (must be {} to {})", f(lo), f(hi))))
}

fn int(id: &str, o: &Map<String, Value>, base: &str, key: &str, lo: i64, hi: i64) -> Result<Option<i64>, String> {
    match o.get(key).filter(|v| !v.is_null()) {
        None => Ok(None),
        // `{"param": ...}`로 들어온 수는 8.0처럼 실수 모양이다: 소수부가 없으면 정수로 본다
        Some(v) => v
            .as_i64()
            .or_else(|| v.as_f64().filter(|x| x.fract() == 0.0 && x.abs() < 1e15).map(|x| x as i64))
            .filter(|n| *n >= lo && *n <= hi)
            .map(Some)
            .ok_or_else(|| err(id, &p(base, key), &format!("must be an integer from {lo} to {hi}"))),
    }
}

fn text<'a>(id: &str, o: &'a Map<String, Value>, base: &str, key: &str) -> Result<&'a str, String> {
    o.get(key).and_then(Value::as_str).ok_or_else(|| err(id, &p(base, key), "is needed (text)"))
}

fn vec3(v: &Value, lim: f64) -> Option<V3> {
    let a = v.as_array().filter(|a| a.len() == 3)?;
    let mut o = [0.0; 3];
    for (i, x) in a.iter().enumerate() {
        o[i] = x.as_f64().filter(|n| n.is_finite() && n.abs() <= lim)?;
    }
    Some(o)
}

fn axis_of(s: &str) -> Option<V3> {
    Some(match s {
        "x" => [1.0, 0.0, 0.0],
        "-x" => [-1.0, 0.0, 0.0],
        "y" => [0.0, 1.0, 0.0],
        "-y" => [0.0, -1.0, 0.0],
        "z" => [0.0, 0.0, 1.0],
        "-z" => [0.0, 0.0, -1.0],
        _ => return None,
    })
}

// ───────────── 분석 ─────────────

struct Parse<'a> {
    id: &'a str,
    c: &'a Ctx<'a>,
    joints: Vec<Joint>,
    /// 관절마다 값을 움직이는 것(`drivers[0]`, `couplings[1]`)
    source: Vec<Option<String>>,
    unbound: Option<String>,
    writes: Vec<(usize, String)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pin {
    /// PLC가 내보내는 비트
    Bit,
    /// PLC가 내보내는 워드나 BCD
    Word,
    /// 설비가 쓰는 비트
    Out,
}

impl Parse<'_> {
    fn joint(&self, path: &str, name: &str) -> Result<usize, String> {
        self.joints.iter().position(|j| j.name == name).ok_or_else(|| {
            let names: Vec<&str> = self.joints.iter().map(|j| j.name.as_str()).collect();
            err(self.id, path, &format!("no joint \"{}\" (joints: {})", name.chars().take(24).collect::<String>(), names.join(", ")))
        })
    }

    fn moving_joint(&self, path: &str, name: &str, what: &str) -> Result<usize, String> {
        let k = self.joint(path, name)?;
        match self.joints[k].ty {
            JType::Prismatic | JType::Revolute | JType::Form => Ok(k),
            _ => Err(err(self.id, path, &format!("{name} is a {} joint; {what}", if self.joints[k].ty == JType::Planar { "planar" } else { "fixed" }))),
        }
    }

    /// 신호 이름을 핀 번호로 푼다. 방향과 종류를 본다(설계 2.7). `who`는 오류 글의 주어(driver, sensor).
    fn pin(&mut self, path: &str, name: &str, kind: Pin, who: &str) -> Result<usize, String> {
        let field = path.rsplit('.').next().unwrap_or(path).split('[').next().unwrap_or(path);
        let Some(k) = self.c.sigs.iter().position(|s| s.name == name) else {
            let names: Vec<&str> = self.c.sigs.iter().map(|s| s.name.as_str()).collect();
            return Err(err(self.id, path, &format!("no signal \"{}\" (signals: {})", name.chars().take(24).collect::<String>(), names.join(", "))));
        };
        let s = &self.c.sigs[k];
        match kind {
            Pin::Bit | Pin::Word if !s.out => return Err(err(self.id, path, &format!("{name} is an input of the PLC (dir in); a {who} reads a signal with dir out"))),
            Pin::Out if s.out => return Err(err(self.id, path, &format!("{name} is an output of the PLC (dir out); a {who} writes a signal with dir in"))),
            _ => {}
        }
        match kind {
            Pin::Bit | Pin::Out if s.word => return Err(err(self.id, path, &format!("{name} is a word signal; {field} needs a bit signal"))),
            Pin::Word if !s.word => return Err(err(self.id, path, &format!("{name} is a bit signal; {field} needs a word or bcd signal"))),
            _ => {}
        }
        if !s.bound && self.unbound.is_none() {
            self.unbound = Some(s.name.clone());
        }
        if kind == Pin::Out {
            if let Some((_, other)) = self.writes.iter().find(|(q, _)| *q == k) {
                return Err(err(self.id, "", &format!("signal {name} is written twice ({other}, {path})")));
            }
            self.writes.push((k, path.to_string()));
        }
        Ok(k)
    }

    fn opt_pin(&mut self, o: &Map<String, Value>, base: &str, key: &str, kind: Pin, who: &str) -> Result<Option<usize>, String> {
        match o.get(key).filter(|v| !v.is_null()) {
            None => Ok(None),
            Some(v) => {
                let n = v.as_str().ok_or_else(|| err(self.id, &p(base, key), "must be a signal name"))?;
                self.pin(&p(base, key), n, kind, who).map(Some)
            }
        }
    }

    fn claim(&mut self, k: usize, who: String, path: &str) -> Result<(), String> {
        if let Some(prev) = &self.source[k] {
            let both = prev.starts_with("drivers") == who.starts_with("drivers");
            let msg = if both { format!("{} is moved twice ({prev}, {who})", self.joints[k].name) } else { format!("{} is moved by {prev} and {who}", self.joints[k].name) };
            return Err(err(self.id, path, &msg));
        }
        self.source[k] = Some(who);
        Ok(())
    }
}

fn parse_joints(id: &str, c: &Ctx, v: &Value) -> Result<Vec<Joint>, String> {
    let a = v.as_array().filter(|a| (1..=MAX_JOINTS).contains(&a.len())).ok_or_else(|| err(id, "joints", &format!("must be 1 to {MAX_JOINTS} joints")))?;
    let mut out: Vec<Joint> = Vec::new();
    let (mut forms, mut planars) = (0, 0);
    for (i, jv) in a.iter().enumerate() {
        let base = format!("joints[{i}]");
        let o = jv.as_object().ok_or_else(|| err(id, &base, "must be an object"))?;
        only(id, o, &base, &["name", "type", "on", "pos", "rot", "axis", "min", "max", "wrap", "zero", "start", "area"])?;
        let name = o.get("name").and_then(Value::as_str).filter(|n| (1..=16).contains(&n.len()) && n.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')).ok_or_else(|| err(id, &p(&base, "name"), "must be 1 to 16 of a-z 0-9 _"))?;
        let ty = match o.get("type").and_then(Value::as_str) {
            Some("fixed") => JType::Fixed,
            Some("prismatic") => JType::Prismatic,
            Some("revolute") => JType::Revolute,
            Some("planar") => JType::Planar,
            Some("form") => JType::Form,
            _ => return Err(err(id, &p(&base, "type"), "must be fixed, prismatic, revolute, planar or form")),
        };
        if out.iter().any(|j| j.name == name) {
            return Err(err(id, &p(&base, "name"), &format!("{name} is used twice")));
        }
        if ty == JType::Form {
            let Some(_) = c.form_range else { return Err(err(id, &base, "form needs a kind with a form axis that has no valve (servo_axis)")) };
            if name != "carriage" {
                return Err(err(id, &p(&base, "name"), "the form joint of servo_axis is named carriage"));
            }
            forms += 1;
        } else if c.frames.iter().any(|fr| fr == name) {
            return Err(err(id, &p(&base, "name"), &format!("{name} is a frame of {id} (frames: {})", c.frames.join(", "))));
        }
        if ty == JType::Planar {
            planars += 1;
        }
        if forms > 1 {
            return Err(err(id, "joints", "at most one form joint"));
        }
        if planars > 1 {
            return Err(err(id, "joints", "at most one planar joint"));
        }
        let on_name = match o.get("on").filter(|v| !v.is_null()) {
            None => "base",
            Some(v) => v.as_str().ok_or_else(|| err(id, &p(&base, "on"), "must be text"))?,
        };
        let on = out.iter().position(|j| j.name == on_name);
        if on.is_none() && !c.frames.iter().any(|fr| fr == on_name) {
            let names: Vec<&str> = out.iter().map(|j| j.name.as_str()).collect();
            return Err(err(id, &p(&base, "on"), &format!("no frame or earlier joint \"{}\" (frames: {}; joints: {})", on_name.chars().take(24).collect::<String>(), c.frames.join(", "), names.join(", "))));
        }
        if ty == JType::Planar && on_name != "base" {
            return Err(err(id, &p(&base, "on"), "planar must be on base"));
        }
        // 부모 사슬의 깊이(장치 안에서 8단계 이하)
        let mut depth = 1;
        let mut cur = on;
        while let Some(k) = cur {
            depth += 1;
            cur = out[k].on;
        }
        if depth > MAX_JOINTS {
            return Err(err(id, &p(&base, "on"), &format!("more than {MAX_JOINTS} levels of joints")));
        }
        let pos = match o.get("pos").filter(|v| !v.is_null()) {
            None => [0.0; 3],
            Some(v) => vec3(v, 20000.0).ok_or_else(|| err(id, &p(&base, "pos"), "must be 3 numbers from -20000 to 20000"))?,
        };
        let rot = match o.get("rot").filter(|v| !v.is_null()) {
            None => [0.0; 3],
            Some(v) => vec3(v, 360.0).ok_or_else(|| err(id, &p(&base, "rot"), "must be 3 numbers from -360 to 360"))?,
        };
        let moving = matches!(ty, JType::Prismatic | JType::Revolute);
        let axis = match (o.get("axis").filter(|v| !v.is_null()), moving) {
            (Some(v), true) => v.as_str().and_then(axis_of).ok_or_else(|| err(id, &p(&base, "axis"), "must be x, y, z, -x, -y or -z"))?,
            (None, true) => return Err(err(id, &p(&base, "axis"), "must be x, y, z, -x, -y or -z")),
            (Some(_), false) => return Err(err(id, &p(&base, "axis"), "only prismatic and revolute joints have an axis")),
            (None, false) => [0.0; 3],
        };
        let wrap = match o.get("wrap").filter(|v| !v.is_null()) {
            None => false,
            Some(Value::Bool(b)) => *b,
            Some(_) => return Err(err(id, &p(&base, "wrap"), "must be true or false")),
        };
        if wrap && ty != JType::Revolute {
            return Err(err(id, &base, "wrap is only for a revolute joint"));
        }
        let (lo_lim, hi_lim) = if ty == JType::Prismatic { (-20000.0, 20000.0) } else { (-3600.0, 3600.0) };
        let (min, max) = (num(id, o, &base, "min", lo_lim, hi_lim)?, num(id, o, &base, "max", lo_lim, hi_lim)?);
        let lim = match ty {
            JType::Form => {
                if min.is_some() || max.is_some() {
                    return Err(err(id, &base, "the range of the form joint is the axis range"));
                }
                c.form_range
            }
            JType::Prismatic | JType::Revolute if wrap => {
                if min.is_some() || max.is_some() {
                    return Err(err(id, &base, "min and max are not for a wrap joint"));
                }
                None
            }
            JType::Prismatic | JType::Revolute => match (min, max) {
                (Some(a), Some(b)) => {
                    if a >= b {
                        return Err(err(id, &p(&base, "min"), "must be below max"));
                    }
                    Some([a, b])
                }
                _ => return Err(err(id, &base, &format!("{} needs min and max{}", if ty == JType::Prismatic { "prismatic" } else { "revolute" }, if ty == JType::Revolute { " (or wrap)" } else { "" }))),
            },
            _ => {
                if min.is_some() || max.is_some() {
                    return Err(err(id, &base, "min and max are only for prismatic and revolute joints"));
                }
                None
            }
        };
        let zero = num(id, o, &base, "zero", -100000.0, 100000.0)?.unwrap_or(0.0);
        let mut area = [-100000.0, 100000.0, -100000.0, 100000.0];
        if ty == JType::Planar {
            if let Some(v) = o.get("area").filter(|v| !v.is_null()) {
                let a = v.as_array().filter(|a| a.len() == 4).and_then(|a| a.iter().map(|x| x.as_f64().filter(|n| n.is_finite() && n.abs() <= 100000.0)).collect::<Option<Vec<_>>>());
                let a = a.filter(|a| a[0] < a[1] && a[2] < a[3]).ok_or_else(|| err(id, &p(&base, "area"), "must be [xmin, xmax, zmin, zmax] from -100000 to 100000, each min below its max"))?;
                area = [a[0], a[1], a[2], a[3]];
            }
        } else if o.get("area").is_some_and(|v| !v.is_null()) {
            return Err(err(id, &p(&base, "area"), "only for a planar joint"));
        }
        let start = match (ty, o.get("start").filter(|v| !v.is_null())) {
            (JType::Form, Some(_)) => return Err(err(id, &p(&base, "start"), "the form joint starts at the first value of its range")),
            (JType::Form, None) => [lim.map_or(0.0, |l| l[0]), 0.0, 0.0],
            (JType::Planar, s) => {
                let s = match s {
                    None => [0.0; 3],
                    Some(v) => vec3(v, 100000.0).ok_or_else(|| err(id, &p(&base, "start"), "must be [x, z, heading] (3 numbers, x and z from -100000 to 100000)"))?,
                };
                if s[0] < area[0] || s[0] > area[1] || s[1] < area[2] || s[1] > area[3] {
                    return Err(err(id, &p(&base, "start"), "is outside the area"));
                }
                s
            }
            (_, None) => [lim.map_or(zero, |l| zero.clamp(l[0], l[1])), 0.0, 0.0],
            (_, Some(v)) => {
                let x = v.as_f64().filter(|n| n.is_finite() && n.abs() <= 100000.0).ok_or_else(|| err(id, &p(&base, "start"), "must be a number"))?;
                if let Some(l) = lim {
                    if x < l[0] || x > l[1] {
                        return Err(err(id, &p(&base, "start"), &format!("{} is outside {name} ({} to {})", f(x), f(l[0]), f(l[1]))));
                    }
                }
                [x, 0.0, 0.0]
            }
        };
        out.push(Joint { name: name.to_string(), ty, on, on_frame: on_name.to_string(), pos, rot, axis, lim, wrap, zero, start, area });
    }
    Ok(out)
}

fn is_valve(s: &str) -> Option<Valve> {
    match s {
        "5/2-double" => Some(Valve::Hold),
        "5/2-single" | "3/2-single" => Some(Valve::Spring),
        "5/3-closed" | "5/3-exhaust" => Some(Valve::Center),
        _ => None,
    }
}

fn parse_driver(ps: &mut Parse, i: usize, v: &Value) -> Result<Driver, String> {
    let id = ps.id;
    let base = format!("drivers[{i}]");
    let o = v.as_object().ok_or_else(|| err(id, &base, "must be an object"))?;
    let ty = o.get("type").and_then(Value::as_str).unwrap_or("");
    let who = format!("drivers[{i}]");
    match ty {
        "coils" => {
            only(id, o, &base, &["type", "joint", "fwd", "back", "valve", "lag_ms", "fwd_ms", "back_ms", "speed"])?;
            let jn = text(id, o, &base, "joint")?;
            let joint = ps.moving_joint(&p(&base, "joint"), jn, "a coils driver moves a prismatic, revolute or form joint")?;
            let vname = o.get("valve").and_then(Value::as_str).unwrap_or("");
            let none = vname == "none";
            let valve = if none { None } else { Some(is_valve(vname).ok_or_else(|| err(id, &p(&base, "valve"), "must be 5/2-double, 5/2-single, 3/2-single, 5/3-closed, 5/3-exhaust or none"))?) };
            let fwd = ps.pin(&p(&base, "fwd"), text(id, o, &base, "fwd")?, Pin::Bit, "driver")?;
            let back = ps.opt_pin(o, &base, "back", Pin::Bit, "driver")?;
            let (mut lag, mut fwd_ms, mut back_ms, mut rate) = (0.0, 1.0, 1.0, 0.0);
            if none {
                if o.get("fwd_ms").is_some() || o.get("back_ms").is_some() || o.get("lag_ms").is_some() {
                    return Err(err(id, &base, "fwd_ms, back_ms and lag_ms are not for valve none"));
                }
                rate = need_num(id, o, &base, "speed", 0.0, 100000.0)? / 1000.0;
            } else {
                if o.get("speed").is_some_and(|x| !x.is_null()) {
                    return Err(err(id, &p(&base, "speed"), "needs valve none"));
                }
                if ps.joints[joint].lim.is_none() {
                    return Err(err(id, &p(&base, "joint"), &format!("a valve driver needs a joint with min and max ({jn} wraps)")));
                }
                lag = num(id, o, &base, "lag_ms", 0.0, 60000.0)?.unwrap_or(50.0);
                fwd_ms = need_num(id, o, &base, "fwd_ms", 1.0, 60000.0)?;
                back_ms = need_num(id, o, &base, "back_ms", 1.0, 60000.0)?;
            }
            ps.claim(joint, who, &p(&base, "joint"))?;
            // 처음 스풀: 행정의 시작 쪽 끝이면 그 방향(원통의 규칙), 아니면 0
            let j = &ps.joints[joint];
            let spool = match (valve, j.lim) {
                (Some(_), Some(l)) if j.start[0] == l[0] => -1,
                (Some(_), Some(l)) if j.start[0] == l[1] => 1,
                _ => 0,
            };
            Ok(Driver::Coils { joint, fwd, back, valve, lag, fwd_ms, back_ms, rate, spool, lag_left: 0.0, both: false, both_off: 0.0 })
        }
        "follow" => {
            only(id, o, &base, &["type", "joint", "word", "width", "per_unit", "speed", "enable", "busy"])?;
            let jn = text(id, o, &base, "joint")?;
            let joint = ps.joint(&p(&base, "joint"), jn)?;
            if !matches!(ps.joints[joint].ty, JType::Prismatic | JType::Form) {
                return Err(err(id, &p(&base, "joint"), &format!("{jn} must be a prismatic or form joint")));
            }
            let width = match o.get("width").filter(|x| !x.is_null()) {
                None => 16,
                Some(x) => x.as_u64().filter(|w| *w == 16 || *w == 32).ok_or_else(|| err(id, &p(&base, "width"), "must be 16 or 32"))? as u8,
            };
            let word = ps.pin(&p(&base, "word"), text(id, o, &base, "word")?, Pin::Word, "driver")?;
            if width == 32 && ps.c.sigs[word].bcd {
                return Err(err(id, &p(&base, "width"), "must be 16 for a bcd signal"));
            }
            let per_unit = num(id, o, &base, "per_unit", 0.0001, 100000.0)?.unwrap_or(1.0);
            let speed = num(id, o, &base, "speed", 0.1, 5000.0)?;
            let enable = ps.opt_pin(o, &base, "enable", Pin::Bit, "driver")?;
            let busy = ps.opt_pin(o, &base, "busy", Pin::Out, "driver")?;
            ps.claim(joint, who, &p(&base, "joint"))?;
            Ok(Driver::Follow { joint, word, width, per_unit, speed, enable, busy, busy_now: false })
        }
        "step" => {
            only(id, o, &base, &["type", "joint", "pulse", "count", "step", "move_ms", "lag_ms", "busy"])?;
            let jn = text(id, o, &base, "joint")?;
            let joint = ps.moving_joint(&p(&base, "joint"), jn, "a step driver moves a prismatic or revolute joint")?;
            let pulse = ps.pin(&p(&base, "pulse"), text(id, o, &base, "pulse")?, Pin::Bit, "driver")?;
            let has = |k: &str| o.get(k).is_some_and(|x| !x.is_null());
            if has("count") == has("step") {
                return Err(err(id, &base, "give exactly one of count and step"));
            }
            let step = if has("count") {
                if ps.joints[joint].ty != JType::Revolute {
                    return Err(err(id, &p(&base, "count"), "needs a revolute joint"));
                }
                360.0 / int(id, o, &base, "count", 2, 360)?.unwrap_or(2) as f64
            } else {
                let s = need_num(id, o, &base, "step", -20000.0, 20000.0)?;
                if s == 0.0 {
                    return Err(err(id, &p(&base, "step"), "must not be 0"));
                }
                s
            };
            let move_ms = need_num(id, o, &base, "move_ms", 1.0, 60000.0)?;
            let lag = num(id, o, &base, "lag_ms", 0.0, 60000.0)?.unwrap_or(0.0);
            let busy = ps.opt_pin(o, &base, "busy", Pin::Out, "driver")?;
            ps.claim(joint, who, &p(&base, "joint"))?;
            let target = ps.joints[joint].start[0];
            Ok(Driver::Step { joint, pulse, step, move_ms, lag, busy, prev: false, moving: false, target, lag_left: 0.0, warned: false, busy_now: false })
        }
        "playback" => {
            only(id, o, &base, &["type", "joints", "select", "start", "busy", "done", "programs"])?;
            let names = o.get("joints").and_then(Value::as_array).filter(|a| (1..=MAX_JOINTS).contains(&a.len())).ok_or_else(|| err(id, &p(&base, "joints"), &format!("must be 1 to {MAX_JOINTS} joints of this device")))?;
            let mut joints = Vec::new();
            for (k, n) in names.iter().enumerate() {
                let n = n.as_str().ok_or_else(|| err(id, &format!("{base}.joints[{k}]"), "must be a joint name"))?;
                let j = ps.moving_joint(&format!("{base}.joints[{k}]"), n, "a playback driver moves prismatic, revolute or form joints")?;
                if joints.contains(&j) {
                    return Err(err(id, &format!("{base}.joints[{k}]"), &format!("{n} is used twice")));
                }
                joints.push(j);
            }
            let sel_name = text(id, o, &base, "select")?;
            let select = ps.pin(&p(&base, "select"), sel_name, Pin::Word, "driver")?;
            let start = ps.pin(&p(&base, "start"), text(id, o, &base, "start")?, Pin::Bit, "driver")?;
            let busy = ps.opt_pin(o, &base, "busy", Pin::Out, "driver")?;
            let done = ps.opt_pin(o, &base, "done", Pin::Out, "driver")?;
            let progs = o.get("programs").and_then(Value::as_array).filter(|a| (1..=16).contains(&a.len())).ok_or_else(|| err(id, &p(&base, "programs"), "must be 1 to 16 programs"))?;
            let mut programs: Vec<Program> = Vec::new();
            for (pi, pv) in progs.iter().enumerate() {
                let pb = format!("{base}.programs[{pi}]");
                let po = pv.as_object().ok_or_else(|| err(id, &pb, "must be an object"))?;
                only(id, po, &pb, &["no", "segments"])?;
                let no = int(id, po, &pb, "no", 1, 9999)?.ok_or_else(|| err(id, &p(&pb, "no"), "is needed (integer from 1 to 9999)"))?;
                if programs.iter().any(|q| q.no == no) {
                    return Err(err(id, &p(&pb, "no"), &format!("{no} is used twice")));
                }
                let sv = po.get("segments").and_then(Value::as_array).filter(|a| (1..=32).contains(&a.len())).ok_or_else(|| err(id, &p(&pb, "segments"), "must be 1 to 32 segments"))?;
                let mut segs = Vec::new();
                for (si, s) in sv.iter().enumerate() {
                    let sb = format!("{pb}.segments[{si}]");
                    let so = s.as_object().ok_or_else(|| err(id, &sb, "must be an object"))?;
                    only(id, so, &sb, &["ms", "to", "rate"])?;
                    let ms = need_num(id, so, &sb, "ms", 1.0, 600000.0)?;
                    let (has_to, has_rate) = (so.get("to").is_some_and(|x| !x.is_null()), so.get("rate").is_some_and(|x| !x.is_null()));
                    if has_to == has_rate {
                        return Err(err(id, &sb, "give exactly one of to and rate"));
                    }
                    let key = if has_to { "to" } else { "rate" };
                    let n = joints.len();
                    // `rate`는 `coils`의 `speed`와 같은 상한을 가진다(없으면 값이 무한이 된다, RR5)
                    let arr = so[key].as_array().filter(|a| a.len() == n && a.iter().all(|x| x.as_f64().is_some_and(|v| v.is_finite() && (has_to || v.abs() <= 100000.0)))).ok_or_else(|| {
                        let range = if has_to { "" } else { " from -100000 to 100000" };
                        err(id, &p(&sb, key), &format!("must be {n} numbers{range} (one for each of joints)"))
                    })?;
                    let vals: Vec<f64> = arr.iter().map(|x| x.as_f64().unwrap_or(0.0)).collect();
                    if has_to {
                        for (k, x) in vals.iter().enumerate() {
                            let j = &ps.joints[joints[k]];
                            if let Some(l) = j.lim {
                                if *x < l[0] || *x > l[1] {
                                    return Err(err(id, &format!("{sb}.to[{k}]"), &format!("{} is outside {} ({} to {})", f(*x), j.name, f(l[0]), f(l[1]))));
                                }
                            }
                        }
                    }
                    segs.push(Segment { ms, to: has_to.then(|| vals.clone()), rate: has_rate.then_some(vals) });
                }
                programs.push(Program { no, segs });
            }
            for &j in &joints {
                ps.claim(j, who.clone(), &p(&base, "joints"))?;
            }
            let v0 = joints.iter().map(|&j| ps.joints[j].start[0]).collect();
            Ok(Driver::Playback { joints, select, select_name: sel_name.to_string(), start, busy, done, programs, prev: false, running: false, cur: 0, seg: 0, e: 0.0, v0, done_now: false, warned_busy: false, warned_bcd: false, warned_no: Vec::new() })
        }
        _ => Err(err(id, &p(&base, "type"), "must be coils, follow, step or playback")),
    }
}

fn parse_coupling(ps: &mut Parse, i: usize, v: &Value) -> Result<(Coupling, usize, usize), String> {
    let id = ps.id;
    let base = format!("couplings[{i}]");
    let o = v.as_object().ok_or_else(|| err(id, &base, "must be an object"))?;
    let who = format!("couplings[{i}]");
    // `from`: 값을 움직이는 것이 앞에 있어야 한다
    let src = |ps: &Parse, path: &str, name: &str| -> Result<usize, String> {
        let k = ps.joint(path, name)?;
        if ps.source[k].is_none() {
            return Err(err(id, path, &format!("{name} has no value source before this coupling (a driver or an earlier coupling must move it)")));
        }
        Ok(k)
    };
    match o.get("type").and_then(Value::as_str).unwrap_or("") {
        "ratio" => {
            only(id, o, &base, &["type", "from", "to", "k", "lead_mm", "pinion_d_mm", "teeth", "reverse"])?;
            let (fp, tp) = (p(&base, "from"), p(&base, "to"));
            let from = src(ps, &fp, text(id, o, &base, "from")?)?;
            let to = ps.moving_joint(&tp, text(id, o, &base, "to")?, "a ratio coupling joins prismatic, revolute or form joints")?;
            if !matches!(ps.joints[from].ty, JType::Prismatic | JType::Revolute | JType::Form) {
                return Err(err(id, &fp, "a ratio coupling joins prismatic, revolute or form joints"));
            }
            let given: Vec<&str> = ["k", "lead_mm", "pinion_d_mm", "teeth"].into_iter().filter(|k| o.get(*k).is_some_and(|x| !x.is_null())).collect();
            if given.len() != 1 {
                return Err(err(id, &base, "give exactly one of k, lead_mm, pinion_d_mm, teeth"));
            }
            let rev = |j: usize| ps.joints[j].ty == JType::Revolute;
            let mut k = match given[0] {
                "k" => {
                    let k = need_num(id, o, &base, "k", -1000000.0, 1000000.0)?;
                    if k == 0.0 {
                        return Err(err(id, &p(&base, "k"), "must not be 0"));
                    }
                    k
                }
                "teeth" => {
                    let t = o["teeth"].as_array().filter(|a| a.len() == 2).and_then(|a| a.iter().map(|x| x.as_i64().filter(|n| (1..=1000).contains(n))).collect::<Option<Vec<_>>>()).ok_or_else(|| err(id, &p(&base, "teeth"), "must be [z_from, z_to], each 1 to 1000"))?;
                    if !(rev(from) && rev(to)) {
                        return Err(err(id, &p(&base, "teeth"), "needs two revolute joints"));
                    }
                    -(t[0] as f64) / t[1] as f64
                }
                key => {
                    let (lo, hi) = if key == "lead_mm" { (0.01, 1000.0) } else { (1.0, 2000.0) };
                    let x = need_num(id, o, &base, key, lo, hi)?;
                    if rev(from) == rev(to) {
                        return Err(err(id, &p(&base, key), "needs one prismatic and one revolute joint"));
                    }
                    let per_deg = if key == "lead_mm" { x / 360.0 } else { std::f64::consts::PI * x / 360.0 };
                    if rev(from) { per_deg } else { 360.0 / (if key == "lead_mm" { x } else { std::f64::consts::PI * x }) }
                }
            };
            if o.get("reverse").is_some_and(|x| x == &Value::Bool(true)) {
                k = -k;
            }
            ps.claim(to, who, &tp)?;
            Ok((Coupling::Ratio { from, to, k }, from, to))
        }
        "table" => {
            only(id, o, &base, &["type", "from", "to", "points", "period"])?;
            let (fp, tp) = (p(&base, "from"), p(&base, "to"));
            let from = src(ps, &fp, text(id, o, &base, "from")?)?;
            let to = ps.moving_joint(&tp, text(id, o, &base, "to")?, "a table coupling joins prismatic, revolute or form joints")?;
            let pv = o.get("points").and_then(Value::as_array).filter(|a| (2..=64).contains(&a.len())).ok_or_else(|| err(id, &p(&base, "points"), "must be 2 to 64 points [x, y]"))?;
            let mut pts: Vec<[f64; 2]> = Vec::new();
            for (k, q) in pv.iter().enumerate() {
                let xy = q.as_array().filter(|a| a.len() == 2).and_then(|a| Some([a[0].as_f64().filter(|n| n.is_finite() && n.abs() <= 1e6)?, a[1].as_f64().filter(|n| n.is_finite() && n.abs() <= 1e6)?]));
                let xy = xy.ok_or_else(|| err(id, &format!("{base}.points[{k}]"), "must be [x, y] with numbers from -1000000 to 1000000"))?;
                if let Some(last) = pts.last() {
                    if xy[0] <= last[0] {
                        return Err(err(id, &format!("{base}.points[{k}]"), &format!("x must grow ({} after {})", f(xy[0]), f(last[0]))));
                    }
                }
                pts.push(xy);
            }
            let period = num(id, o, &base, "period", 1.0, 3600.0)?;
            if let Some(pe) = period {
                if pts[0][0] != 0.0 || pts[pts.len() - 1][0] != pe {
                    return Err(err(id, &p(&base, "period"), &format!("the first x must be 0 and the last x {}", f(pe))));
                }
                if pts[0][1] != pts[pts.len() - 1][1] {
                    return Err(err(id, &p(&base, "period"), &format!("the last y ({}) must equal the first y ({})", f(pts[pts.len() - 1][1]), f(pts[0][1]))));
                }
            }
            ps.claim(to, who, &tp)?;
            Ok((Coupling::Table { from, to, pts, period }, from, to))
        }
        "diff_drive" => {
            only(id, o, &base, &["type", "left", "right", "base", "wheel_d_mm", "track_mm"])?;
            let (lp, rp, bp) = (p(&base, "left"), p(&base, "right"), p(&base, "base"));
            let left = src(ps, &lp, text(id, o, &base, "left")?)?;
            let right = src(ps, &rp, text(id, o, &base, "right")?)?;
            for (k, path) in [(left, &lp), (right, &rp)] {
                if ps.joints[k].ty != JType::Revolute {
                    return Err(err(id, path, &format!("{} is not a revolute joint", ps.joints[k].name)));
                }
            }
            let bn = text(id, o, &base, "base")?;
            let b = ps.joint(&bp, bn)?;
            if ps.joints[b].ty != JType::Planar {
                return Err(err(id, &bp, &format!("{bn} is not a planar joint")));
            }
            let wheel_d = need_num(id, o, &base, "wheel_d_mm", 1.0, 2000.0)?;
            let track = need_num(id, o, &base, "track_mm", 1.0, 10000.0)?;
            ps.claim(b, who, &bp)?;
            let last = [ps.joints[left].start[0], ps.joints[right].start[0]];
            Ok((Coupling::Diff { left, right, base: b, wheel_d, track, last }, left, b))
        }
        _ => Err(err(id, &p(&base, "type"), "must be ratio, table or diff_drive")),
    }
}

fn parse_sensor(ps: &mut Parse, i: usize, v: &Value) -> Result<Sensor, String> {
    let id = ps.id;
    let base = format!("sensors[{i}]");
    let o = v.as_object().ok_or_else(|| err(id, &base, "must be an object"))?;
    only(id, o, &base, &["type", "joint", "at", "window", "every", "per_turn", "out", "on_ms", "off_ms"])?;
    if o.get("type").and_then(Value::as_str) != Some("window") {
        return Err(err(id, &p(&base, "type"), "must be window"));
    }
    let jn = text(id, o, &base, "joint")?;
    let joint = ps.joint(&p(&base, "joint"), jn)?;
    let ty = ps.joints[joint].ty;
    if ty == JType::Planar {
        return Err(err(id, &p(&base, "joint"), "a planar joint has no window sensor in this build"));
    }
    if ty == JType::Fixed {
        return Err(err(id, &p(&base, "joint"), &format!("{jn} is a fixed joint; a window sensor needs a moving joint")));
    }
    let at = need_num(id, o, &base, "at", -100000.0, 100000.0)?;
    let window = need_num(id, o, &base, "window", 0.0, 1000.0)?;
    let (has_every, has_turn) = (o.get("every").is_some_and(|x| !x.is_null()), o.get("per_turn").is_some_and(|x| !x.is_null()));
    if has_every && has_turn {
        return Err(err(id, &base, "give at most one of every and per_turn"));
    }
    let mut period = if ps.joints[joint].wrap { Some(360.0) } else { None };
    if has_every {
        let e = need_num(id, o, &base, "every", 0.0, 100000.0)?;
        if e <= 0.0 {
            return Err(err(id, &p(&base, "every"), "must be more than 0"));
        }
        period = Some(e);
    }
    if has_turn {
        if ty != JType::Revolute {
            return Err(err(id, &p(&base, "per_turn"), "needs a revolute joint"));
        }
        period = Some(360.0 / int(id, o, &base, "per_turn", 2, 360)?.unwrap_or(2) as f64);
    }
    let out = ps.pin(&p(&base, "out"), text(id, o, &base, "out")?, Pin::Out, "sensor")?;
    let on_ms = num(id, o, &base, "on_ms", 0.0, 60000.0)?.unwrap_or(0.0);
    let off_ms = num(id, o, &base, "off_ms", 0.0, 60000.0)?.unwrap_or(0.0);
    Ok(Sensor { joint, at, window, period, out, on_ms, off_ms, state: false, acc: 0.0 })
}

// ───────────── 프리셋(설계 45의 3.2절, 4.2절) ─────────────

/// 신호 이름을 가진 필드(템플릿 항목의 `optional`이 본다)
const SIG_FIELDS: &[&str] = &["out", "fwd", "back", "pulse", "start", "enable", "busy", "done", "select", "word"];
/// 프리셋 인자 이름과 같은 뜻의 필드 이름(`basis`와 `ask`가 필드 이름으로 본다)
const BASIS_ALIAS: &[(&str, &str)] = &[("speed_mm_s", "speed"), ("per_mm", "per_unit")];

fn canon(x: f64) -> Value {
    if x.fract() == 0.0 && x.abs() < 1e15 {
        json!(x as i64)
    } else {
        json!(x)
    }
}

fn clip(s: &str) -> String {
    s.chars().take(24).collect()
}

/// 인자 이름의 선언 순서(JSON 객체는 이름순으로 정렬되므로 `arg_order`가 순서를 든다)
fn arg_order(pr: &Value) -> Vec<String> {
    match pr["arg_order"].as_array() {
        Some(a) => a.iter().filter_map(|x| x.as_str().map(String::from)).collect(),
        None => pr["args"].as_object().map(|m| m.keys().cloned().collect()).unwrap_or_default(),
    }
}

/// 인자 값을 정한다: 준 값을 검사하고, 안 준 값은 기본값, 기본값이 신호 이름인데 장치에 그 신호가 없으면 없는 것(None)으로 한다.
fn resolve_args(id: &str, o: &Map<String, Value>, pname: &str, pr: &Value, c: &Ctx) -> Result<BTreeMap<String, Option<Value>>, String> {
    let defs = pr["args"].as_object().ok_or_else(|| err(id, "preset", "the preset has no args"))?;
    let order = arg_order(pr);
    let given = match o.get("args").filter(|v| !v.is_null()) {
        None => Map::new(),
        Some(Value::Object(m)) => m.clone(),
        Some(_) => return Err(err(id, "args", "must be an object")),
    };
    if let Some(k) = given.keys().find(|k| !defs.contains_key(*k)) {
        return Err(err(id, &p("args", &clip(k)), &format!("not an arg of preset {pname} (args: {})", order.join(", "))));
    }
    let mut out = BTreeMap::new();
    for name in &order {
        let def = &defs[name];
        let path = p("args", name);
        let ty = def["type"].as_str().unwrap_or("");
        let value = match (ty, given.get(name).filter(|v| !v.is_null())) {
            ("number", Some(x)) => {
                let (lo, hi) = (def["min"].as_f64().unwrap_or(f64::MIN), def["max"].as_f64().unwrap_or(f64::MAX));
                x.as_f64().filter(|n| n.is_finite() && *n >= lo && *n <= hi).ok_or_else(|| err(id, &path, &format!("must be {} to {}", f(lo), f(hi))))?;
                Some(x.clone())
            }
            ("enum", Some(x)) => {
                let vals = def["values"].as_array().cloned().unwrap_or_default();
                if !vals.contains(x) {
                    let w: Vec<String> = vals.iter().map(|y| y.to_string()).collect();
                    return Err(err(id, &path, &format!("must be {}", w.join(" or "))));
                }
                Some(x.clone())
            }
            ("signal", Some(x)) => {
                x.as_str().ok_or_else(|| err(id, &path, "must be a signal name"))?;
                Some(x.clone())
            }
            ("programs", Some(x)) => {
                if !x.is_array() {
                    return Err(err(id, &path, "must be a list of programs"));
                }
                Some(x.clone())
            }
            (_, Some(_)) => return Err(err(id, &path, "has a type that this build does not know")),
            ("signal", None) => def.get("default").and_then(Value::as_str).filter(|n| c.sigs.iter().any(|s| s.name == *n)).map(|n| json!(n)),
            (_, None) => match def.get("default") {
                Some(d) => Some(d.clone()),
                None if def["optional"] == true => None,
                None => return Err(err(id, &path, &format!("needed by preset {pname}"))),
            },
        };
        // 기본값도 없고 optional도 아닌 신호 인자는 꼭 필요하다
        if value.is_none() && ty == "signal" && def.get("default").is_none() && def["optional"] != true {
            return Err(err(id, &path, &format!("needed by preset {pname}")));
        }
        out.insert(name.clone(), value);
    }
    Ok(out)
}

/// 템플릿의 `{"arg": 이름, "mul": m}`을 인자 값으로 바꾼다. 없는 인자의 필드는 없앤다(None). 구동기의 `enable`, `busy`, `done`이
/// 템플릿에 글자로 적혀 있고 장치에 그 신호가 없으면 그 필드도 없앤다. `{"param": ...}`는 그대로 둔다.
fn fill(v: &Value, args: &BTreeMap<String, Option<Value>>, key: Option<&str>, sigs: &[Sig]) -> Option<Value> {
    match v {
        Value::Object(m) if m.contains_key("arg") => {
            let a = args.get(m["arg"].as_str()?)?.as_ref()?;
            match m.get("mul").and_then(Value::as_f64) {
                Some(k) => a.as_f64().map(|x| canon(x * k)),
                None => Some(a.clone()),
            }
        }
        Value::Object(m) => Some(Value::Object(m.iter().filter_map(|(k, x)| fill(x, args, Some(k), sigs).map(|y| (k.clone(), y))).collect())),
        Value::Array(a) => Some(Value::Array(a.iter().filter_map(|x| fill(x, args, None, sigs)).collect())),
        Value::String(s) if matches!(key, Some("enable" | "busy" | "done")) && !sigs.iter().any(|g| g.name == *s) => None,
        _ => Some(v.clone()),
    }
}

/// `"optional": true`인 항목은 이름을 든 신호가 모두 있을 때만 남긴다(키 `optional`은 뺀다)
fn keep_optional(items: &[Value], sigs: &[Sig]) -> Vec<Value> {
    items
        .iter()
        .filter_map(|it| {
            let Some(m) = it.as_object().filter(|m| m.get("optional") == Some(&Value::Bool(true))) else { return Some(it.clone()) };
            let all = SIG_FIELDS.iter().filter_map(|k| m.get(*k).and_then(Value::as_str)).all(|n| sigs.iter().any(|g| g.name == n));
            all.then(|| Value::Object(m.iter().filter(|(k, _)| *k != "optional").map(|(k, x)| (k.clone(), x.clone())).collect()))
        })
        .collect()
}

/// 프리셋을 명시 형태의 `mech`(joints, couplings, drivers, sensors)로 펼친다. 돌려주는 것: 프리셋 이름, 펼친 객체, 인자 이름들.
fn expand(id: &str, o: &Map<String, Value>, c: &Ctx) -> Result<(String, Map<String, Value>, Vec<String>), String> {
    let Some(pv) = o.get("preset").filter(|v| !v.is_null()) else { return Err(err(id, "args", "is only for a preset")) };
    if ["joints", "couplings", "drivers"].iter().any(|k| o.contains_key(*k)) {
        return Err(err(id, "", "give preset or joints, not both"));
    }
    let name = pv.as_str().ok_or_else(|| err(id, "preset", "must be a preset name"))?;
    let presets = c.presets.as_object();
    let ok = |n: &str| presets.and_then(|m| m.get(n)).is_some_and(|pr| pr["kinds"].as_array().is_some_and(|k| k.iter().any(|x| x == c.kind)));
    if !ok(name) {
        let list: Vec<&str> = presets.into_iter().flat_map(|m| m.keys()).map(String::as_str).filter(|n| ok(n)).collect();
        return Err(err(id, "preset", &format!("unknown preset \"{}\" (presets for {}: {})", clip(name), c.kind, if list.is_empty() { "none".to_string() } else { list.join(", ") })));
    }
    let pr = presets.and_then(|m| m.get(name)).cloned().unwrap_or(Value::Null);
    let pr = &pr;
    let args = resolve_args(id, o, name, pr, c)?;
    let tpl = pr["mech"].as_object().ok_or_else(|| err(id, "preset", "the preset has no template"))?;
    let mut ex = Map::new();
    for key in ["joints", "couplings", "drivers"] {
        if let Some(a) = tpl.get(key).and_then(Value::as_array) {
            let items: Vec<Value> = a.iter().filter_map(|x| fill(x, &args, None, c.sigs)).collect();
            ex.insert(key.to_string(), Value::Array(keep_optional(&items, c.sigs)));
        }
    }
    let mut sensors: Vec<Value> = match tpl.get("sensors").and_then(Value::as_array) {
        Some(a) => keep_optional(&a.iter().filter_map(|x| fill(x, &args, None, c.sigs)).collect::<Vec<_>>(), c.sigs),
        None => Vec::new(),
    };
    match o.get("sensors").filter(|v| !v.is_null()) {
        None => {}
        Some(Value::Array(a)) => sensors.extend(a.iter().cloned()),
        Some(_) => return Err(err(id, "sensors", &format!("at most {MAX_SENSORS}"))),
    }
    if !sensors.is_empty() {
        ex.insert("sensors".to_string(), Value::Array(sensors));
    }
    Ok((name.to_string(), ex, arg_order(pr)))
}

/// 장치의 `mech` 값을 읽고 모든 검사를 한다(설계 2절, 3.2절). `preset`은 펼친 뒤 명시 형태와 같은 길로 읽는다.
pub fn parse(id: &str, v: &Value, c: &Ctx) -> Result<Mech, String> {
    let o = v.as_object().ok_or_else(|| err(id, "", "must be an object"))?;
    if v.to_string().len() > MAX_BYTES {
        return Err(err(id, "", &format!("more than {MAX_BYTES} bytes")));
    }
    only(id, o, "", &["preset", "args", "joints", "couplings", "drivers", "sensors", "basis"])?;
    let (mut src, preset, arg_names) = if o.contains_key("preset") || o.contains_key("args") {
        let (name, ex, names) = expand(id, o, c)?;
        (ex, Some(name), names)
    } else {
        (o.clone(), None, Vec::new())
    };
    let expanded = preset.as_ref().map(|_| Value::Object(src.clone()));
    if let Some(b) = o.get("basis") {
        src.insert("basis".to_string(), b.clone());
    }
    let o = match subst(id, "", &Value::Object(src), c.params)? {
        Value::Object(m) => m,
        _ => return Err(err(id, "", "must be an object")),
    };
    let jv = o.get("joints").ok_or_else(|| err(id, "", "give preset or joints"))?;
    let joints = parse_joints(id, c, jv)?;
    let mut ps = Parse { id, c, source: vec![None; joints.len()], joints, unbound: None, writes: Vec::new() };
    let list = |key: &str, max: usize| -> Result<Vec<Value>, String> {
        match o.get(key).filter(|v| !v.is_null()) {
            None => Ok(Vec::new()),
            Some(Value::Array(a)) if a.len() <= max => Ok(a.clone()),
            Some(_) => Err(err(id, key, &format!("at most {max}"))),
        }
    };
    let mut drivers = Vec::new();
    for (i, d) in list("drivers", MAX_ITEMS)?.iter().enumerate() {
        drivers.push(parse_driver(&mut ps, i, d)?);
    }
    let mut couplings = Vec::new();
    for (i, cp) in list("couplings", MAX_ITEMS)?.iter().enumerate() {
        couplings.push(parse_coupling(&mut ps, i, cp)?.0);
    }
    let mut sensors = Vec::new();
    for (i, s) in list("sensors", MAX_SENSORS)?.iter().enumerate() {
        sensors.push(parse_sensor(&mut ps, i, s)?);
    }
    let mut basis = BTreeMap::new();
    match o.get("basis").filter(|v| !v.is_null()) {
        None => {}
        Some(Value::Object(m)) if m.len() <= 32 => {
            for (k, x) in m {
                if !BASIS_KEYS.contains(&k.as_str()) && !arg_names.contains(k) {
                    return Err(err(id, &p("basis", &k.chars().take(24).collect::<String>()), "not a field of mech"));
                }
                let w = x.as_str().filter(|w| BASIS_VALUES.contains(w)).ok_or_else(|| err(id, &p("basis", k), "must be user, label, measured, ref or guess"))?;
                basis.insert(k.clone(), w.to_string());
                // 인자 이름으로 적은 근거는 같은 뜻의 필드 이름으로도 읽힌다(`ask`가 필드 이름으로 본다)
                if let Some((_, field)) = BASIS_ALIAS.iter().find(|(a, _)| a == k) {
                    basis.entry(field.to_string()).or_insert_with(|| w.to_string());
                }
            }
        }
        Some(Value::Object(_)) => return Err(err(id, "basis", "at most 32 keys")),
        Some(_) => return Err(err(id, "basis", "must be an object")),
    }
    let mut given = BTreeSet::new();
    walk_keys(&Value::Object(o.clone()), &mut given);
    let n_points = couplings.iter().map(|c| if let Coupling::Table { pts, .. } = c { pts.len() } else { 0 }).sum();
    let n_segments = drivers.iter().map(|d| if let Driver::Playback { programs, .. } = d { programs.iter().map(|q| q.segs.len()).sum() } else { 0 }).sum();
    Ok(Mech { joints: ps.joints, couplings, drivers, sensors, basis, given, unbound: ps.unbound, writes: ps.writes, n_points, n_segments, preset, expanded })
}

// ───────────── 관절 변환(geom이 틀에 곱한다) ─────────────

/// 관절 값 `v`에서 관절 틀이 영점 틀에 대해 갖는 변환: (자기 x, y, z축을 부모 좌표로, 원점). 형태 관절과 고정 관절은 단위 변환이다.
pub fn motion(j: &Joint, v: V3) -> ([V3; 3], V3) {
    const I: [V3; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    match j.ty {
        JType::Prismatic => {
            let d = v[0] - j.zero;
            (I, [j.axis[0] * d, j.axis[1] * d, j.axis[2] * d])
        }
        JType::Revolute => {
            let th = (v[0] - j.zero).to_radians();
            let (c, s) = (th.cos(), th.sin());
            let a = j.axis;
            // 로드리게스: R e = e c + (a × e) s + a (a · e)(1 - c)
            let r = I.map(|e| {
                let cr = [a[1] * e[2] - a[2] * e[1], a[2] * e[0] - a[0] * e[2], a[0] * e[1] - a[1] * e[0]];
                let dot = a[0] * e[0] + a[1] * e[1] + a[2] * e[2];
                [0, 1, 2].map(|k| e[k] * c + cr[k] * s + a[k] * dot * (1.0 - c))
            });
            (r, [0.0; 3])
        }
        JType::Planar => {
            let (c, s) = (v[2].to_radians().cos(), v[2].to_radians().sin());
            ([[c, 0.0, -s], [0.0, 1.0, 0.0], [s, 0.0, c]], [v[0], 0.0, v[1]])
        }
        _ => (I, [0.0; 3]),
    }
}

// ───────────── 실행 ─────────────

/// 실행 상태가 붙은 기계. `Mech`를 복사해서 만들고 한 걸음마다 상태를 바꾼다.
#[derive(Clone, Debug)]
pub struct Machine {
    pub m: Mech,
    /// 관절 값(풀지 않은 값; 평면 관절은 [x, z, 머리방향도])
    pub vals: Vec<V3>,
    limit_hit: Vec<bool>,
    moved: bool,
}

fn clamp_j(j: &Joint, v: f64) -> f64 {
    match j.lim {
        Some(l) => v.clamp(l[0], l[1]),
        None => v,
    }
}

/// 밸브 규칙: 코일 상태에서 스풀을 정한다. 코일이 둘 다 켜지면 스풀을 그대로 두고 true를 돌려준다. (`plant`의 오래된 장치도 이 한 곳을 부른다.)
pub fn drive(spool: &mut i8, lag_left: &mut f64, v: Valve, cf: bool, cb: bool, lag: f64) -> bool {
    let both = cf && cb;
    let want = if both {
        None
    } else if cf {
        Some(1)
    } else if cb {
        Some(-1)
    } else {
        match v {
            Valve::Hold => None,
            Valve::Spring => Some(-1),
            Valve::Center => Some(0),
        }
    };
    if let Some(s) = want {
        if s != *spool {
            *spool = s;
            *lag_left = if s != 0 { lag } else { 0.0 };
        }
    }
    both
}

/// 값 `v`가 센서의 창 안에 있는가(주기가 있으면 한 바퀴 안의 거리)
fn window_hit(v: f64, s: &Sensor) -> bool {
    let d = match s.period {
        Some(pe) => {
            let d = (v - s.at).rem_euclid(pe);
            d.min(pe - d)
        }
        None => (v - s.at).abs(),
    };
    d <= s.window + EPS
}

impl Machine {
    pub fn new(m: &Mech) -> Machine {
        let vals = m.joints.iter().map(|j| j.start).collect();
        Machine { m: m.clone(), vals, limit_hit: vec![false; m.joints.len()], moved: false }
    }

    /// 시작할 때 한 번: 센서는 시작 값에서의 값을, 구동기의 `busy`와 `done`은 꺼진 값을 쓴다.
    pub fn init(&mut self, pins: &mut dyn Pins) {
        for k in 0..self.m.sensors.len() {
            let raw = window_hit(self.vals[self.m.sensors[k].joint][0], &self.m.sensors[k]);
            let s = &mut self.m.sensors[k];
            s.state = raw;
            s.acc = 0.0;
            pins.set_bit(s.out, raw);
        }
        for d in &self.m.drivers {
            let outs: Vec<usize> = match d {
                // 따라가기 방식(`speed` 없음)은 `busy`를 쓰지 않는다: 그 신호는 모듈 모델이 쓴다(옛 `position`의 규칙)
                Driver::Follow { busy, speed: Some(_), .. } | Driver::Step { busy, .. } => busy.iter().copied().collect(),
                Driver::Follow { .. } => Vec::new(),
                Driver::Playback { busy, done, .. } => busy.iter().chain(done.iter()).copied().collect(),
                Driver::Coils { .. } => Vec::new(),
            };
            for p in outs {
                pins.set_bit(p, false);
            }
        }
    }

    /// `t`에서 `t + dt`(ms)까지 한 걸음. 순서: 구동기, 결합, 한계, 센서, 구동기의 `busy`/`done`(설계 2.8).
    pub fn tick(&mut self, pins: &mut dyn Pins, dt: f64, alarm: &mut dyn FnMut(MAlarm)) {
        let before = self.vals.clone();
        self.tick_core(pins, dt, alarm);
        self.moved = self.vals.iter().zip(&before).any(|(a, b)| (0..3).any(|k| (a[k] - b[k]).abs() > 1e-9));
    }

    /// 마지막 걸음에서 관절 값이 바뀌었는가(`plant.axes.<장치>.moving`)
    pub fn moving(&self) -> bool {
        self.moved
    }

    fn tick_core(&mut self, pins: &mut dyn Pins, dt: f64, alarm: &mut dyn FnMut(MAlarm)) {
        let Machine { m, vals, limit_hit, .. } = self;
        let Mech { joints, couplings, drivers, sensors, .. } = m;
        // (b) 구동기
        for d in drivers.iter_mut() {
            match d {
                Driver::Coils { joint, fwd, back, valve, lag, fwd_ms, back_ms, rate, spool, lag_left, both, both_off } => {
                    let (cf, cb) = (pins.bit(*fwd), back.is_some_and(|b| pins.bit(b)));
                    let j = &joints[*joint];
                    let two = drive(spool, lag_left, valve.unwrap_or(Valve::Center), cf, cb, *lag);
                    if two {
                        if !*both {
                            alarm(MAlarm { kind: "both_coils", key: None });
                            *both = true;
                        }
                        *both_off = 0.0;
                    } else if !cf && !cb {
                        *both_off += dt;
                        if *both_off >= NEVER_MS {
                            *both = false;
                        }
                    } else {
                        *both_off = 0.0;
                    }
                    if *spool != 0 {
                        let used = lag_left.min(dt);
                        *lag_left -= used;
                        let rest = dt - used;
                        if rest > 0.0 {
                            let s = *spool as f64;
                            let v = &mut vals[*joint][0];
                            match (valve, j.lim) {
                                (Some(_), Some(l)) => {
                                    let ms = if *spool > 0 { *fwd_ms } else { *back_ms };
                                    *v = (*v + s * (l[1] - l[0]) / ms * rest).clamp(l[0], l[1]);
                                }
                                _ => *v = clamp_j(j, *v + s * *rate * rest),
                            }
                        }
                    }
                }
                Driver::Follow { joint, word, width, per_unit, speed, enable, busy_now, .. } => {
                    let j = &joints[*joint];
                    let v = &mut vals[*joint][0];
                    if let Some(raw) = pins.word(*word, *width) {
                        let target = clamp_j(j, raw as f64 / *per_unit);
                        let on = enable.is_none_or(|e| pins.bit(e));
                        match speed {
                            None => *v = target,
                            Some(sp) if on => {
                                let step = *sp / 1000.0 * dt;
                                *v = if (target - *v).abs() <= step { target } else { *v + step.copysign(target - *v) };
                            }
                            Some(_) => {}
                        }
                        *busy_now = on && (target - *v).abs() > EPS;
                    }
                }
                Driver::Step { joint, pulse, step, move_ms, lag, prev, moving, target, lag_left, warned, busy_now, .. } => {
                    let p = pins.bit(*pulse);
                    let edge = p && !*prev;
                    *prev = p;
                    if edge {
                        if *moving {
                            if !*warned {
                                alarm(MAlarm { kind: "pulse_while_moving", key: None });
                                *warned = true;
                            }
                        } else {
                            *target += *step;
                            *lag_left = *lag;
                            *moving = true;
                        }
                    }
                    if *moving {
                        let used = lag_left.min(dt);
                        *lag_left -= used;
                        let rest = dt - used;
                        if rest > 0.0 {
                            let d = step.abs() * rest / *move_ms;
                            let v = &mut vals[*joint][0];
                            if (*target - *v).abs() <= d + EPS {
                                *v = *target;
                                *moving = false;
                            } else {
                                *v += d * (*target - *v).signum();
                            }
                        }
                    }
                    *busy_now = *moving;
                }
                Driver::Playback { joints: pj, select, select_name, start, programs, prev, running, cur, seg, e, v0, done_now, warned_busy, warned_bcd, warned_no, .. } => {
                    let s = pins.bit(*start);
                    let edge = s && !*prev;
                    *prev = s;
                    if edge {
                        if *running {
                            if !*warned_busy {
                                alarm(MAlarm { kind: "start_while_busy", key: None });
                                *warned_busy = true;
                            }
                        } else {
                            match pins.word(*select, 16) {
                                None => {
                                    if !*warned_bcd {
                                        alarm(MAlarm { kind: "bad_bcd", key: Some(("signal", json!(select_name))) });
                                        *warned_bcd = true;
                                    }
                                }
                                Some(n) if programs.iter().any(|q| q.no == n) => {
                                    *cur = programs.iter().position(|q| q.no == n).unwrap_or(0);
                                    (*running, *seg, *e, *done_now) = (true, 0, 0.0, false);
                                    *v0 = pj.iter().map(|&j| vals[j][0]).collect();
                                }
                                Some(n) => {
                                    if !warned_no.contains(&n) {
                                        warned_no.push(n);
                                        alarm(MAlarm { kind: "no_program", key: Some(("program", json!(n))) });
                                    }
                                }
                            }
                        }
                    }
                    let mut rest = dt;
                    while *running && rest > 0.0 {
                        let sg = &programs[*cur].segs[*seg];
                        let take = rest.min(sg.ms - *e);
                        *e += take;
                        rest -= take;
                        if let Some(to) = &sg.to {
                            for (k, &j) in pj.iter().enumerate() {
                                vals[j][0] = v0[k] + (to[k] - v0[k]) * *e / sg.ms;
                            }
                        } else if let Some(rate) = &sg.rate {
                            for (k, &j) in pj.iter().enumerate() {
                                vals[j][0] += rate[k] * take / 1000.0;
                            }
                        }
                        if *e >= sg.ms {
                            if let Some(to) = &sg.to {
                                for (k, &j) in pj.iter().enumerate() {
                                    vals[j][0] = to[k];
                                }
                            }
                            *seg += 1;
                            *e = 0.0;
                            *v0 = pj.iter().map(|&j| vals[j][0]).collect();
                            if *seg == programs[*cur].segs.len() {
                                *running = false;
                                *done_now = true;
                            }
                        }
                    }
                }
            }
        }
        // (c) 결합: 목록 순서
        for c in couplings.iter_mut() {
            match c {
                Coupling::Ratio { from, to, k } => vals[*to][0] = joints[*to].zero + *k * (vals[*from][0] - joints[*from].zero),
                Coupling::Table { from, to, pts, period } => {
                    let mut u = vals[*from][0];
                    u = match period {
                        Some(pe) => u.rem_euclid(*pe),
                        None => u.clamp(pts[0][0], pts[pts.len() - 1][0]),
                    };
                    let i = (0..pts.len() - 1).find(|&i| pts[i][0] <= u && u <= pts[i + 1][0]).unwrap_or(pts.len() - 2);
                    let (a, b) = (pts[i], pts[i + 1]);
                    vals[*to][0] = a[1] + (b[1] - a[1]) * (u - a[0]) / (b[0] - a[0]);
                }
                Coupling::Diff { left, right, base, wheel_d, track, last } => {
                    let (l, r) = (vals[*left][0], vals[*right][0]);
                    let mm = std::f64::consts::PI * *wheel_d / 360.0;
                    let (dl, dr) = ((l - last[0]) * mm, (r - last[1]) * mm);
                    *last = [l, r];
                    let ds = (dl + dr) / 2.0;
                    let dh = (dr - dl) / *track;
                    let b = &mut vals[*base];
                    let hm = b[2].to_radians() + dh / 2.0;
                    // 호의 현 길이 / 호 길이: dh가 아주 작아도 상쇄가 없다
                    let fc = if dh == 0.0 { 1.0 } else { (dh / 2.0).sin() / (dh / 2.0) };
                    b[0] += ds * fc * hm.cos();
                    b[1] -= ds * fc * hm.sin();
                    b[2] += dh.to_degrees();
                }
            }
        }
        // (d) 한계: 한계 밖이면 자르고 관절마다 한 번 경보
        for (k, j) in joints.iter().enumerate() {
            let v = &mut vals[k];
            let hit = if j.ty == JType::Planar {
                let (x, z) = (v[0].clamp(j.area[0], j.area[1]), v[1].clamp(j.area[2], j.area[3]));
                let hit = (x - v[0]).abs() > LIMIT_TOL || (z - v[1]).abs() > LIMIT_TOL;
                (v[0], v[1]) = (x, z);
                hit
            } else if let Some(l) = j.lim {
                let c = v[0].clamp(l[0], l[1]);
                let hit = (c - v[0]).abs() > LIMIT_TOL;
                v[0] = c;
                hit
            } else {
                false
            };
            if hit && !limit_hit[k] {
                limit_hit[k] = true;
                alarm(MAlarm { kind: "joint_limit", key: Some(("joint", json!(j.name))) });
            }
        }
        // (e) 창 센서
        for s in sensors.iter_mut() {
            let raw = window_hit(vals[s.joint][0], s);
            if raw != s.state {
                s.acc += dt;
                if s.acc >= if raw { s.on_ms } else { s.off_ms } {
                    (s.state, s.acc) = (raw, 0.0);
                }
            } else {
                s.acc = 0.0;
            }
            pins.set_bit(s.out, s.state);
        }
        // (f) 구동기의 출력
        for d in drivers.iter() {
            match d {
                Driver::Follow { busy: Some(b), speed: Some(_), busy_now, .. } | Driver::Step { busy: Some(b), busy_now, .. } => pins.set_bit(*b, *busy_now),
                Driver::Playback { busy, done, running, done_now, .. } => {
                    if let Some(b) = busy {
                        pins.set_bit(*b, *running);
                    }
                    if let Some(x) = done {
                        pins.set_bit(*x, *done_now);
                    }
                }
                _ => {}
            }
        }
    }

    /// 형태 관절(서보 축의 `carriage`)의 값
    pub fn form_value(&self) -> Option<f64> {
        self.m.joints.iter().position(|j| j.ty == JType::Form).map(|k| self.vals[k][0])
    }

    /// 보고용 관절 값: 고정 관절은 빼고, 평면 관절은 [x, z, 머리방향]
    pub fn joints_json(&self) -> Map<String, Value> {
        let mut o = Map::new();
        for (j, v) in self.m.joints.iter().zip(&self.vals) {
            match j.ty {
                JType::Fixed => {}
                JType::Planar => {
                    o.insert(j.name.clone(), json!([round3(v[0]), round3(v[1]), round3(v[2]).rem_euclid(360.0) + 0.0]));
                }
                _ => {
                    o.insert(j.name.clone(), json!(report(j, v[0])));
                }
            }
        }
        o
    }

    /// 보고하는 값의 개수(결과 크기 상한용)
    pub fn joint_count(&self) -> usize {
        self.m.joints.iter().filter(|j| j.ty != JType::Fixed).count()
    }
}

/// 정적 위치(`geom`이 쓴다): 시작 값에서의 관절 변환
pub fn start_motion(j: &Joint) -> ([V3; 3], V3) {
    motion(j, j.start)
}
