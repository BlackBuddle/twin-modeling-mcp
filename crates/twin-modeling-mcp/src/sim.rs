//! 프로그램 없는 실행(reusable-parts 3.1d-2, 설계 46의 3.3·3.4절): 도구 `model_sim`과 3D 화면의 시뮬레이션 경로(`/api/ladder/sim/*`).
//! 둘 다 핀 이름으로 읽고 쓰는 입출력(`NameIo`)과 설비 모델(`Plant`)을 쓴다. PLC도 IL도 없다. 실제 시간의 시계는 여기에만 있다(엔진에는 없다).

use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};
use twin_engine::bind::{NameIo, PlantIo, NO_ADDRESSES};
use twin_engine::model::{self, PinEvent};
use twin_engine::plant::{parse_faults, Mode, Plant};
use twin_engine::run::PartScript;
use twin_engine::scene::{self, Scene};
use twin_engine::work::Act;
use twin_page::store::Store;

/// 한 호출의 실제 시간 상한(`model_sim`)
const MAX_WALL: Duration = Duration::from_secs(5);
const MAX_SIMS: usize = 4;
/// 요청 하나가 하는 일의 시간 상한과 따라잡는 스캔 수의 상한(plc-mcp의 화면 시뮬레이션과 같은 값)
const SLICE: Duration = Duration::from_millis(250);
const MAX_RT_SCANS: i64 = 200;
const MAX_STEP_SCANS: u64 = 500;

pub type Fail = (u16, String);

