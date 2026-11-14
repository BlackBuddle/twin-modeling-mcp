//! 공작물(마일스톤 B, 형식 2만). 계약 `docs/twin-api.md` 14.12.
//! 위쪽은 장면 키의 읽기와 검사(작업 B1), 아래쪽은 설비 모델(규칙 R1~R9, 부품 기록, 사건; 작업 B2~B4)이다.
//! 재귀가 없고(AD15), 개수는 목록을 만들기 전에 보고, 나눗셈과 색인은 모두 검사한다.

use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

use super::bind::Addresses;
use super::geom::{self, V3};
use super::scene::Dev;

pub const MAX_POINTS: usize = 32;
pub const MAX_TYPES: usize = 8;
pub const MAX_START: usize = 20;
pub const MAX_STACKS: usize = 4;
pub const MAX_CONTENT: usize = 20;
pub const MAX_PUSH_DEV: usize = 4;
pub const MAX_PUSH: usize = 16;
pub const MAX_STOPS: usize = 8;
pub const MAX_SENSE: usize = 32;
/// 한 번에 있는 부품의 한도(시작 때의 적재 + 처음 놓은 부품)
pub const MAX_PARTS_START: usize = 50;
const MAX_POS: f64 = 100_000.0;
/// 색 이름 오류 글에 적는 순서(계약 14.12). `catalog.tones`에서 `mark`를 뺀 것과 같은 집합이다(시험이 확인한다).
pub const TONE_TEXT: &str = "white, black, dark, metal, orange, green, red, blue, yellow, clear";

/// 점 하나. `dev`와 `frame`은 `on`이 가리키는 장치와 틀 번호, `at_world`는 쉬는 자리의 세계 좌표이다.
#[derive(Clone, Debug)]
pub struct Point {
    pub name: String,
    pub on: String,
    pub dev: usize,
    pub frame: usize,
    pub pos: V3,
    pub catch_mm: f64,
    pub holds: u32,
    pub at_world: V3,
}

/// 공작물 종류. `size`는 둥근 것의 지름이거나 네모의 한 변이다.
#[derive(Clone, Debug)]
pub struct WType {
    pub name: String,
    pub round: bool,
    pub size: f64,
    pub h_mm: f64,
    pub metal: bool,
    pub tone: String,
    pub basis: String,
}

impl WType {
    /// 부품 반 폭(둥근 것은 반지름, 네모는 한 변의 반)
    pub fn half(&self) -> f64 {
        self.size / 2.0
    }
}

/// 장치 키 `stack`: 매거진 하나
#[derive(Clone, Debug)]
pub struct Stack {
    pub dev: usize,
    pub point: usize,
    pub fed_by: Option<usize>,
    /// 종류 번호(아래부터)
    pub content: Vec<usize>,
    pub refill: Option<usize>,
}

/// 밀기 규칙: 점 `from`의 부품을 점 `to`로 민다
#[derive(Clone, Copy, Debug)]
pub struct Push {
    pub dev: usize,
    pub from: usize,
    pub to: usize,
}

/// 멈춤 규칙. `belt`가 None이면 벨트 장치 자신의 고정 멈춤이다.
#[derive(Clone, Copy, Debug)]
pub struct Stop {
    pub dev: usize,
    pub belt: Option<usize>,
    pub at: usize,
}

#[derive(Clone, Debug)]
pub enum SenseRule {
    Point { point: usize, within_mm: f64, metal: bool },
    HeldBy(usize),
    Pulse { belt: usize, mm_per_pulse: f64 },
}