fn fail(c: u16, m: impl Into<String>) -> Fail {
    (c, m.into())
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// 모델 하나를 읽는다: 초안이 있으면 초안, 없으면 `scene.json`. 검사(`model::check`)를 먼저 하고 핀 호스트로 읽는다.
/// 오류는 (HTTP 코드, 글)이다: 없는 모델은 저장소의 404를 그대로 주고, 검사에 걸린 모델은 400이다(R3-12).
pub fn load(st: &Store, name: &str) -> Result<(Scene, &'static str), Fail> {
    let (v, ran) = st.base_kind(name)?;
    model::check(&v).map_err(|e| fail(400, e))?;
    Ok((scene::parse_with(name, &v, &NO_ADDRESSES).map_err(|e| fail(400, e))?, ran))
}

/// `model_sim`: 돌린 입력을 그 모델의 마지막 실행으로 메모리에 둔다(화면의 "마지막 model_sim 입력"). 디스크에는 쓰지 않는다.
pub fn model_sim(st: &Store, args: &Value, last: &Mutex<BTreeMap<String, Value>>) -> Result<Value, String> {
    let name = args.get("name").and_then(Value::as_str).ok_or("name must be a string")?;
    let (s, ran) = load(st, name).map_err(|(_, m)| m)?;
    let t0 = Instant::now();
    let mut r = model::sim(&s, args, &mut || t0.elapsed() > MAX_WALL)?;
    r["ran"] = json!(ran);
    lock(last).insert(name.to_string(), args.clone());
    Ok(r)
}

struct Pin {
    name: String,
    word: bool,
    out: bool,
}

struct Run {
    /// `sim/input`이 값을 `model_sim`의 사건과 같은 규칙으로 읽는 데 쓴다
    scene: Scene,
    plant: Plant,
    io: NameIo,
    pins: Vec<Pin>,
    t: i64,
    scans: u64,
    scan_ms: i64,
    realtime: bool,
    last: Instant,
    events: Vec<PinEvent>,
    ei: usize,
    script: PartScript,
    total: Option<i64>,
    err: Option<String>,
}

impl Run {
    /// 스캔 하나: 때가 된 핀 사건, 공작물 사건, 설비 한 걸음의 차례(`model::sim`과 같다).
    fn scan(&mut self) -> Result<(), String> {
        while self.ei < self.events.len() && self.events[self.ei].t <= self.t {
            let (pin, word, value) = {
                let e = &self.events[self.ei];
                (e.pin.clone(), e.word, e.value)
            };
            self.set(&pin, word, value);
            self.ei += 1;
        }
        // 공작물 사건 대본: 시각 순서로, 하나라도 있었으면 부품 센서를 다시 센다(`model::sim`과 같다)
        self.script.due(&mut self.plant, self.t, &mut self.io)?;
        self.plant.tick(&mut self.io, self.scan_ms as f64);
        self.t += self.scan_ms;
        self.scans += 1;
        Ok(())
    }

    /// 핀 값을 쓴다. 부품 감지 규칙이 쓰는 비트이면 그때부터 규칙은 그 비트를 쓰지 않는다(`forced`).
    fn set(&mut self, pin: &str, word: bool, value: i64) {
        if word {
            self.io.set_word(pin, value);
        } else {
            self.io.set_bit(pin, value != 0);
            if let Some(r) = self.plant.work_mut().filter(|r| r.sense_bit(pin)) {
                r.force(pin);
            }
        }
    }

    /// `n`번 스캔한다(시간 상한에서 멈춘다). 돌린 횟수를 돌려준다. 정해진 길이가 있으면(`last_run`) 그 끝에서 멈춘다.
    fn run(&mut self, n: u64) -> u64 {
        let end = Instant::now() + SLICE;
        let mut done = 0;
        while done < n && self.err.is_none() && self.total.is_none_or(|t| self.t < t) {
            if let Err(e) = self.scan() {
                self.err = Some(e);
                break;
            }
            done += 1;
            if Instant::now() > end {
                break;
            }
        }
        done
    }

    fn catch_up(&mut self) {
        if !self.realtime || self.err.is_some() {
            self.last = Instant::now();
            return;
        }
        let n = self.last.elapsed().as_millis() as i64 / self.scan_ms;
        if n == 0 {
            return;
        }
        self.last = if n > MAX_RT_SCANS { Instant::now() } else { self.last + Duration::from_millis((n * self.scan_ms) as u64) };
        self.run(n.min(MAX_RT_SCANS) as u64);
    }

    /// 화면의 시뮬레이션 상태. `bits`와 `words`에는 모델의 모든 핀이 있다(없는 키가 없다: 화면은 "모름"을 그리지 않는다).
    fn json(&self, id: &str, source: &str) -> Value {
        let bits: BTreeMap<&str, bool> = self.pins.iter().filter(|p| !p.word).map(|p| (p.name.as_str(), self.io.bit(&p.name))).collect();
        let words: BTreeMap<&str, i64> = self.pins.iter().filter(|p| p.word).map(|p| (p.name.as_str(), self.io.word(&p.name))).collect();
        let mut v = json!({"sim": id, "source": source, "scan_ms": self.scan_ms, "scans": self.scans, "time_ms": self.t, "realtime": self.realtime, "running": self.err.is_none(), "error": self.err, "bits": bits, "words": words, "dwords": {}, "plant": self.plant.state_json()});
        if let Some(t) = self.total {
            v["total_ms"] = json!(t);
            v["done"] = json!(self.t >= t);
        }
        v
    }
}

struct PageSim {
    id: String,
    source: String,
    run: Mutex<Run>,
}

/// 3D 화면의 시뮬레이션들(같은 때 4개까지)과 모델마다 마지막 `model_sim` 입력
#[derive(Default)]
pub struct Sims {
    list: Mutex<Vec<std::sync::Arc<PageSim>>>,
    next: std::sync::atomic::AtomicU64,
    pub last: Mutex<BTreeMap<String, Value>>,
}

fn body(b: &[u8]) -> Result<Value, Fail> {
    serde_json::from_slice::<Value>(b).ok().filter(Value::is_object).ok_or_else(|| fail(400, "body must be a JSON object"))
}

fn text_arg(o: &serde_json::Map<String, Value>, k: &str) -> Result<Option<String>, Fail> {
    match o.get(k).filter(|x| !x.is_null()) {
        None => Ok(None),
        Some(x) => x.as_str().filter(|s| s.len() <= 32).map(|s| Some(s.to_string())).ok_or_else(|| fail(400, format!("{k} must be text of at most 32 characters"))),
    }
}

/// `sim/part` 본문을 항목 하나로 읽는다(새 부품: `at`, `pos`, `remove`, `reset` 중 하나. 부품 하나: `part`와 `at`, `tone`, `remove`(true) 중 하나).
fn part_act(v: &Value) -> Result<Act, Fail> {
    let o = v.as_object().ok_or_else(|| fail(400, "body must be an object"))?;
    if let Some(k) = o.keys().find(|k| !matches!(k.as_str(), "sim" | "part" | "at" | "pos" | "remove" | "reset" | "type" | "tone" | "s_mm")) {
        return Err(fail(400, format!("unknown field {}", k.chars().take(20).collect::<String>())));
    }
    let (ty, tone) = (text_arg(o, "type")?, text_arg(o, "tone")?);
    if tone.as_deref().is_some_and(|t| !twin_engine::work::TONE_TEXT.split(", ").any(|x| x == t)) {
        return Err(fail(400, format!("tone must be one of {}", twin_engine::work::TONE_TEXT)));
    }
    let s_mm = match o.get("s_mm").filter(|x| !x.is_null()) {
        None => None,
        Some(x) => Some(x.as_f64().filter(|n| n.is_finite()).ok_or_else(|| fail(400, "s_mm must be a number"))?),
    };
    let has = |k: &str| o.get(k).is_some_and(|x| !x.is_null());
    if let Some(p) = o.get("part").filter(|x| !x.is_null()) {
        let part = p.as_u64().filter(|n| *n <= u64::from(u32::MAX)).ok_or_else(|| fail(400, "part must be a part number"))? as u32;
        return match (text_arg(o, "at")?, tone, o.get("remove").filter(|x| !x.is_null())) {
            (Some(at), None, None) => Ok(Act::Move { part, at, s_mm }),
            (None, Some(tone), None) => Ok(Act::Tone { part, tone }),
            (None, None, Some(Value::Bool(true))) => Ok(Act::RemovePart(part)),
            _ => Err(fail(400, "give part with one of at, tone, remove (true)")),
        };
    }
    let reset = o.get("reset") == Some(&Value::Bool(true));
    if [has("at"), has("pos"), has("remove"), reset].iter().filter(|b| **b).count() != 1 {
        return Err(fail(400, "give one of at, pos, remove, reset (or part with at, tone, remove)"));
    }
    if reset {
        return Ok(Act::Reset);
    }
    if let Some(x) = text_arg(o, "remove")? {
        return Ok(Act::Remove(x));
    }
    if let Some(at) = text_arg(o, "at")? {
        return Ok(Act::Put { at, ty, tone, s_mm });
    }
    let pos = o.get("pos").and_then(|p| twin_engine::geom::vec3(p, 100_000.0)).ok_or_else(|| fail(400, "pos must be 3 numbers from -100000 to 100000"))?;
    Ok(Act::PutPos { pos, ty, tone })
}

impl Sims {
    fn find(&self, id: Option<&str>) -> Result<std::sync::Arc<PageSim>, Fail> {
        lock(&self.list).iter().find(|s| Some(s.id.as_str()) == id).cloned().ok_or_else(|| fail(404, "unknown simulation (it was stopped or replaced)"))
    }

    /// `/api/ladder/sim/*`: (HTTP 코드, JSON). 토큰과 Origin 검사는 호스트가 이미 했다. `None`이면 이 경로가 아니다.
    pub fn route(&self, st: &Store, method: &str, path: &str, query: &str, b: &[u8]) -> Option<(u16, Value)> {
        let r = match (method, path) {
            ("POST", "/api/ladder/sim/start") => self.start(st, b),
            ("GET", "/api/ladder/sim/state") => self.state(query),
            ("POST", "/api/ladder/sim/step") => self.step(b),
            ("POST", "/api/ladder/sim/input") => self.input(b),
            ("POST", "/api/ladder/sim/part") => self.part(b),
            ("POST", "/api/ladder/sim/stop") => self.stop(b),
            (_, "/api/ladder/sim/start" | "/api/ladder/sim/state" | "/api/ladder/sim/step" | "/api/ladder/sim/input" | "/api/ladder/sim/part" | "/api/ladder/sim/stop") => Err(fail(405, "method not allowed")),
            _ => return None,
        };
        Some(match r {
            Ok(v) => (200, v),
            Err((c, m)) => (c, json!({"error": m})),
        })
    }

    fn start(&self, st: &Store, b: &[u8]) -> Result<Value, Fail> {
        let v = body(b)?;
        let name = v.get("scene").and_then(Value::as_str).ok_or_else(|| fail(400, "scene must be a string"))?;
        // null은 없는 것과 같다. 글이 아니거나 모르는 값은 오류이다(R3-11)
        let source = match v.get("source").filter(|x| !x.is_null()).map(Value::as_str) {
            None | Some(Some("none")) => "none",
            Some(Some("last_run")) => "last_run",
            Some(_) => return Err(fail(400, "source must be none or last_run")),
        };
        let (s, _) = load(st, name)?;
        let last = if source == "last_run" { Some(lock(&self.last).get(name).cloned().ok_or_else(|| fail(409, "no model_sim run of this model yet"))?) } else { None };
        let int = |o: Option<&Value>, k: &str, lo: i64, hi: i64, def: i64| match o.and_then(|o| o.get(k)).filter(|x| !x.is_null()) {
            None => Ok(def),
            Some(x) => x.as_i64().filter(|n| (lo..=hi).contains(n)).ok_or_else(|| fail(400, format!("{k} must be {lo} to {hi}"))),
        };
        let scan_ms = int(Some(&v), "scan_ms", 1, 100, int(last.as_ref(), "scan_ms", 1, 100, 10)?)?;
        let realtime = match v.get("realtime").filter(|x| !x.is_null()) {
            None => true,
            Some(x) => x.as_bool().ok_or_else(|| fail(400, "realtime must be true or false"))?,
        };
        let mut plant = Plant::new(&s, Mode::Sim);
        // 고장: 화면의 시뮬레이션에만 넣고 저장하지 않는다
        if let Some(f) = v.get("faults").filter(|x| !x.is_null()) {
            plant.apply_faults(&parse_faults(f, &NO_ADDRESSES).map_err(|e| fail(400, e))?).map_err(|e| fail(400, e))?;
        }
        // 매거진의 시작 내용: 장면의 내용을 바꾸고 저장하지 않는다
        if let Some(sv) = v.get("stacks").filter(|x| !x.is_null()) {
            let o = sv.as_object().filter(|o| o.len() <= 8).ok_or_else(|| fail(400, "stacks must be an object of at most 8 stacks, such as {\"magazine\":[\"white\"]}"))?;
            let r = plant.work_mut().filter(|r| r.has_stacks()).ok_or_else(|| fail(400, "the scene has no stacks"))?;
            for (id, list) in o {
                let types: Vec<String> = list
                    .as_array()
                    .filter(|a| a.len() <= 20)
                    .and_then(|a| a.iter().map(|t| t.as_str().filter(|s| s.len() <= 24).map(String::from)).collect())
                    .ok_or_else(|| fail(400, "each stack takes a list of at most 20 type names"))?;
                r.set_stack_content(id, &types).map_err(|e| fail(400, e))?;
            }
        }
        let (mut events, parts, total) = (Vec::new(), last.as_ref().map(|l| model::parse_part_events(l.get("part_events"))).transpose().map_err(|e| fail(400, e))?.unwrap_or_default(), match &last {
            Some(l) => Some(int(Some(l), "total_ms", 10, model::MAX_SIM_MS, 1000)?),
            None => None,
        });
        if let Some(l) = &last {
            events = model::parse_events(&s, l.get("events")).map_err(|e| fail(400, e))?;
            events.sort_by_key(|e| e.t);
        }
        // 시작의 차례는 `model::sim`과 같다: 대본의 t = 0 매거진 항목(첫 항목이 장면의 내용을 지운다), 설비 시작, 나머지 t = 0 사건
        let mut script = PartScript::new(&plant, parts).map_err(|e| fail(400, e))?;
        script.begin(&mut plant).map_err(|e| fail(400, e))?;
        let mut io = NameIo::new();
        plant.init(&mut io);
        script.after_init(&mut plant, &mut io).map_err(|e| fail(400, e))?;
        let pins = model::pins(&s).iter().map(|p| Pin { name: p["pin"].as_str().unwrap_or("").into(), word: p["type"] != "bit", out: p["dir"] == "out" }).collect();
        let id = format!("s{}", self.next.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1);
        let run = Run { scene: s, plant, io, pins, t: 0, scans: 0, scan_ms, realtime, last: Instant::now(), events, ei: 0, script, total, err: None };
        let out = run.json(&id, source);
        let mut list = lock(&self.list);
        if list.len() >= MAX_SIMS {
            list.remove(0);
        }
        list.push(std::sync::Arc::new(PageSim { id, source: source.into(), run: Mutex::new(run) }));
        Ok(out)
    }

    fn state(&self, query: &str) -> Result<Value, Fail> {
        let id = query.split('&').find_map(|p| p.strip_prefix("sim="));
        let sim = self.find(id)?;
        let mut r = lock(&sim.run);
        r.catch_up();
        Ok(r.json(&sim.id, &sim.source))
    }

    fn step(&self, b: &[u8]) -> Result<Value, Fail> {
        let v = body(b)?;
        let sim = self.find(v.get("sim").and_then(Value::as_str))?;
        let n = v.get("scans").and_then(Value::as_u64).filter(|n| (1..=MAX_STEP_SCANS).contains(n)).ok_or_else(|| fail(400, format!("scans must be 1 to {MAX_STEP_SCANS}")))?;
        let mut r = lock(&sim.run);
        let ran = r.run(n);
        r.last = Instant::now();
        let mut out = r.json(&sim.id, &sim.source);
        out["ran"] = ran.into();
        Ok(out)
    }

    fn input(&self, b: &[u8]) -> Result<Value, Fail> {
        let v = body(b)?;
        let sim = self.find(v.get("sim").and_then(Value::as_str))?;
        let pin = v.get("device").and_then(Value::as_str).ok_or_else(|| fail(400, "device is needed"))?;
        let value = v.get("value").ok_or_else(|| fail(400, "value is needed"))?;
        let mut r = lock(&sim.run);
        if r.err.is_some() {
            return Err(fail(409, "the simulation stopped with an error; start a new one"));
        }
        r.catch_up();
        let Some(p) = r.pins.iter().find(|p| p.name == pin) else {
            let names: Vec<&str> = r.pins.iter().map(|p| p.name.as_str()).take(16).collect();
            return Err(fail(400, format!("no pin \"{}\" (pins: {})", pin.chars().take(57).collect::<String>(), names.join(", "))));
        };
        let out = p.out;
        // `null` gives a bit that a part rule writes back to the rule
        if value.is_null() {
            let rule = r.plant.work_mut().filter(|w| w.sense_bit(pin)).ok_or_else(|| fail(400, "value null is only for a sensor bit that a part rule writes"))?;
            rule.release(pin);
        } else {
            if !out {
                return Err(fail(400, format!("{pin} is written by the plant (dir in); an event sets a pin with dir out")));
            }
            // 값은 `model_sim`의 사건과 같은 규칙으로 읽는다(bcd 핀은 BCD 워드, 32비트 구동기가 읽는 핀은 32비트 범위)
            let ev = model::parse_events(&r.scene, Some(&json!([{"t_ms": 0, "pin": pin, "value": value}]))).map_err(|e| fail(400, e.replacen("events[0].", "", 1)))?;
            r.set(&ev[0].pin, ev[0].word, ev[0].value);
        }
        // 입력을 바꾼 다음 스캔 한 번을 바로 돌려, 응답이 그 스캔의 결과가 된다
        r.run(1);
        r.last = Instant::now();
        Ok(r.json(&sim.id, &sim.source))
    }

    fn part(&self, b: &[u8]) -> Result<Value, Fail> {
        let v = body(b)?;
        let sim = self.find(v.get("sim").and_then(Value::as_str))?;
        let act = part_act(&v)?;
        let mut r = lock(&sim.run);
        if r.err.is_some() {
            return Err(fail(409, "the simulation stopped with an error; start a new one"));
        }
        r.catch_up();
        let r = &mut *r;
        let w = r.plant.work_mut().ok_or_else(|| fail(400, "the scene has no workpieces"))?;
        let target = match &act {
            Act::Put { at, .. } | Act::Move { at, .. } | Act::Remove(at) => Some(at),
            _ => None,
        };
        if let Some(x) = target.filter(|x| !w.target_exists(x)) {
            return Err(fail(400, format!("no point \"{}\"", x.chars().take(24).collect::<String>())));
        }
        w.check_act(&act).map_err(|e| fail(400, e))?;
        w.apply(&act);
        w.sense_now(&mut r.io);
        r.last = Instant::now();
        Ok(r.json(&sim.id, &sim.source))
    }

    fn stop(&self, b: &[u8]) -> Result<Value, Fail> {
        let v = body(b)?;
        let id = v.get("sim").and_then(Value::as_str).ok_or_else(|| fail(400, "sim is needed"))?;
        let mut list = lock(&self.list);
        let n = list.len();
        list.retain(|s| s.id != id);
        if list.len() == n {
            return Err(fail(404, "unknown simulation (it was stopped or replaced)"));
        }
        Ok(json!({"ok": true}))
    }
}