/// 서보 축의 위치 단어(`position`)
#[derive(Clone, Debug)]
pub struct Position {
    pub dev: usize,
    pub from: String,
    pub width: u8,
    pub per_mm: f64,
    pub speed_mm_s: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct SenseItem {
    pub dev: usize,
    pub rule: SenseRule,
}

/// 장면의 공작물 설정 전체
#[derive(Clone, Debug, Default)]
pub struct Work {
    pub points: Vec<Point>,
    pub types: Vec<WType>,
    /// (종류 번호, 점 번호)
    pub start: Vec<(usize, usize)>,
    pub stacks: Vec<Stack>,
    pub pushes: Vec<Push>,
    pub stops: Vec<Stop>,
    pub senses: Vec<SenseItem>,
    pub positions: Vec<Position>,
    /// 장면 키 `modules`의 원래 값(작업 B3가 돌린다; 여기서는 검사만 했다)
    pub modules: Option<Value>,
}

impl Work {
    /// 장면 키 `modules`가 모델을 두는 모듈 번호들(없으면 빈 목록). 입력 핀 사건과 감시가 이 번호만 받는다.
    pub fn module_ids(&self, addrs: &dyn Addresses) -> Vec<u8> {
        self.modules.as_ref().and_then(|m| addrs.modules(m).ok()).unwrap_or_default()
    }
}

/// 장치 하나가 가진 새 키를 이름 그대로 읽은 것(점 이름은 장면 전체를 읽은 뒤에 푼다)
#[derive(Debug, Default)]
pub struct RawDev {
    stack: Option<(String, Option<String>, Vec<String>, Option<String>)>,
    push: Vec<(String, String)>,
    stop: Option<(Option<String>, String)>,
    sense: Option<RawSense>,
    position: Option<Position>,
}

#[derive(Debug)]
enum RawSense {
    Point(String, f64, bool),
    Held(String),
    Pulse(String, f64),
}

impl RawDev {
    fn is_empty(&self) -> bool {
        self.stack.is_none() && self.push.is_empty() && self.stop.is_none() && self.sense.is_none() && self.position.is_none()
    }
}

fn dev_err(id: &str, field: &str, msg: &str) -> String {
    format!("device {id}: {field}: {msg}")
}

fn clip(s: &str) -> String {
    s.chars().take(24).collect()
}

fn name_ok(s: &str) -> bool {
    (1..=24).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

fn num_in(v: Option<&Value>, lo: f64, hi: f64) -> Option<f64> {
    v?.as_f64().filter(|n| n.is_finite() && *n >= lo && *n <= hi)
}

fn only_keys(id: &str, field: &str, o: &Map<String, Value>, ok: &[&str]) -> Result<(), String> {
    match o.keys().find(|k| !ok.contains(&k.as_str())) {
        Some(k) => Err(dev_err(id, &format!("{field}.{}", clip(k)), "unknown field")),
        None => Ok(()),
    }
}

/// 장치 한 개의 새 키(`stack`, `push`, `stop`, `sense`, `position`)를 읽는다. 아무 키도 없으면 None.
/// `position.from`(D, W, R, ZR의 워드 장치나 모듈 워드)의 규칙은 호스트의 `Addresses::word_device`이다.
pub fn parse_dev(id: &str, kind: &str, behaviors: &[String], d: &Map<String, Value>, addrs: &dyn Addresses) -> Result<Option<RawDev>, String> {
    let given = |k: &str| d.get(k).filter(|v| !v.is_null());
    let has = |b: &str| behaviors.iter().any(|x| x == b);
    let mut raw = RawDev::default();
    if let Some(v) = given("stack") {
        let o = v.as_object().ok_or_else(|| dev_err(id, "stack", "must be an object"))?;
        only_keys(id, "stack", o, &["point", "fed_by", "content", "refill"])?;
        let point = o.get("point").and_then(Value::as_str).ok_or_else(|| dev_err(id, "stack.point", "must be the name of a point"))?;
        let fed = match o.get("fed_by").filter(|v| !v.is_null()) {
            None => None,
            Some(f) => Some(f.as_str().ok_or_else(|| dev_err(id, "stack.fed_by", "must be the id of a cylinder"))?.to_string()),
        };
        let content = match o.get("content").filter(|v| !v.is_null()) {
            None => Vec::new(),
            Some(c) => {
                let a = c.as_array().filter(|a| a.len() <= MAX_CONTENT).ok_or_else(|| dev_err(id, "stack.content", &format!("at most {MAX_CONTENT} items")))?;
                let mut out = Vec::new();
                for (i, t) in a.iter().enumerate() {
                    out.push(t.as_str().ok_or_else(|| dev_err(id, &format!("stack.content[{i}]"), "must be the name of a workpiece type"))?.to_string());
                }
                out
            }
        };
        let refill = match o.get("refill").filter(|v| !v.is_null()) {
            None => None,
            Some(r) => Some(r.as_str().ok_or_else(|| dev_err(id, "stack.refill", "must be the name of a workpiece type"))?.to_string()),
        };
        raw.stack = Some((point.to_string(), fed, content, refill));
    }
    if let Some(v) = given("push") {
        if !(kind == "cylinder_double" || kind == "cylinder_single") {
            return Err(dev_err(id, "push", "only a cylinder with a valve"));
        }
        let a = v.as_array().filter(|a| (1..=MAX_PUSH_DEV).contains(&a.len())).ok_or_else(|| dev_err(id, "push", &format!("at most {MAX_PUSH_DEV} rules")))?;
        for (i, r) in a.iter().enumerate() {
            let f = format!("push[{i}]");
            let o = r.as_object().ok_or_else(|| dev_err(id, &f, "must be an object"))?;
            only_keys(id, &f, o, &["from", "to"])?;
            let name = |k: &str| o.get(k).and_then(Value::as_str).map(String::from).ok_or_else(|| dev_err(id, &format!("{f}.{k}"), "must be the name of a point"));
            raw.push.push((name("from")?, name("to")?));
        }
    }
    if let Some(v) = given("stop") {
        let o = v.as_object().ok_or_else(|| dev_err(id, "stop", "must be an object"))?;
        only_keys(id, "stop", o, &["belt", "at"])?;
        let at = o.get("at").and_then(Value::as_str).ok_or_else(|| dev_err(id, "stop.at", "must be the name of a point"))?.to_string();
        let belt = if kind == "conveyor" {
            if o.contains_key("belt") {
                return Err(dev_err(id, "stop.belt", "a conveyor stops its own belt (give only at)"));
            }
            None
        } else if has("valve") {
            Some(o.get("belt").and_then(Value::as_str).ok_or_else(|| dev_err(id, "stop.belt", "must be the id of a conveyor"))?.to_string())
        } else {
            return Err(dev_err(id, "stop", "only a conveyor or a cylinder with a valve"));
        };
        raw.stop = Some((belt, at));
    }
    if let Some(v) = given("sense") {
        if !(kind == "photo_sensor" || kind == "prox_sensor") {
            return Err(dev_err(id, "sense", "only a photo or proximity sensor"));
        }
        let o = v.as_object().ok_or_else(|| dev_err(id, "sense", "must be an object"))?;
        only_keys(id, "sense", o, &["of", "point", "within_mm", "material", "held_by", "pulse_of", "mm_per_pulse"])?;
        let forms = ["of", "point", "held_by", "pulse_of"].iter().filter(|k| o.get(**k).is_some_and(|v| !v.is_null())).count();
        if forms != 1 {
            return Err(dev_err(id, "sense", "give exactly one of of, point, held_by, pulse_of"));
        }
        if o.get("of").is_some_and(|v| !v.is_null()) {
            return Err(dev_err(id, "sense.of", "the axis form is planned and not built yet"));
        }
        let text = |k: &str, what: &str| o.get(k).and_then(Value::as_str).map(String::from).ok_or_else(|| dev_err(id, &format!("sense.{k}"), what));
        raw.sense = Some(if o.contains_key("point") {
            let within = num_in(o.get("within_mm"), 1.0, 200.0).ok_or_else(|| dev_err(id, "sense.within_mm", "must be 1 to 200"))?;
            let metal = match o.get("material") {
                None | Some(Value::Null) => false,
                Some(Value::String(s)) if s == "any" => false,
                Some(Value::String(s)) if s == "metal" => true,
                _ => return Err(dev_err(id, "sense.material", "must be any or metal")),
            };
            RawSense::Point(text("point", "must be the name of a point")?, within, metal)
        } else {
            if o.contains_key("within_mm") || o.contains_key("material") {
                return Err(dev_err(id, "sense", "within_mm and material belong to the point form"));
            }
            if o.contains_key("held_by") {
                RawSense::Held(text("held_by", "must be the id of a device that holds")?)
            } else {
                let p = num_in(o.get("mm_per_pulse"), 0.5, 1000.0).ok_or_else(|| dev_err(id, "sense.mm_per_pulse", "must be 0.5 to 1000"))?;
                RawSense::Pulse(text("pulse_of", "must be the id of a conveyor")?, p)
            }
        });
    }
    if let Some(v) = given("position") {
        if kind != "servo_axis" {
            return Err(dev_err(id, "position", "only a servo_axis"));
        }
        let o = v.as_object().ok_or_else(|| dev_err(id, "position", "must be an object"))?;
        only_keys(id, "position", o, &["from", "width", "per_mm", "speed_mm_s"])?;
        // 단어 이름은 호스트가 만든 정규형으로 둔다
        let from = o.get("from").and_then(Value::as_str).and_then(|t| addrs.word_device(t)).ok_or_else(|| dev_err(id, "position.from", "must be a word device (D, W, R, ZR) or a module word U<n>\\G<n>"))?;
        let width = match o.get("width") {
            None | Some(Value::Null) => 16,
            Some(w) => w.as_u64().filter(|w| *w == 16 || *w == 32).ok_or_else(|| dev_err(id, "position.width", "must be 16 or 32"))? as u8,
        };
        let per_mm = match o.get("per_mm").filter(|v| !v.is_null()) {
            None => 1.0,
            Some(p) => num_in(Some(p), 0.0001, 100_000.0).ok_or_else(|| dev_err(id, "position.per_mm", "must be 0.0001 to 100000"))?,
        };
        let speed = match o.get("speed_mm_s").filter(|v| !v.is_null()) {
            None => None,
            Some(s) => Some(num_in(Some(s), 0.1, 5000.0).ok_or_else(|| dev_err(id, "position.speed_mm_s", "must be 0.1 to 5000"))?),
        };
        raw.position = Some(Position { dev: 0, from, width, per_mm, speed_mm_s: speed });
    }
    Ok((!raw.is_empty()).then_some(raw))
}

fn basis_of(o: &Map<String, Value>, path: &str) -> Result<String, String> {
    match o.get("basis").filter(|v| !v.is_null()) {
        None => Ok("guess".into()),
        Some(b) => b.as_str().filter(|b| geom::BASIS_VALUES.contains(b)).map(String::from).ok_or_else(|| format!("{path}.basis: must be user, label, measured, ref or guess")),
    }
}

fn find_point(points: &[Point], name: &str) -> Option<usize> {
    points.iter().position(|p| p.name == name)
}

fn types_list(types: &[WType]) -> String {
    let names: BTreeSet<&str> = types.iter().map(|t| t.name.as_str()).collect();
    names.into_iter().collect::<Vec<_>>().join(", ")
}

fn find_type(types: &[WType], name: &str) -> Option<usize> {
    types.iter().position(|t| t.name == name)
}

/// 장면 키 `points`를 읽는다(개수를 먼저 본다). `on`은 장치 틀로 풀고 쉬는 자리의 세계 좌표를 구한다.
fn parse_points(v: &Value, devices: &[Dev]) -> Result<Vec<Point>, String> {
    let a = v.as_array().filter(|a| a.len() <= MAX_POINTS).ok_or(format!("scene.points: at most {MAX_POINTS} items"))?;
    let ids: Vec<&str> = devices.iter().map(|d| d.id.as_str()).collect();
    let kinds: Vec<&str> = devices.iter().map(|d| d.kind.as_str()).collect();
    let frames: Vec<&[(String, geom::Xf, bool)]> = devices.iter().map(|d| d.geom.as_ref().map_or(&[][..], |g| g.frames.as_slice())).collect();
    let mut out: Vec<Point> = Vec::new();
    for (i, p) in a.iter().enumerate() {
        let f = format!("scene.points[{i}]");
        let o = p.as_object().ok_or(format!("{f}: must be an object"))?;
        if let Some(k) = o.keys().find(|k| !matches!(k.as_str(), "name" | "on" | "pos" | "catch_mm" | "holds" | "basis")) {
            return Err(format!("{f}.{}: unknown field", clip(k)));
        }
        let name = o.get("name").and_then(Value::as_str).filter(|n| name_ok(n)).ok_or(format!("{f}.name: must be 1 to 24 of a-z 0-9 _"))?;
        if find_point(&out, name).is_some() {
            return Err(format!("{f}.name: {name} is used twice"));
        }
        let on = o.get("on").and_then(Value::as_str).ok_or(format!("{f}.on: must be \"<id>\" or \"<id>.<frame>\""))?;
        let (dev, frame) = geom::locate(&ids, &kinds, &frames, on).map_err(|m| format!("{f}.on: {m}"))?;
        let pos = o.get("pos").and_then(|v| geom::vec3(v, MAX_POS)).ok_or(format!("{f}.pos: must be 3 numbers from -{MAX_POS} to {MAX_POS}"))?;
        let catch_mm = match o.get("catch_mm").filter(|v| !v.is_null()) {
            None => 2.0,
            Some(c) => num_in(Some(c), 0.0, 200.0).ok_or(format!("{f}.catch_mm: must be 0 to 200"))?,
        };
        let holds = match o.get("holds").filter(|v| !v.is_null()) {
            None => 1,
            Some(h) => h.as_u64().filter(|h| (1..=50).contains(h)).ok_or(format!("{f}.holds: must be 1 to 50"))? as u32,
        };
        basis_of(o, &f)?; // 점의 출처 낱말은 검사만 한다 (모델은 읽지 않는다)
        let g = devices[dev].geom.as_ref().ok_or(format!("{f}.on: the scene is not schema 2"))?;
        let at_world = g.world.mul(&g.frames[frame].1).pt(pos);
        out.push(Point { name: name.into(), on: on.into(), dev, frame, pos, catch_mm, holds, at_world });
    }
    Ok(out)
}

/// 장면 키 `workpieces`: (종류, 시작 부품의 원래 이름 쌍). 시작 부품은 점을 읽은 뒤에 푼다.
fn parse_workpieces(v: &Value) -> Result<(Vec<WType>, Vec<(String, String)>), String> {
    let o = v.as_object().ok_or("scene.workpieces: must be an object")?;
    if let Some(k) = o.keys().find(|k| !matches!(k.as_str(), "types" | "start")) {
        return Err(format!("scene.workpieces.{}: unknown field", clip(k)));
    }
    let tm = o.get("types").and_then(Value::as_object).ok_or("scene.workpieces.types: needs 1 to 8 types")?;
    if tm.len() > MAX_TYPES {
        return Err(format!("scene.workpieces.types: at most {MAX_TYPES}"));
    }
    if tm.is_empty() {
        return Err("scene.workpieces.types: needs 1 to 8 types".into());
    }
    let tones: Vec<&str> = super::scene::catalog()["tones"].as_array().into_iter().flatten().filter_map(Value::as_str).filter(|t| *t != "mark").collect();
    let mut types = Vec::new();
    for (name, t) in tm {
        if !name_ok(name) {
            return Err(format!("scene.workpieces.types: the name \"{}\" must be 1 to 24 of a-z 0-9 _", clip(name)));
        }
        let f = format!("scene.workpieces.types.{name}");
        let t = t.as_object().ok_or(format!("{f}: must be an object"))?;
        if let Some(k) = t.keys().find(|k| !matches!(k.as_str(), "shape" | "d_mm" | "side_mm" | "h_mm" | "material" | "tone" | "basis")) {
            return Err(format!("{f}.{}: unknown field", clip(k)));
        }
        let round = match t.get("shape").and_then(Value::as_str) {
            Some("round") => true,
            Some("square") => false,
            _ => return Err(format!("{f}.shape: must be round or square")),
        };
        let (key, other, article) = if round { ("d_mm", "side_mm", "a round") } else { ("side_mm", "d_mm", "a square") };
        let size = num_in(t.get(key), 2.0, 500.0).ok_or(format!("{f}.{key}: {article} part needs {key} (2 to 500)"))?;
        if t.get(other).is_some_and(|v| !v.is_null()) {
            return Err(format!("{f}.{other}: {article} part has no {other}"));
        }
        let h_mm = num_in(t.get("h_mm"), 1.0, 500.0).ok_or(format!("{f}.h_mm: must be 1 to 500"))?;
        let metal = match t.get("material").and_then(Value::as_str) {
            Some("metal") => true,
            Some("plastic") => false,
            _ => return Err(format!("{f}.material: must be metal or plastic")),
        };
        let tone = match t.get("tone").filter(|v| !v.is_null()) {
            None => if metal { "metal" } else { "white" }.to_string(),
            Some(x) => x.as_str().filter(|x| tones.contains(x)).map(String::from).ok_or(format!("{f}.tone: must be one of {TONE_TEXT}"))?,
        };
        types.push(WType { name: name.clone(), round, size, h_mm, metal, tone, basis: basis_of(t, &f)? });
    }
    let mut start = Vec::new();
    if let Some(s) = o.get("start").filter(|v| !v.is_null()) {
        let a = s.as_array().filter(|a| a.len() <= MAX_START).ok_or(format!("scene.workpieces.start: at most {MAX_START} items"))?;
        for (i, it) in a.iter().enumerate() {
            let f = format!("scene.workpieces.start[{i}]");
            let it = it.as_object().ok_or(format!("{f}: must be an object"))?;
            if let Some(k) = it.keys().find(|k| !matches!(k.as_str(), "type" | "at")) {
                return Err(format!("{f}.{}: unknown field", clip(k)));
            }
            let g = |k: &str| it.get(k).and_then(Value::as_str).map(String::from).ok_or(format!("{f}.{k}: must be text"));
            start.push((g("type")?, g("at")?));
        }
    }
    Ok((types, start))
}

/// 모든 장치를 읽고 `on`을 푼 뒤에 부른다. 새 키가 하나도 없으면 None이다(옛 장면은 그대로).
pub fn build(o: &Map<String, Value>, devices: &[Dev], raws: Vec<(usize, RawDev)>, addrs: &dyn Addresses) -> Result<Option<Work>, String> {
    let given = |k: &str| o.get(k).filter(|v| !v.is_null());
    if raws.is_empty() && given("points").is_none() && given("workpieces").is_none() && given("modules").is_none() {
        return Ok(None);
    }
    let mut w = Work::default();
    if let Some(p) = given("points") {
        w.points = parse_points(p, devices)?;
    }
    let mut raw_start = Vec::new();
    if let Some(v) = given("workpieces") {
        (w.types, raw_start) = parse_workpieces(v)?;
    }
    if let Some(m) = given("modules") {
        addrs.modules(m).map_err(|e| format!("scene.modules: {}", e.strip_prefix("modules: ").unwrap_or(&e)))?;
        w.modules = Some(m.clone());
    }
    let no_types = |f: &str, t: &str| format!("{f}: no workpiece type \"{}\" (types: {})", clip(t), types_list(&w.types));
    for (i, (t, at)) in raw_start.iter().enumerate() {
        let f = format!("scene.workpieces.start[{i}]");
        let ti = find_type(&w.types, t).ok_or_else(|| no_types(&format!("{f}.type"), t))?;
        let pi = find_point(&w.points, at).ok_or(format!("{f}.at: no point \"{}\"", clip(at)))?;
        w.start.push((ti, pi));
    }
    // 개수는 이름을 풀기 전에 센다
    if raws.iter().map(|(_, r)| r.push.len()).sum::<usize>() > MAX_PUSH {
        return Err(format!("scene: more than {MAX_PUSH} push rules"));
    }
    if raws.iter().filter(|(_, r)| r.stop.is_some()).count() > MAX_STOPS {
        return Err(format!("scene: more than {MAX_STOPS} stops"));
    }
    if raws.iter().filter(|(_, r)| r.sense.is_some()).count() > MAX_SENSE {
        return Err(format!("scene: more than {MAX_SENSE} sensor rules"));
    }
    if raws.iter().filter(|(_, r)| r.stack.is_some()).count() > MAX_STACKS {
        return Err(format!("scene: more than {MAX_STACKS} stacks"));
    }
    let dev_of = |id: &str| devices.iter().position(|d| d.id == id);
    for (di, r) in &raws {
        let id = devices[*di].id.as_str();
        for (i, (from, to)) in r.push.iter().enumerate() {
            let pt = |k: &str, n: &str| find_point(&w.points, n).ok_or_else(|| dev_err(id, &format!("push[{i}].{k}"), &format!("no point \"{}\"", clip(n))));
            let (f, t) = (pt("from", from)?, pt("to", to)?);
            if f == t {
                return Err(dev_err(id, &format!("push[{i}]"), "from and to are the same point"));
            }
            w.pushes.push(Push { dev: *di, from: f, to: t });
        }
        if let Some((belt, at)) = &r.stop {
            let a = find_point(&w.points, at).ok_or_else(|| dev_err(id, "stop.at", &format!("no point \"{}\"", clip(at))))?;
            let b = match belt {
                None => None,
                Some(b) => Some(dev_of(b).filter(|i| devices[*i].kind == "conveyor").ok_or_else(|| dev_err(id, "stop.belt", &format!("{} is not a conveyor of the scene", clip(b))))?),
            };
            w.stops.push(Stop { dev: *di, belt: b, at: a });
        }
        if let Some(s) = &r.sense {
            let rule = match s {
                RawSense::Point(p, within_mm, metal) => SenseRule::Point { point: find_point(&w.points, p).ok_or_else(|| dev_err(id, "sense.point", &format!("no point \"{}\"", clip(p))))?, within_mm: *within_mm, metal: *metal },
                RawSense::Held(h) => SenseRule::HeldBy(dev_of(h).filter(|i| devices[*i].has("hold")).ok_or_else(|| dev_err(id, "sense.held_by", &format!("{} is not a device that holds a part", clip(h))))?),
                RawSense::Pulse(b, p) => SenseRule::Pulse { belt: dev_of(b).filter(|i| devices[*i].kind == "conveyor").ok_or_else(|| dev_err(id, "sense.pulse_of", &format!("{} is not a conveyor of the scene", clip(b))))?, mm_per_pulse: *p },
            };
            w.senses.push(SenseItem { dev: *di, rule });
        }
        if let Some(p) = &r.position {
            w.positions.push(Position { dev: *di, ..p.clone() });
        }
    }
    // 매거진은 밀기 규칙을 모두 푼 뒤에 본다(`fed_by`가 그 규칙을 찾는다)
    let mut parts = w.start.len();
    for (di, r) in &raws {
        let Some((point, fed, content, refill)) = &r.stack else { continue };
        let id = devices[*di].id.as_str();
        let p = find_point(&w.points, point).ok_or_else(|| dev_err(id, "stack.point", &format!("no point \"{}\"", clip(point))))?;
        let fed_by = match fed {
            None => None,
            Some(f) => {
                let fi = dev_of(f).ok_or_else(|| dev_err(id, "stack.fed_by", &format!("no device \"{}\"", clip(f))))?;
                if !w.pushes.iter().any(|x| x.dev == fi && x.from == p) {
                    return Err(dev_err(id, "stack.fed_by", &format!("{} has no push rule from {point}", devices[fi].id)));
                }
                Some(fi)
            }
        };
        let mut idx = Vec::new();
        for (i, t) in content.iter().enumerate() {
            idx.push(find_type(&w.types, t).ok_or_else(|| dev_err(id, &format!("stack.content[{i}]"), &format!("no workpiece type \"{}\" (types: {})", clip(t), types_list(&w.types))))?);
        }
        let refill = match refill {
            Some(t) => Some(find_type(&w.types, t).ok_or_else(|| dev_err(id, "stack.refill", &format!("no workpiece type \"{}\" (types: {})", clip(t), types_list(&w.types))))?),
            // 기본: 내용의 마지막 종류, 없으면 이름 순으로 첫 종류
            None => idx.last().copied().or_else(|| w.types.iter().enumerate().min_by(|a, b| a.1.name.cmp(&b.1.name)).map(|(i, _)| i)),
        };
        parts += idx.len();
        w.stacks.push(Stack { dev: *di, point: p, fed_by, content: idx, refill });
    }
    if parts > MAX_PARTS_START {
        return Err(format!("scene: more than {MAX_PARTS_START} parts at the start"));
    }
    Ok(Some(w))
}

fn half_xz(a: V3, b: V3) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// `facts.points`, `facts.workpieces`, `facts.push`(쓸 것이 없으면 None)와 `ask` 줄, `notes` 줄
pub struct WorkFacts {
    pub points: Value,
    pub workpieces: Value,
    pub push: Value,
    pub ask: Vec<String>,
    pub notes: Vec<String>,
}

pub fn facts(devices: &[Dev], w: &Work) -> WorkFacts {
    let points: Vec<Value> = w.points.iter().map(|p| json!({"name": p.name, "on": p.on, "at_world": [geom::n(p.at_world[0]), geom::n(p.at_world[1]), geom::n(p.at_world[2])]})).collect();
    let mut sorted: Vec<&WType> = w.types.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    let types: Vec<Value> = sorted.iter().map(|t| {
        let mut j = json!({"name": t.name, "shape": if t.round { "round" } else { "square" }});
        j[if t.round { "d_mm" } else { "side_mm" }] = geom::n(t.size);
        j["h_mm"] = geom::n(t.h_mm);
        j["material"] = json!(if t.metal { "metal" } else { "plastic" });
        j["tone"] = json!(t.tone);
        j["basis"] = json!(t.basis);
        j
    }).collect();
    let stacks: BTreeMap<&str, usize> = w.stacks.iter().map(|s| (devices[s.dev].id.as_str(), s.content.len())).collect();
    let at_start = w.start.len() + w.stacks.iter().map(|s| s.content.len()).sum::<usize>();
    let mut ask = Vec::new();
    for t in &sorted {
        if !matches!(t.basis.as_str(), "user" | "label" | "measured") {
            ask.push(format!("workpieces.types.{}.material: a photo does not show metal; ask the person", t.name));
        }
        if matches!(t.basis.as_str(), "guess" | "ref") {
            ask.push(format!("workpieces.types.{}: sizes from a photo are guesses until one puck is measured", t.name));
        }
    }
    // 밀기: 장치 id 순, 각 장치의 목록 순. 부품은 이름 순 첫 종류를 점 자리에 둔 것으로 잡는다.
    let half = sorted.first().map_or(0.0, |t| t.half());
    let mut order: Vec<&Push> = w.pushes.iter().collect();
    order.sort_by(|a, b| devices[a.dev].id.cmp(&devices[b.dev].id));
    let (mut push, mut notes) = (Vec::new(), Vec::new());
    for p in order {
        let d = &devices[p.dev];
        let Some(g) = d.geom.as_ref() else { continue };
        let Some(ax) = &g.axis else { continue };
        let tip = |a: f64| g.world.pt(geom::moving_pos(&d.kind, &g.dims, ax, a));
        let (t0, u) = (tip(0.0), g.world.dir([1.0, 0.0, 0.0]));
        let (from, to) = (&w.points[p.from], &w.points[p.to]);
        let stroke = ax.range[1] - ax.range[0];
        let rel = [from.at_world[0] - t0[0], from.at_world[1] - t0[1], from.at_world[2] - t0[2]];
        let contact = (rel[0] * u[0] + rel[1] * u[1] + rel[2] * u[2] - half).clamp(0.0, stroke);
        // 막대가 끝까지 나왔을 때 부품 자리와 `to`의 수평 거리
        let end = [from.at_world[0] + u[0] * (stroke - contact), from.at_world[1], from.at_world[2] + u[2] * (stroke - contact)];
        let gap = half_xz(end, to.at_world);
        push.push(json!({"device": d.id, "from": from.name, "to": to.name, "contact_mm": geom::n(contact), "end_gap_mm": geom::n(gap)}));
        if geom::n(gap).as_f64().unwrap_or(0.0) > 10.0 {
            notes.push(format!("{}: push from {} to {} ends {} mm from {}", d.id, from.name, to.name, geom::n(gap), to.name));
        }
    }
    WorkFacts {
        points: json!(points),
        workpieces: json!({"types": types, "stacks": stacks, "parts_at_start": at_start}),
        push: json!(push),
        ask,
        notes,
    }
}

// ───────────── 실행(작업 B2): 규칙 R1~R9, 설계 43의 2절 ─────────────

pub const MAX_LOG: usize = 500;
/// 한 번의 시뮬레이션이 가진 부품 수의 상한(`plant::MAX_PARTS`와 같은 값)
const MAX_PARTS: usize = super::plant::MAX_PARTS;
const EPS: f64 = 1e-9;
const NONE: usize = usize::MAX;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PState {
    Stack,
    Rest,
    Belt,
    Ride,
    Held,
    Left,
}

impl PState {
    pub fn name(self) -> &'static str {
        match self {
            PState::Stack => "stack",
            PState::Rest => "rest",
            PState::Belt => "belt",
            PState::Ride => "ride",
            PState::Held => "held",
            PState::Left => "left",
        }
    }
}

#[derive(Clone, Debug)]
pub struct WPart {
    pub id: u32,
    pub ty: usize,
    /// 이 부품만의 색(없으면 종류의 색)
    pub tone: Option<String>,
    pub state: PState,
    /// 점 번호(`NONE`이면 점에 있지 않다)
    pub at: usize,
    /// 세계 좌표의 바닥 중심
    pub p: V3,
    /// 벨트 장치나 밀기·잡기 장치의 번호
    carrier: usize,
    off: V3,
    /// 벨트 위 좌표(벨트 `surface`의 +x)
    s: f64,
    /// 밀기 규칙 번호
    rule: usize,
    /// 그 점에 몇 번째로 왔는가(1부터; 쌓임을 그리는 화면이 쓴다)
    pub arrival: u32,
}

/// 시나리오 항목 한 개(`part_events`와 화면 길이 같이 쓴다)
#[derive(Clone, Debug)]
pub enum Act {
    Source { source: String, ty: Option<String>, tone: Option<String> },
    Put { at: String, ty: Option<String>, tone: Option<String>, s_mm: Option<f64> },
    PutPos { pos: V3, ty: Option<String>, tone: Option<String> },
    Remove(String),
    Reset,
    /// 화면: 부품 하나를 옮긴다(번호는 그대로), 색만 바꾼다, 치운다
    Move { part: u32, at: String, s_mm: Option<f64> },
    Tone { part: u32, tone: String },
    RemovePart(u32),
}

#[derive(Clone, Copy)]
enum Target {
    Point(usize),
    Stack(usize),
    Pad(usize),
    Belt(usize),
}

pub struct Rt {
    pub w: Work,
    ids: Vec<String>,
    kinds: Vec<String>,
    geoms: Vec<geom::DevGeom>,
    order: Vec<usize>,
    /// 장치마다 축 값(원통과 대리 축은 mm, 벨트는 이동 거리)과 이번 tick의 변화
    pub vals: Vec<f64>,
    pub deltas: Vec<f64>,
    ws: Vec<geom::Xf>,
    /// `mech`가 있는 장치의 관절 틀의 지금 자리(`geom::joint_frames_now`; 없는 장치는 빈 목록)
    jx: Vec<Vec<geom::Xf>>,
    pts: Vec<V3>,
    win: Vec<f64>,
    belts: Vec<usize>,
    surface: Vec<usize>,
    pads: Vec<(usize, Option<String>, f64)>,
    /// 밀기 장치(id 순)와 그 규칙 번호
    pushers: Vec<(usize, Vec<usize>)>,
    pts_sorted: Vec<usize>,
    sense_order: Vec<usize>,
    sense_out: Vec<Option<String>>,
    pub stuck: Vec<Option<bool>>,
    forced: Vec<bool>,
    /// 시작 내용(종류 번호, 부품만의 색)과 그 매거진이 `t = 0` 항목으로 바뀌었는가
    start: Vec<Vec<(usize, Option<String>)>>,
    start_set: Vec<bool>,
    waiting: Vec<Vec<u32>>,
    arrivals: Vec<u32>,
    pub parts: Vec<WPart>,
    next_id: u32,
    pub parts_dropped: u64,
    pub log: Vec<Value>,
    pub log_truncated: bool,
    old_p: Vec<V3>,
    t: f64,
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dist3(a: V3, b: V3) -> f64 {
    let d = sub(a, b);
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

fn dist_xz(a: V3, b: V3) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// 선분 a→b 위에서 p에 가장 가까운 점까지의 거리(a와 b가 같으면 점 거리)
fn seg_dist(a: V3, b: V3, p: V3) -> f64 {
    let ab = sub(b, a);
    let l2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
    let k = if l2 < EPS { 0.0 } else { ((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1] + (p[2] - a[2]) * ab[2]) / l2 };
    let k = k.clamp(0.0, 1.0);
    dist3([a[0] + ab[0] * k, a[1] + ab[1] * k, a[2] + ab[2] * k], p)
}

impl Rt {
    /// 장면에 공작물 키가 있을 때만 만든다. 부품은 `begin`이 만든다.
    pub fn new(scene: &super::scene::Scene) -> Option<Rt> {
        let w = scene.work.clone()?;
        let n = scene.devices.len();
        let geoms: Vec<geom::DevGeom> = scene.devices.iter().map(|d| d.geom.clone()).collect::<Option<_>>()?;
        let ids: Vec<String> = scene.devices.iter().map(|d| d.id.clone()).collect();
        let kinds: Vec<String> = scene.devices.iter().map(|d| d.kind.clone()).collect();
        let mut by_id: Vec<usize> = (0..n).collect();
        by_id.sort_by(|a, b| ids[*a].cmp(&ids[*b]));
        let order = geom::base_order(&geoms);
        let belts: Vec<usize> = by_id.iter().copied().filter(|&i| kinds[i] == "conveyor").collect();
        let mut surface = vec![0; n];
        for &b in &belts {
            surface[b] = geoms[b].frames.iter().position(|f| f.0 == "surface").unwrap_or(0);
        }
        let pads = by_id.iter().copied().filter(|&i| scene.devices[i].has("hold")).map(|i| (i, scene.devices[i].slot("coil").or_else(|| scene.devices[i].slot("coil_fwd")).map(String::from), scene.devices[i].num("reach_mm"))).collect();
        let pushers = by_id.iter().copied().map(|i| (i, (0..w.pushes.len()).filter(|&k| w.pushes[k].dev == i).collect::<Vec<_>>())).filter(|(_, r)| !r.is_empty()).collect();
        let mut pts_sorted: Vec<usize> = (0..w.points.len()).collect();
        pts_sorted.sort_by(|a, b| w.points[*a].name.cmp(&w.points[*b].name));
        let mut sense_order: Vec<usize> = (0..w.senses.len()).collect();
        sense_order.sort_by(|a, b| ids[w.senses[*a].dev].cmp(&ids[w.senses[*b].dev]));
        let sense_out = w.senses.iter().map(|s| scene.devices[s.dev].slot("output").map(String::from)).collect();
        let mut vals = vec![0.0; n];
        // 서보 축의 값은 플랜트의 엔진이 `vals`에 넘긴다(짧은 형태 `position`도 `follow` 구동기이다); 시작 값만 여기서 둔다
        for p in &w.positions {
            if let Some(a) = geoms[p.dev].axis.as_ref() {
                vals[p.dev] = a.range[0];
            }
        }
        let start = w.stacks.iter().map(|s| s.content.iter().map(|&t| (t, None)).collect()).collect();
        Some(Rt {
            ws: vec![geom::IDENT; n],
            jx: vec![Vec::new(); n],
            pts: vec![[0.0; 3]; w.points.len()],
            win: scene.devices.iter().map(|d| d.num("window_mm")).collect(),
            start_set: vec![false; w.stacks.len()],
            waiting: vec![Vec::new(); w.stacks.len()],
            arrivals: vec![0; w.points.len()],
            stuck: vec![None; w.senses.len()],
            forced: vec![false; w.senses.len()],
            deltas: vec![0.0; n],
            parts: Vec::with_capacity(MAX_PARTS),
            old_p: Vec::with_capacity(MAX_PARTS),
            w,
            ids,
            kinds,
            geoms,
            order,
            vals,
            belts,
            surface,
            pads,
            pushers,
            pts_sorted,
            sense_order,
            sense_out,
            start,
            next_id: 1,
            parts_dropped: 0,
            log: Vec::new(),
            log_truncated: false,
            t: 0.0,
        })
    }

    pub fn dev_index(&self, id: &str) -> Option<usize> {
        self.ids.iter().position(|x| x == id)
    }

    /// 세계 변환과 점의 세계 좌표를 지금 축 값으로 다시 구한다
    fn refresh(&mut self) {
        geom::worlds_at(&self.kinds, &self.geoms, &self.order, &self.vals, &self.jx, &mut self.ws);
        for (i, p) in self.w.points.iter().enumerate() {
            let g = &self.geoms[p.dev];
            self.pts[i] = self.ws[p.dev].mul(&geom::frame_now(&self.kinds[p.dev], g, p.frame, self.vals[p.dev], &self.jx[p.dev])).pt(p.pos);
        }
    }

    /// 장치 `dev`의 관절 틀을 엔진의 지금 관절 값으로 놓는다. 플랜트가 엔진을 한 걸음 돌린 뒤, 규칙이 돌기 전에 부른다.
    pub fn set_joints(&mut self, dev: usize, m: &super::mech::Machine) {
        self.jx[dev] = geom::joint_frames_now(&self.kinds[dev], &self.geoms[dev], &m.m, &m.vals);
    }

    fn ev(&mut self, kind: &str, part: Option<u32>, extra: Value) {
        if self.log.len() >= MAX_LOG {
            self.log_truncated = true;
            return;
        }
        let mut e = json!({"t": self.t.round() as i64, "kind": kind, "part": part});
        if let (Some(m), Some(x)) = (e.as_object_mut(), extra.as_object()) {
            m.extend(x.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
        self.log.push(e);
    }

    fn pi_of(&self, id: u32) -> Option<usize> {
        self.parts.iter().position(|p| p.id == id)
    }

    fn half(&self, p: &WPart) -> f64 {
        self.w.types[p.ty].half()
    }

    /// 새 부품 하나(상한을 넘으면 만들지 않고 센다)
    fn make(&mut self, ty: usize, tone: Option<String>) -> Option<usize> {
        if self.parts.len() >= MAX_PARTS {
            self.parts_dropped += 1;
            return None;
        }
        self.parts.push(WPart { id: self.next_id, ty, tone, state: PState::Stack, at: NONE, p: [0.0; 3], carrier: NONE, off: [0.0; 3], s: 0.0, rule: NONE, arrival: 0 });
        self.next_id += 1;
        Some(self.parts.len() - 1)
    }

    /// 지금 자리가 벨트 위인가: 길이와 너비 안이고 면에서 2 mm 안이면 (벨트 장치 번호, 벨트 좌표)
    fn on_belt(&self, p: V3) -> Option<(usize, f64)> {
        for &b in &self.belts {
            let (g, surf) = (&self.geoms[b], self.ws[b].mul(&self.geoms[b].frames[self.surface[b]].1));
            let loc = surf.inv().pt(p);
            if loc[0].abs() <= g.dims.v("length_mm") / 2.0 + EPS && loc[2].abs() <= g.dims.v("width_mm") / 2.0 + EPS && loc[1].abs() <= 2.0 + EPS {
                return Some((b, loc[0]));
            }
        }
        None
    }

    /// 부품을 점에 둔다(벨트 위의 점이면 벨트 위 상태). 점이 가진 수를 넘으면 `full`을 적는다.
    fn place(&mut self, pi: usize, pt: usize) {
        let p = self.pts[pt];
        let belt = self.on_belt(p);
        self.arrivals[pt] += 1;
        let arrival = self.arrivals[pt];
        let q = &mut self.parts[pi];
        (q.p, q.at, q.arrival, q.rule) = (p, pt, arrival, NONE);
        match belt {
            Some((b, s)) => (q.state, q.carrier, q.s) = (PState::Belt, b, s),
            None => (q.state, q.carrier) = (PState::Rest, NONE),
        }
        let n = self.parts.iter().enumerate().filter(|(j, q)| *j != pi && q.at == pt && matches!(q.state, PState::Rest | PState::Belt) && dist3(q.p, p) < 1.0).count();
        if n + 1 > self.w.points[pt].holds as usize {
            let id = self.parts[pi].id;
            let name = self.w.points[pt].name.clone();
            self.ev("full", Some(id), json!({"point": name}));
        }
    }

    /// 가장 높은 점(`ok`가 거른 것 중), 같으면 이름이 앞선 것
    fn highest(&self, ok: impl Fn(V3, usize) -> bool) -> Option<usize> {
        let mut best: Option<usize> = None;
        for &k in &self.pts_sorted {
            if ok(self.pts[k], k) && best.is_none_or(|b| self.pts[k][1] > self.pts[b][1]) {
                best = Some(k);
            }
        }
        best
    }

    /// R8: 점이 비었고 `fed_by`가 뒤에 있으면 가장 아래 부품이 점으로 떨어진다. 기다리는 부품은 점 위의 기둥에 선다.
    fn drop_stack(&mut self, si: usize, force: bool) {
        if self.waiting[si].is_empty() {
            return;
        }
        let st = self.w.stacks[si].clone();
        let here = self.pts[st.point];
        let catch = self.w.points[st.point].catch_mm;
        let busy = self.parts.iter().any(|q| q.state != PState::Stack && dist3(q.p, here) <= catch);
        let back = st.fed_by.is_none_or(|f| self.vals[f] <= self.win[f] + EPS);
        if !busy && (back || force) {
            let id = self.waiting[si].remove(0);
            if let Some(pi) = self.pi_of(id) {
                self.place(pi, st.point);
                let (stack, point) = (self.ids[st.dev].clone(), self.w.points[st.point].name.clone());
                self.ev("drop", Some(id), json!({"stack": stack, "point": point}));
                if self.waiting[si].is_empty() {
                    self.ev("empty", None, json!({"stack": stack}));
                }
            }
        }
        for k in 0..self.waiting[si].len() {
            if let Some(pi) = self.pi_of(self.waiting[si][k]) {
                let h = self.w.types[self.parts[pi].ty].h_mm;
                self.parts[pi].p = [here[0], here[1] + (k as f64 + 1.0) * h, here[2]];
            }
        }
    }

    /// 시작 상태를 만든다: 매거진 내용, 시작 부품, 처음 떨어짐(힘으로). 시작 때와 처음으로 돌릴 때 쓴다.
    fn build_start(&mut self) {
        self.refresh();
        for si in 0..self.w.stacks.len() {
            for (ty, tone) in self.start[si].clone() {
                if let Some(pi) = self.make(ty, tone) {
                    self.waiting[si].push(self.parts[pi].id);
                }
            }
        }
        for (ty, pt) in self.w.start.clone() {
            if let Some(pi) = self.make(ty, None) {
                self.place(pi, pt);
            }
        }
        for si in 0..self.w.stacks.len() {
            self.drop_stack(si, true);
        }
    }

    /// 시작 내용에 한 개를 더한다(`t = 0`의 매거진 항목: 그 매거진의 첫 항목이 장면의 내용을 바꾼다)
    pub fn start_item(&mut self, stack: usize, ty: usize, tone: Option<String>) {
        if !self.start_set[stack] {
            self.start[stack].clear();
            self.start_set[stack] = true;
        }
        self.start[stack].push((ty, tone));
    }

    pub fn begin(&mut self, io: &mut dyn super::plant::PlantIo) {
        self.build_start();
        self.sense_now(io);
    }

    // ── 사건 ──

    pub fn type_index(&self, t: Option<&str>, default: usize) -> Result<usize, String> {
        match t {
            None => Ok(default),
            Some(n) => self.w.types.iter().position(|x| x.name == n).ok_or_else(|| format!("type {} is not a workpiece type of the scene (types: {})", clip(n), types_list(&self.w.types))),
        }
    }

    fn first_type(&self) -> usize {
        self.w.types.iter().enumerate().min_by(|a, b| a.1.name.cmp(&b.1.name)).map_or(0, |x| x.0)
    }

    pub fn stack_of(&self, id: &str) -> Option<usize> {
        self.w.stacks.iter().position(|s| self.ids[s.dev] == id)
    }

    /// 매거진 `si`에서 `type`을 안 적었을 때의 종류(`refill`, 없으면 이름순 첫 종류)
    pub fn refill_type(&self, si: usize) -> usize {
        self.w.stacks[si].refill.unwrap_or_else(|| self.first_type())
    }

    fn target(&self, x: &str) -> Option<Target> {
        if let Some(k) = find_point(&self.w.points, x) {
            return Some(Target::Point(k));
        }
        if let Some(k) = self.stack_of(x) {
            return Some(Target::Stack(k));
        }
        let d = self.dev_index(x)?;
        if self.pads.iter().any(|p| p.0 == d) {
            return Some(Target::Pad(d));
        }
        self.belts.contains(&d).then_some(Target::Belt(d))
    }

    /// 항목이 이 장면에서 말이 되는가(실행 전에 모두 본다). 오류 글에는 접두어가 없다.
    pub fn check_act(&self, a: &Act) -> Result<(), String> {
        self.check_shape(a)?;
        self.check_room(a)
    }

    fn check_shape(&self, a: &Act) -> Result<(), String> {
        let need_types = || if self.w.types.is_empty() { Err("the scene has no workpieces".to_string()) } else { Ok(()) };
        match a {
            Act::Source { source, ty, .. } => {
                if self.stack_of(source).is_none() {
                    return Err(format!("source {} is not a part source or stack of the scene", clip(source)));
                }
                need_types()?;
                self.type_index(ty.as_deref(), 0).map(|_| ())
            }
            Act::Put { at, ty, s_mm, .. } => {
                need_types()?;
                self.type_index(ty.as_deref(), 0)?;
                let t = self.target(at).ok_or_else(|| format!("at {} is not a point, stack, pad or belt of the scene", clip(at)))?;
                match (t, s_mm) {
                    (Target::Belt(b), Some(s)) => {
                        let l = self.geoms[b].dims.v("length_mm");
                        if !(0.0..=l).contains(s) {
                            return Err(format!("s_mm must be 0 to {}", geom::n(l)));
                        }
                    }
                    (Target::Belt(_), None) => {}
                    (_, Some(_)) => return Err("s_mm needs a belt in at".into()),
                    _ => {}
                }
                Ok(())
            }
            Act::PutPos { ty, .. } => {
                need_types()?;
                self.type_index(ty.as_deref(), 0).map(|_| ())
            }
            Act::Remove(x) => self.target(x).map(|_| ()).ok_or_else(|| format!("remove {} is not a point, stack, pad or belt of the scene", clip(x))),
            Act::Reset => Ok(()),
            Act::Move { part, at, s_mm } => {
                self.part_exists(*part).then_some(()).ok_or_else(|| format!("no part {part}"))?;
                self.check_shape(&Act::Put { at: at.clone(), ty: None, tone: None, s_mm: *s_mm })
            }
            Act::Tone { part, .. } | Act::RemovePart(part) => self.part_exists(*part).then_some(()).ok_or_else(|| format!("no part {part}")),
        }
    }

    /// 매거진에는 기다리는 부품이 20개까지다(43의 4.1, B8 BR3). 시험 중에도 지금의 개수로 본다: 길과 쪽은 `check_act`가, 시나리오는 실행할 때 `run_act`가 부른다.
    pub fn check_room(&self, a: &Act) -> Result<(), String> {
        let (at, part) = match a {
            Act::Source { source, .. } => (source, None),
            Act::Put { at, .. } => (at, None),
            Act::Move { part, at, .. } => (at, Some(*part)),
            _ => return Ok(()),
        };
        let Some(Target::Stack(si)) = self.target(at) else { return Ok(()) };
        let own = part.is_some_and(|p| self.waiting[si].contains(&p));
        if self.waiting[si].len() >= MAX_CONTENT && !own {
            return Err(format!("the stack {} already holds {MAX_CONTENT} parts", clip(at)));
        }
        Ok(())
    }

    /// 항목 하나를 실행한다(`check_act`를 통과한 것). 실행 뒤에는 `sense_now`를 부른다.
    pub fn apply(&mut self, a: &Act) {
        match a {
            Act::Source { source, ty, tone } => {
                let Some(si) = self.stack_of(source) else { return };
                let Ok(ty) = self.type_index(ty.as_deref(), self.refill_type(si)) else { return };
                if let Some(pi) = self.make(ty, tone.clone()) {
                    let id = self.parts[pi].id;
                    self.waiting[si].push(id);
                    self.ev("refill", Some(id), json!({"stack": source}));
                    self.drop_stack(si, false);
                }
            }
            Act::Put { at, ty, tone, s_mm } => {
                let Some(t) = self.target(at) else { return };
                let Ok(ty) = self.type_index(ty.as_deref(), self.first_type()) else { return };
                let Some(pi) = self.make(ty, tone.clone()) else { return };
                let id = self.parts[pi].id;
                self.put_part(pi, t, *s_mm);
                self.ev("put", Some(id), json!({"at": at}));
            }
            Act::Move { part, at, s_mm } => {
                let (Some(pi), Some(t)) = (self.pi_of(*part), self.target(at)) else { return };
                // 옛 자리를 떠난다: 기둥에서 빠지고 점·운반자와의 연결이 끊긴다
                self.waiting.iter_mut().for_each(|w| w.retain(|x| *x != *part));
                let q = &mut self.parts[pi];
                (q.state, q.at, q.carrier, q.rule) = (PState::Stack, NONE, NONE, NONE);
                self.put_part(pi, t, *s_mm);
                self.ev("put", Some(*part), json!({"at": at}));
            }
            Act::Tone { part, tone } => {
                self.set_tone(*part, tone);
            }
            Act::RemovePart(id) => {
                if self.pi_of(*id).is_some() {
                    self.parts.retain(|q| q.id != *id);
                    self.waiting.iter_mut().for_each(|w| w.retain(|x| *x != *id));
                    self.ev("removed", Some(*id), json!({"from": "part"}));
                }
            }
            Act::PutPos { pos, ty, tone } => {
                let Ok(ty) = self.type_index(ty.as_deref(), self.first_type()) else { return };
                let Some(pi) = self.make(ty, tone.clone()) else { return };
                let at = (0..self.pts.len()).find(|&k| dist3(self.pts[k], *pos) <= self.w.points[k].catch_mm).unwrap_or(NONE);
                let q = &mut self.parts[pi];
                (q.state, q.p, q.at) = (PState::Left, *pos, at);
                let id = q.id;
                self.ev("put", Some(id), json!({"pos": [geom::n(pos[0]), geom::n(pos[1]), geom::n(pos[2])]}));
            }
            Act::Remove(x) => {
                let Some(t) = self.target(x) else { return };
                let gone: Vec<u32> = match t {
                    Target::Point(k) => self.parts.iter().filter(|q| q.at == k && q.state != PState::Stack).map(|q| q.id).collect(),
                    Target::Stack(si) => self.waiting[si].last().copied().into_iter().collect(),
                    Target::Pad(d) => self.parts.iter().filter(|q| q.state == PState::Held && q.carrier == d).map(|q| q.id).collect(),
                    Target::Belt(b) => self.parts.iter().filter(|q| q.state == PState::Belt && q.carrier == b).map(|q| q.id).collect(),
                };
                for id in gone {
                    self.parts.retain(|q| q.id != id);
                    self.waiting.iter_mut().for_each(|w| w.retain(|x| *x != id));
                    self.ev("removed", Some(id), json!({"from": x}));
                }
            }
            Act::Reset => {
                self.parts.clear();
                self.waiting.iter_mut().for_each(Vec::clear);
                self.arrivals.iter_mut().for_each(|a| *a = 0);
                self.ev("reset", None, json!({}));
                self.build_start();
            }
        }
    }

    /// 부품 `pi`를 `t`에 둔다(새 부품과 옮기는 부품이 같이 쓴다). 매거진이면 맨 위에 서고, 패드면 잡힌 것으로 하고, 벨트면 `s_mm`에 둔다.
    fn put_part(&mut self, pi: usize, t: Target, s_mm: Option<f64>) {
        let id = self.parts[pi].id;
        match t {
            Target::Point(k) => self.place(pi, k),
            Target::Stack(si) => {
                self.waiting[si].push(id);
                self.drop_stack(si, false);
            }
            Target::Pad(d) => {
                let p = self.ws[d].p;
                if self.parts.iter().any(|q| q.state == PState::Held && q.carrier == d && q.id != id) {
                    (self.parts[pi].state, self.parts[pi].p) = (PState::Left, p);
                    self.ev("left", Some(id), json!({"why": "the pad already holds a part"}));
                } else {
                    let q = &mut self.parts[pi];
                    (q.state, q.carrier, q.off, q.p) = (PState::Held, d, [0.0; 3], p);
                }
            }
            Target::Belt(b) => {
                let (l, surf) = (self.geoms[b].dims.v("length_mm"), self.ws[b].mul(&self.geoms[b].frames[self.surface[b]].1));
                let s = -l / 2.0 + s_mm.unwrap_or(0.0);
                let q = &mut self.parts[pi];
                (q.state, q.carrier, q.s, q.p) = (PState::Belt, b, s, surf.pt([s, 0.0, 0.0]));
            }
        }
    }

    /// 화면 길이 쓰는 검사: 이름이 점, 매거진, 패드, 벨트 중 하나인가
    pub fn target_exists(&self, x: &str) -> bool {
        self.target(x).is_some()
    }

    pub fn part_exists(&self, id: u32) -> bool {
        self.pi_of(id).is_some()
    }

    pub fn has_stacks(&self) -> bool {
        !self.w.stacks.is_empty()
    }

    /// `sim/start`의 `stacks`: 매거진 하나의 시작 내용을 모두 바꾼다(빈 목록이면 비운다). 종류 이름은 먼저 검사한다.
    pub fn set_stack_content(&mut self, stack: &str, types: &[String]) -> Result<(), String> {
        let si = self.stack_of(stack).ok_or_else(|| format!("stack {} is not a stack of the scene", clip(stack)))?;
        let idx: Vec<usize> = types.iter().map(|t| self.type_index(Some(t), 0)).collect::<Result<_, _>>()?;
        self.start_set[si] = true;
        self.start[si].clear();
        self.start[si].extend(idx.into_iter().map(|t| (t, None)));
        Ok(())
    }

    /// 부품 하나의 색만 바꾼다(화면)
    pub fn set_tone(&mut self, id: u32, tone: &str) -> bool {
        match self.pi_of(id) {
            Some(pi) => {
                self.parts[pi].tone = Some(tone.to_string());
                self.ev("tone", Some(id), json!({"tone": tone}));
                true
            }
            None => false,
        }
    }

    // ── 센서(R9) ──

    pub fn sense_bit(&self, name: &str) -> bool {
        self.sense_out.iter().any(|o| o.as_deref() == Some(name))
    }

    /// 감지 규칙이 쓰는 비트 이름들
    pub fn sense_bits(&self) -> impl Iterator<Item = &str> {
        self.sense_out.iter().flatten().map(String::as_str)
    }

    pub fn has_sense(&self, device: &str) -> bool {
        self.w.senses.iter().any(|s| self.ids[s.dev] == device)
    }

    /// 감지 장치의 비트를 이번 시뮬레이션 내내 `v`로 둔다(고장 `sensor_stuck_on`, `sensor_stuck_off`). 그런 장치가 없으면 false.
    pub fn stick(&mut self, device: &str, v: bool) -> bool {
        let mut hit = false;
        for k in 0..self.w.senses.len() {
            if self.ids[self.w.senses[k].dev] == device {
                self.stuck[k] = Some(v);
                hit = true;
            }
        }
        hit
    }

    /// 장면의 사건이 센서 비트를 썼다: 그때부터 규칙은 그 비트를 쓰지 않는다
    pub fn force(&mut self, name: &str) {
        for k in 0..self.sense_out.len() {
            if self.sense_out[k].as_deref() == Some(name) {
                self.forced[k] = true;
            }
        }
    }

    /// 규칙에게 비트를 돌려준다(화면의 `{"value": null}`)
    pub fn release(&mut self, name: &str) {
        for k in 0..self.sense_out.len() {
            if self.sense_out[k].as_deref() == Some(name) {
                self.forced[k] = false;
            }
        }
    }

    pub fn forced_names(&self) -> Vec<String> {
        let set: BTreeSet<&str> = (0..self.forced.len()).filter(|&k| self.forced[k]).filter_map(|k| self.sense_out[k].as_deref()).collect();
        set.into_iter().map(String::from).collect()
    }

    /// 지금 자리로 센서를 쓴다(경로는 점이다)
    pub fn sense_now(&mut self, io: &mut dyn super::plant::PlantIo) {
        self.old_p.clear();
        self.old_p.extend(self.parts.iter().map(|p| p.p));
        self.sense(io);
    }

    fn sense(&mut self, io: &mut dyn super::plant::PlantIo) {
        for &k in &self.sense_order {
            let v = match self.stuck[k] {
                Some(v) => v,
                None if self.forced[k] => continue,
                None => match &self.w.senses[k].rule {
                    SenseRule::Point { point, within_mm, metal } => {
                        let pp = self.pts[*point];
                        self.parts.iter().enumerate().any(|(i, q)| (!*metal || self.w.types[q.ty].metal) && q.state != PState::Stack && seg_dist(self.old_p.get(i).copied().unwrap_or(q.p), q.p, pp) <= *within_mm + EPS)
                    }
                    SenseRule::HeldBy(d) => self.parts.iter().any(|q| q.state == PState::Held && q.carrier == *d),
                    // 한 펄스(mm_per_pulse)마다 켜짐과 꺼짐 한 번씩. `mm_per_pulse >= 0.5`라 나누기가 0이 되지 않는다.
                    SenseRule::Pulse { belt, mm_per_pulse } => ((self.vals[*belt] / (mm_per_pulse / 2.0) + EPS).floor() as i64).rem_euclid(2) == 1,
                },
            };
            if let Some(n) = &self.sense_out[k] {
                io.set_bit(n, v);
            }
        }
    }

    // ── 한 걸음 ──

    /// 축이 움직인 뒤에 부른다(`vals`와 `deltas`는 플랜트가 채운다). 순서는 설계 2.4.
    pub fn tick(&mut self, io: &mut dyn super::plant::PlantIo, dt: f64) {
        self.old_p.clear();
        self.old_p.extend(self.parts.iter().map(|p| p.p));
        self.refresh();
        // R1: 점에 쉬는 부품은 그 점의 틀을 따라간다
        for q in self.parts.iter_mut().filter(|q| q.state == PState::Rest && q.at != NONE) {
            q.p = self.pts[q.at];
        }
        self.release_pads(io);
        self.rides();
        self.catches();
        self.picks(io);
        self.belts_step();
        for si in 0..self.w.stacks.len() {
            self.drop_stack(si, false);
        }
        self.sense(io);
        self.t += dt;
    }

    /// R2: 코일이 꺼진 패드는 잡은 부품을 놓는다. 부품은 바로 아래의 가장 높은 점으로 간다.
    fn release_pads(&mut self, io: &mut dyn super::plant::PlantIo) {
        for k in 0..self.pads.len() {
            let (d, reach) = (self.pads[k].0, self.pads[k].2);
            if self.pads[k].1.as_deref().is_some_and(|n| io.bit(n)) {
                continue;
            }
            for pi in 0..self.parts.len() {
                if self.parts[pi].state != PState::Held || self.parts[pi].carrier != d {
                    continue;
                }
                let (p, id) = (self.parts[pi].p, self.parts[pi].id);
                match self.highest(|pp, _| dist_xz(pp, p) <= reach && pp[1] <= p[1] + EPS) {
                    Some(pt) => {
                        self.place(pi, pt);
                        let (pad, point) = (self.ids[d].clone(), self.w.points[pt].name.clone());
                        self.ev("release", Some(id), json!({"pad": pad, "point": point}));
                    }
                    None => {
                        (self.parts[pi].state, self.parts[pi].carrier) = (PState::Left, NONE);
                        self.ev("left", Some(id), json!({"why": "released with no point below"}));
                    }
                }
            }
        }
    }

    /// 막대 끝의 세계 좌표(축 값 `a`)
    fn tip(&self, d: usize, a: f64) -> V3 {
        let g = &self.geoms[d];
        match &g.axis {
            Some(ax) => self.ws[d].pt(geom::moving_pos(&self.kinds[d], &g.dims, ax, a)),
            None => self.ws[d].p,
        }
    }

    fn stroke(&self, d: usize) -> f64 {
        self.geoms[d].axis.as_ref().map_or(0.0, |a| a.range[1])
    }

    /// R3: 타는 부품은 막대 끝을 따라가고, 끝(행정 - 창)에서 `to`에 놓인다. 그 전에 막대가 물러나면 `left`.
    fn rides(&mut self) {
        for pi in 0..self.parts.len() {
            if self.parts[pi].state != PState::Ride {
                continue;
            }
            let (c, rule, id) = (self.parts[pi].carrier, self.parts[pi].rule, self.parts[pi].id);
            let tip = self.tip(c, self.vals[c]);
            let off = self.parts[pi].off;
            self.parts[pi].p = [tip[0] + off[0], tip[1] + off[1], tip[2] + off[2]];
            if self.vals[c] >= self.stroke(c) - self.win[c] - EPS {
                let (to, end) = (self.w.pushes[rule].to, self.parts[pi].p);
                self.place(pi, to);
                let (by, point) = (self.ids[c].clone(), self.w.points[to].name.clone());
                self.ev("place", Some(id), json!({"by": by, "point": point}));
                let gap = dist_xz(end, self.parts[pi].p);
                if gap > f64::max(10.0, self.w.points[to].catch_mm) {
                    self.ev("far", Some(id), json!({"point": point, "mm": geom::n(gap)}));
                }
            } else if self.deltas[c] < -EPS {
                (self.parts[pi].state, self.parts[pi].carrier) = (PState::Left, NONE);
                let why = format!("{} went back before its end", self.ids[c]);
                self.ev("left", Some(id), json!({"why": why}));
            }
        }
    }

    /// R4: 막대가 접촉 값을 앞으로 지나가면 `from`의 부품을 잡는다
    fn catches(&mut self) {
        for k in 0..self.pushers.len() {
            let d = self.pushers[k].0;
            let (pos, old) = (self.vals[d], self.vals[d] - self.deltas[d]);
            if pos <= old + EPS {
                continue;
            }
            let stroke = self.stroke(d);
            let (tip0, u) = (self.tip(d, 0.0), self.ws[d].dir([1.0, 0.0, 0.0]));
            for ri in 0..self.pushers[k].1.len() {
                let r = self.pushers[k].1[ri];
                let (from, to) = (self.w.pushes[r].from, self.w.pushes[r].to);
                let (fp, catch) = (self.pts[from], self.w.points[from].catch_mm);
                for pi in 0..self.parts.len() {
                    let q = &self.parts[pi];
                    if !matches!(q.state, PState::Rest | PState::Belt) || dist3(q.p, fp) > catch + EPS {
                        continue;
                    }
                    let rel = sub(q.p, tip0);
                    let ac = (rel[0] * u[0] + rel[1] * u[1] + rel[2] * u[2] - self.half(q)).clamp(0.0, stroke);
                    if old - EPS <= ac && ac <= pos + EPS {
                        let id = q.id;
                        let tc = [tip0[0] + u[0] * ac, tip0[1] + u[1] * ac, tip0[2] + u[2] * ac];
                        let off = sub(q.p, tc);
                        let q = &mut self.parts[pi];
                        (q.off, q.state, q.carrier, q.rule, q.at) = (off, PState::Ride, d, r, NONE);
                        let (by, point) = (self.ids[d].clone(), self.w.points[from].name.clone());
                        self.ev("catch", Some(id), json!({"by": by, "point": point, "contact_mm": geom::n((ac * 100.0).round() / 100.0)}));
                        let tipw = self.tip(d, pos);
                        self.parts[pi].p = [tipw[0] + off[0], tipw[1] + off[1], tipw[2] + off[2]];
                        if pos >= stroke - self.win[d] - EPS {
                            self.place(pi, to);
                            let point = self.w.points[to].name.clone();
                            self.ev("place", Some(id), json!({"by": by, "point": point}));
                        }
                    }
                }
            }
        }
    }

    /// R5, R6: 코일이 켜진 빈 패드는 가장 가까운 부품을 집고, 집은 부품은 패드를 따라간다
    fn picks(&mut self, io: &mut dyn super::plant::PlantIo) {
        for k in 0..self.pads.len() {
            let (d, reach) = (self.pads[k].0, self.pads[k].2);
            if !self.pads[k].1.as_deref().is_some_and(|n| io.bit(n)) || self.parts.iter().any(|q| q.state == PState::Held && q.carrier == d) {
                continue;
            }
            let hp = self.ws[d].p;
            let best = self
                .parts
                .iter()
                .enumerate()
                .filter(|(_, q)| matches!(q.state, PState::Rest | PState::Belt | PState::Left))
                .map(|(i, q)| (dist3([q.p[0], q.p[1] + self.w.types[q.ty].h_mm, q.p[2]], hp), q.id, i))
                .filter(|c| c.0 <= reach + EPS)
                .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            if let Some((_, id, pi)) = best {
                let off = sub(self.parts[pi].p, hp);
                let q = &mut self.parts[pi];
                (q.state, q.carrier, q.at, q.off) = (PState::Held, d, NONE, off);
                let pad = self.ids[d].clone();
                self.ev("pick", Some(id), json!({"pad": pad}));
            }
        }
        for q in self.parts.iter_mut().filter(|q| q.state == PState::Held) {
            let hp = self.ws[q.carrier].p;
            q.p = [hp[0] + q.off[0], hp[1] + q.off[1], hp[2] + q.off[2]];
        }
    }

    /// R7: 벨트 이동, 멈춤, 입구 끝, 줄서기, 끝에서 떨어짐
    fn belts_step(&mut self) {
        for bi in 0..self.belts.len() {
            let b = self.belts[bi];
            let (l, delta) = (self.geoms[b].dims.v("length_mm"), self.deltas[b]);
            let surf = self.ws[b].mul(&self.geoms[b].frames[self.surface[b]].1);
            let inv = surf.inv();
            // 멈춤 좌표: 멈춤 점의 자리를 벨트의 +x에 비춘 값. 실린더 멈춤은 축 값이 행정의 반 이상일 때만 선다.
            let mut stops: Vec<f64> = Vec::new();
            for s in &self.w.stops {
                let active = match s.belt {
                    None => s.dev == b,
                    Some(sb) => sb == b && self.vals[s.dev] >= self.stroke(s.dev) / 2.0,
                };
                if active {
                    stops.push(inv.pt(self.pts[s.at])[0]);
                }
            }
            let mut on: Vec<usize> = (0..self.parts.len()).filter(|&i| self.parts[i].state == PState::Belt && self.parts[i].carrier == b).collect();
            for &i in &on {
                let (s0, half) = (self.parts[i].s, self.half(&self.parts[i]));
                let mut s1 = s0 + delta;
                for &k in &stops {
                    if delta > 0.0 && s0 <= k + EPS {
                        s1 = s1.min(k);
                    }
                    if delta < 0.0 && s0 >= k - EPS {
                        s1 = s1.max(k);
                    }
                }
                self.parts[i].s = s1.max(-l / 2.0 + half);
            }
            on.sort_by(|&a, &c| self.parts[c].s.total_cmp(&self.parts[a].s).then(self.parts[a].id.cmp(&self.parts[c].id)));
            for j in 1..on.len() {
                let front = (self.parts[on[j - 1]].s, self.half(&self.parts[on[j - 1]]));
                let me = on[j];
                let lim = front.0 - front.1 - self.half(&self.parts[me]);
                self.parts[me].s = self.parts[me].s.min(lim);
            }
            for &i in &on {
                self.parts[i].p = surf.pt([self.parts[i].s, 0.0, 0.0]);
                let at = self.parts[i].at;
                if at != NONE && dist3(self.parts[i].p, self.pts[at]) > self.w.points[at].catch_mm {
                    self.parts[i].at = NONE;
                }
                if self.parts[i].s > l / 2.0 + EPS {
                    let (id, half) = (self.parts[i].id, self.half(&self.parts[i]));
                    let edge = surf.pt([l / 2.0 + half, 0.0, 0.0]);
                    match self.highest(|pp, k| pp[1] < edge[1] - EPS && dist_xz(pp, edge) <= self.w.points[k].catch_mm + EPS) {
                        Some(pt) => {
                            self.place(i, pt);
                            let (belt, point) = (self.ids[b].clone(), self.w.points[pt].name.clone());
                            self.ev("fall", Some(id), json!({"belt": belt, "point": point}));
                        }
                        None => {
                            (self.parts[i].state, self.parts[i].carrier, self.parts[i].p) = (PState::Left, NONE, edge);
                            let why = format!("fell off the end of {} with no point below", self.ids[b]);
                            self.ev("left", Some(id), json!({"why": why}));
                        }
                    }
                }
            }
        }
    }

    // ── 결과 ──

    pub fn tone_of(&self, p: &WPart) -> String {
        p.tone.clone().unwrap_or_else(|| self.w.types[p.ty].tone.clone())
    }

    /// 부품마다 한 줄: `id`, `type`, `tone`, `material`, `state`, `at`, `arrival`, `x`, `y`, `z`, `holder`
    pub fn parts_json(&self) -> Vec<Value> {
        let r = |x: f64| (x * 1000.0).round() / 1000.0;
        self.parts
            .iter()
            .map(|p| {
                let holder = matches!(p.state, PState::Ride | PState::Held).then(|| self.ids[p.carrier].clone());
                json!({
                    "id": p.id, "type": self.w.types[p.ty].name, "tone": self.tone_of(p),
                    "material": if self.w.types[p.ty].metal { "metal" } else { "plastic" },
                    "state": p.state.name(), "at": (p.at != NONE).then(|| self.w.points[p.at].name.clone()),
                    "arrival": (p.at != NONE).then_some(p.arrival),
                    "x": r(p.p[0]), "y": r(p.p[1]), "z": r(p.p[2]), "holder": holder
                })
            })
            .collect()
    }

    pub fn stacks_json(&self) -> Value {
        let m: BTreeMap<&str, usize> = self.w.stacks.iter().enumerate().map(|(i, s)| (self.ids[s.dev].as_str(), self.waiting[i].len())).collect();
        json!(m)
    }

    /// 펄스 한 반 폭이 스캔보다 짧은 센서의 글(`limits`에 실린다)
    pub fn pulse_limits(&self, scan: f64, speed: impl Fn(usize) -> f64) -> Vec<String> {
        let mut out = Vec::new();
        for it in &self.w.senses {
            if let SenseRule::Pulse { belt, mm_per_pulse } = &it.rule {
                let v = speed(*belt);
                if v > 0.0 && mm_per_pulse / 2.0 / v * 1000.0 < scan {
                    out.push(format!("{}: a pulse level lasts {} ms, less than the scan ({} ms): the program misses pulses", self.ids[it.dev], geom::n(mm_per_pulse / 2.0 / v * 1000.0), geom::n(scan)));
                }
            }
        }
        out
    }
}
