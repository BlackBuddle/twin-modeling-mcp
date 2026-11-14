//! 트윈 모델(reusable-parts 3.1d-1, 설계 46의 2절): PLC 주소가 없는 형식 2 장면. 이 파일은 세 가지를 한다.
//! (1) `check`: 트윈 모델이 두지 않는 키를 고정된 글로 거절한다(2.3절)와 핀 개수 한도(2.7절).
//! (2) `pins`: 장면의 핀 목록(`facts.pins`, 2.4절의 순서).
//! (3) `sim`: IL 프로그램 없이 핀 사건으로 설비를 돌린다(`model_sim`, 3.3절). 시계가 없으므로 실제 시간 상한은 호스트가 콜백으로 준다.
//! 장면 읽기는 `scene::parse_with(.., &NO_ADDRESSES)`이다(모든 칸과 신호가 핀 이름을 주소로 가진다). OS 호출과 PLC 코드가 없다.

use super::bind::NameIo;
use super::plant::{Mode, Plant};
use super::run::{PartScript, Recorder};
use super::scene::{self, Dev, Scene, Slot};
use serde_json::{json, Value};

/// 한 모델의 핀 개수 한도(실제 가장 큰 경우의 추정: 칸 5개짜리 장치 200개)
pub const MAX_PINS: usize = 1024;
pub const MAX_SIM_MS: i64 = 60_000;
pub const MAX_EVENTS: usize = 100;
pub const MAX_WATCH: usize = 40;
/// 이 글이 `model_sim`의 `note`이다(트윈 모델은 PLC가 없으므로 `run::NOTE`의 "PLC" 말을 쓰지 않는다).
pub const SIM_NOTE: &str = "A simulation pass is not a test on real equipment.";

fn dev_err(id: &str, field: &str, msg: &str) -> String {
    format!("device {id}: {field}: {msg}")
}

/// 트윈 모델 검사: 장면 키 `schema`, `photos`, `modules`, 그리고 장치를 목록 순서로, 장치마다 표의 순서로 보고 첫 오류를 돌려준다(`scene::parse`처럼).
/// 모양이 틀린 값(배열이 아닌 `devices` 따위)은 여기서 거절하지 않는다: 그 오류는 `parse_with`의 글이다. 그래서 `parse_with` 앞에서 부른다.
pub fn check(v: &Value) -> Result<(), String> {
    let Some(o) = v.as_object() else { return Ok(()) };
    if o.get("schema").and_then(Value::as_i64) != Some(2) {
        return Err("scene.schema: a twin model is schema 2".into());
    }
    if o.get("photos").is_some_and(|p| !p.is_null()) {
        return Err("scene.photos: a twin model names no photo file (photos stay with the person)".into());
    }
    if o.get("modules").is_some_and(|p| !p.is_null()) {
        return Err("scene.modules: a module model is a PLC feature; plc-mcp adds it when it binds the model".into());
    }
    let kinds = scene::catalog()["kinds"].as_object();
    let mut pins = 0usize;
    for d in o.get("devices").and_then(Value::as_array).into_iter().flatten() {
        let Some(d) = d.as_object() else { continue };
        let Some(id) = d.get("id").and_then(Value::as_str) else { continue };
        let set = |k: &str| d.get(k).filter(|v| !v.is_null());
        if set("io").is_some() {
            return Err(dev_err(id, "io", &format!("a twin model holds no PLC address; leave io out (its slots are the pins {id}.<slot>, bound by plc-mcp)")));
        }
        let signals: &[Value] = set("signals").and_then(Value::as_array).map_or(&[], Vec::as_slice);
        for (i, s) in signals.iter().enumerate() {
            if s.get("device").is_some_and(|x| !x.is_null()) {
                let n = s.get("name").and_then(Value::as_str).filter(|n| scene::split_pin(&format!("{id}.{n}")).is_some()).unwrap_or("<name>");
                return Err(dev_err(id, &format!("signals[{i}].device"), &format!("a twin model holds no PLC address; leave device out (the pin is {id}.{n})")));
            }
        }
        // `show`·`hide`·`indicators`는 핀 형식만 둔다(tasks.md: 핀 형식 결정). 주소 형식은 거절한다.
        for (gi, g) in set("skin").and_then(|s| s.get("groups")).and_then(Value::as_array).into_iter().flatten().enumerate() {
            for key in ["show", "hide"] {
                if g.get(key).is_some_and(|x| !x.is_null() && !x.as_str().is_some_and(|t| scene::split_pin(t).is_some())) {
                    return Err(dev_err(id, &format!("skin.groups[{gi}].{key}"), "a twin model holds no PLC address; use a pin \"<device id>.<signal>\""));
                }
            }
        }
        for (ri, r) in set("indicators").and_then(Value::as_array).into_iter().flatten().enumerate() {
            if ["from", "count"].iter().any(|k| r.get(*k).is_some_and(|x| !x.is_null())) {
                return Err(dev_err(id, &format!("indicators[{ri}].from"), "a twin model holds no PLC address; use pins: [\"<device id>.<signal>\", ...]"));
            }
        }
        if set("position").is_some() {
            return Err(dev_err(id, "position", "a twin model holds no PLC address; use mech with the preset servo_screw and a word signal"));
        }
        let io_spec = d.get("kind").and_then(Value::as_str).and_then(|k| kinds?.get(k)).and_then(|k| k["io"].as_object());
        let kname = d.get("kind").and_then(Value::as_str).unwrap_or("");
        for (i, s) in signals.iter().enumerate() {
            if let Some(n) = s.get("name").and_then(Value::as_str).filter(|n| io_spec.is_some_and(|m| m.contains_key(*n))) {
                return Err(scene::pin_clash(id, kname, i, n));
            }
        }
        pins += io_spec.map_or(0, |m| m.len()) + signals.len();
    }
    if pins > MAX_PINS {
        return Err(format!("scene: more than {MAX_PINS} pins"));
    }
    Ok(())
}

fn pin_type(x: &Slot) -> &'static str {
    if x.bcd {
        "bcd"
    } else if x.word {
        "word"
    } else {
        "bit"
    }
}

/// `facts.pins`: 장치 순서, 그 안에서 종류의 칸 순서(목록 `catalog.kinds.<종류>.io`의 키 순서), 그 뒤 신호 순서.
/// `[{"pin": "screw.target", "dir": "out", "type": "word"}, ...]`; 방향은 PLC에서 본 쪽(`out` = 사람이나 사건이 정함, `in` = 설비가 씀).
pub fn pins(s: &Scene) -> Vec<Value> {
    s.devices
        .iter()
        .flat_map(|d| d.slots.iter().chain(&d.signals).map(move |x| json!({"pin": format!("{}.{}", d.id, x.name), "dir": if x.out { "out" } else { "in" }, "type": pin_type(x)})))
        .collect()
}

/// 핀 하나를 장면에서 찾는다. 오류 글은 앞말 없이 돌려준다(부르는 쪽이 `events[0].pin: `이나 `watch[1]: `을 붙인다).
fn find_pin<'a>(s: &'a Scene, text: &str) -> Result<(&'a Dev, &'a Slot), String> {
    let (id, name) = scene::split_pin(text).ok_or("must be a pin \"<device id>.<name>\"")?;
    let list = |v: Vec<String>| {
        let more = if v.len() > 16 { format!(" and {} more", v.len() - 16) } else { String::new() };
        format!("{}{more}", v.into_iter().take(16).collect::<Vec<_>>().join(", "))
    };
    let Some(d) = s.devices.iter().find(|d| d.id == id) else {
        return Err(format!("no device \"{}\" (devices: {})", id.chars().take(32).collect::<String>(), list(s.devices.iter().map(|d| d.id.clone()).collect())));
    };
    match d.slots.iter().chain(&d.signals).find(|x| x.name == name) {
        Some(x) => Ok((d, x)),
        None => {
            let mut v: Vec<String> = d.slots.iter().chain(&d.signals).map(|x| format!("{id}.{}", x.name)).collect();
            v.sort();
            Err(format!("no pin \"{id}.{}\" (pins of {id}: {})", name.chars().take(24).collect::<String>(), list(v)))
        }
    }
}

/// 이 핀을 32비트로 읽는 구동기가 있는가(그러면 값 범위가 32비트이다)
fn wide(d: &Dev, x: &Slot) -> bool {
    let Some(m) = &d.mech else { return false };
    let idx = d.slots.iter().chain(&d.signals).position(|y| y.name == x.name);
    m.drivers.iter().any(|dr| matches!(dr, super::mech::Driver::Follow { word, width: 32, .. } if Some(*word) == idx))
}

/// BCD 4자리의 워드(12 → 0x0012)
fn to_bcd(n: i64) -> i64 {
    (0..4).fold(0, |w, k| w | ((n / 10i64.pow(k)) % 10) << (4 * k))
}

/// 핀 사건 하나(검사를 마친 값: 비트는 0 또는 1, bcd 핀은 BCD 워드). 페이지 시뮬레이션이 `last_run`을 다시 돌릴 때도 쓴다.
pub struct PinEvent {
    pub t: i64,
    pub pin: String,
    pub word: bool,
    pub value: i64,
}

pub fn parse_events(s: &Scene, v: Option<&Value>) -> Result<Vec<PinEvent>, String> {
    let Some(v) = v.filter(|v| !v.is_null()) else { return Ok(Vec::new()) };
    let a = v.as_array().filter(|a| a.len() <= MAX_EVENTS).ok_or(format!("events: must be an array of at most {MAX_EVENTS}"))?;
    let mut out = Vec::new();
    for (i, e) in a.iter().enumerate() {
        let e = e.as_object().ok_or(format!("events[{i}]: must be an object"))?;
        if let Some(k) = e.keys().find(|k| !matches!(k.as_str(), "t_ms" | "pin" | "value")) {
            return Err(format!("events[{i}].{}: unknown field", k.chars().take(20).collect::<String>()));
        }
        let t = e.get("t_ms").and_then(Value::as_i64).filter(|t| (0..=MAX_SIM_MS).contains(t)).ok_or(format!("events[{i}].t_ms: must be an integer from 0 to {MAX_SIM_MS}"))?;
        let pin = e.get("pin").and_then(Value::as_str).ok_or(format!("events[{i}].pin: must be text \"<device id>.<name>\""))?;
        let (d, x) = find_pin(s, pin).map_err(|m| format!("events[{i}].pin: {m}"))?;
        if !x.out {
            return Err(format!("events[{i}].pin: {pin} is written by the plant (dir in); an event sets a pin with dir out"));
        }
        let val = e.get("value").and_then(|v| v.as_i64().or_else(|| v.as_bool().map(i64::from))).ok_or(format!("events[{i}].value: must be an integer or true or false"))?;
        let value = if !x.word {
            i64::from(val != 0)
        } else if x.bcd {
            if !(0..=9999).contains(&val) {
                return Err(format!("events[{i}].value: {val} is not a number 0 to 9999 for the bcd pin {pin}"));
            }
            to_bcd(val)
        } else if wide(d, x) {
            if i32::try_from(val).is_err() {
                return Err(format!("events[{i}].value: {val} is outside the 32-bit range of {pin} (-2147483648 to 2147483647)"));
            }
            val
        } else {
            if i16::try_from(val).is_err() {
                return Err(format!("events[{i}].value: {val} is outside the 16-bit range of {pin} (-32768 to 32767)"));
            }
            val
        };
        out.push(PinEvent { t, pin: pin.to_string(), word: x.word, value });
    }
    Ok(out)
}

/// 감시 핀: 이름은 글 그대로, 값은 비트이면 0 또는 1, 워드이면 그 값
fn parse_watch(s: &Scene, v: Option<&Value>) -> Result<Vec<(String, bool)>, String> {
    match v.filter(|v| !v.is_null()) {
        Some(v) => v
            .as_array()
            .filter(|a| a.len() <= MAX_WATCH)
            .ok_or(format!("watch: must be an array of at most {MAX_WATCH} pins"))?
            .iter()
            .enumerate()
            .map(|(k, p)| {
                let p = p.as_str().ok_or(format!("watch[{k}]: must be text \"<device id>.<name>\""))?;
                find_pin(s, p).map(|(_, x)| (p.to_string(), x.word)).map_err(|m| format!("watch[{k}]: {m}"))
            })
            .collect(),
        // 안 적으면 설비가 쓰는 비트 핀(`dir: in`)의 처음 40개를 이름 순서로
        None => {
            let mut v: Vec<String> = s.devices.iter().flat_map(|d| d.slots.iter().chain(&d.signals).filter(|x| !x.out && !x.word).map(move |x| format!("{}.{}", d.id, x.name))).collect();
            v.sort();
            Ok(v.into_iter().take(MAX_WATCH).map(|p| (p, false)).collect())
        }
    }
}

/// `part_events`: `twin_sim_run`과 같은 항목(항목마다 `source`, `at`, `pos`, `remove`, `reset` 중 하나). 색 이름은 여기서 본다.
pub fn parse_part_events(v: Option<&Value>) -> Result<Vec<(i64, super::work::Act)>, String> {
    use super::work::Act;
    let Some(v) = v.filter(|v| !v.is_null()) else { return Ok(Vec::new()) };
    let a = v.as_array().filter(|a| a.len() <= MAX_EVENTS).ok_or(format!("part_events must be an array of at most {MAX_EVENTS}"))?;
    let text = |e: &Value, k: &str, i: usize| -> Result<Option<String>, String> {
        match e.get(k).filter(|x| !x.is_null()) {
            None => Ok(None),
            Some(x) => x.as_str().filter(|s| s.len() <= 32).map(|s| Some(s.to_string())).ok_or(format!("part_events[{i}]: {k} must be text of at most 32 characters")),
        }
    };
    a.iter()
        .enumerate()
        .map(|(i, e)| {
            let o = e.as_object().ok_or(format!("part_events[{i}]: must be an object"))?;
            if let Some(k) = o.keys().find(|k| !matches!(k.as_str(), "t_ms" | "source" | "type" | "tone" | "at" | "s_mm" | "pos" | "remove" | "reset")) {
                return Err(format!("part_events[{i}].{}: unknown field", k.chars().take(20).collect::<String>()));
            }
            let t = e.get("t_ms").and_then(Value::as_i64).filter(|t| (0..=MAX_SIM_MS).contains(t)).ok_or("each part event needs t_ms (integer, 0 to 60000)")?;
            let (ty, tone) = (text(e, "type", i)?, text(e, "tone", i)?);
            if let Some(n) = tone.as_deref() {
                if !super::work::TONE_TEXT.split(", ").any(|x| x == n) {
                    return Err(format!("part_events[{i}]: tone must be one of {}", super::work::TONE_TEXT));
                }
            }
            let reset = o.get("reset").is_some_and(|r| r == &Value::Bool(true));
            let given = ["source", "at", "pos", "remove"].iter().filter(|k| o.get(**k).is_some_and(|x| !x.is_null())).count() + usize::from(reset);
            if given != 1 {
                return Err(format!("part_events[{i}]: give one of source, at, pos, remove, reset"));
            }
            let s_mm = match o.get("s_mm").filter(|x| !x.is_null()) {
                None => None,
                Some(x) => Some(x.as_f64().filter(|n| n.is_finite()).ok_or(format!("part_events[{i}]: s_mm must be a number"))?),
            };
            let act = if reset {
                Act::Reset
            } else if let Some(s) = text(e, "source", i)? {
                Act::Source { source: s, ty, tone }
            } else if let Some(x) = text(e, "remove", i)? {
                Act::Remove(x)
            } else if let Some(x) = text(e, "at", i)? {
                Act::Put { at: x, ty, tone, s_mm }
            } else {
                let p = o.get("pos").and_then(|p| super::geom::vec3(p, 100_000.0)).ok_or(format!("part_events[{i}]: pos must be 3 numbers from -100000 to 100000"))?;
                Act::PutPos { pos: p, ty, tone }
            };
            Ok((t, act))
        })
        .collect()
}

/// 트윈 모델의 한 번 실행(`model_sim`): IL 프로그램이 없다. 스캔 하나는 사건(시각이 온 것), 공작물 사건, 표본, 감시 기록, 설비 틱의 차례이다
/// (`twin_sim_run`의 차례에서 프로그램 스캔만 빠졌다). 그래서 같은 핀 값이 같은 시각에 오면 두 호스트의 `joints`와 `positions`가 같다.
/// `args`: `events`(`{t_ms, pin, value}`, 100개), `part_events`, `total_ms`(10~60000, 기본 1000), `scan_ms`(1~100, 기본 10), `watch`(핀 40개; 기본은 `dir: in` 비트 핀).
/// `over_wall`: 스캔 하나가 끝날 때마다 부른다. 참이면 실제 시간 상한을 넘은 것이다(시계는 호스트의 것).
/// 결과는 `twin_sim_run`의 키(`simulated`, `scene`, `scan_ms`, `total_ms`, `watch`, `changes`, `positions`, `joints`, ...)이다. `plc_stopped`와 `modules`는 없다.
pub fn sim(scene: &Scene, args: &Value, over_wall: &mut dyn FnMut() -> bool) -> Result<Value, String> {
    let o = args.as_object().ok_or("arguments must be an object")?;
    if let Some(k) = o.keys().find(|k| !matches!(k.as_str(), "name" | "events" | "part_events" | "total_ms" | "scan_ms" | "watch")) {
        return Err(format!("unknown argument {}", k.chars().take(20).collect::<String>()));
    }
    let total = match o.get("total_ms") {
        None | Some(Value::Null) => 1000,
        Some(v) => v.as_i64().filter(|t| (10..=MAX_SIM_MS).contains(t)).ok_or(format!("total_ms must be an integer from 10 to {MAX_SIM_MS}"))?,
    };
    let scan = match o.get("scan_ms") {
        None | Some(Value::Null) => 10,
        Some(v) => v.as_i64().filter(|t| (1..=100).contains(t)).ok_or("scan_ms must be an integer from 1 to 100")?,
    };
    let mut events = parse_events(scene, o.get("events"))?;
    events.sort_by_key(|e| e.t);
    let part_events = parse_part_events(o.get("part_events"))?;
    let watch = parse_watch(scene, o.get("watch"))?;

    let mut io = NameIo::new();
    let mut plant = Plant::new(scene, Mode::Sim);
    let mut script = PartScript::new(&plant, part_events)?;
    script.begin(&mut plant)?;
    plant.init(&mut io);
    script.after_init(&mut plant, &mut io)?;
    let mut rec = Recorder::new(watch.iter().map(|(n, _)| n.clone()).collect());
    let (mut t, mut i) = (0i64, 0usize);
    while t < total {
        use super::bind::PlantIo;
        while i < events.len() && events[i].t <= t {
            let e = &events[i];
            if e.word {
                io.set_word(&e.pin, e.value);
            } else {
                io.set_bit(&e.pin, e.value != 0);
                // 부품 감지 규칙이 쓰는 비트에 사건이 오면 그때부터 규칙은 그 비트를 쓰지 않는다
                if let Some(r) = plant.work_mut().filter(|r| r.sense_bit(&e.pin)) {
                    r.force(&e.pin);
                }
            }
            i += 1;
        }
        script.due(&mut plant, t, &mut io)?;
        rec.sample(&plant, t);
        rec.observe(t, watch.iter().map(|(n, w)| if *w { io.word(n) } else { i64::from(io.bit(n)) }).collect());
        plant.tick(&mut io, scan as f64);
        t += scan;
        if over_wall() {
            return Err(format!("the simulation took more than 5 s of real time (stopped at t = {t} ms of {total} ms); make the run shorter or raise scan_ms"));
        }
    }
    let mut result = rec.finish(scene, &plant, scan, total, t);
    Recorder::add_part_result(&mut result, scene, &plant, scan);
    if let Some(r) = result.as_object_mut() {
        r.insert("note".into(), json!(SIM_NOTE));
    }
    Ok(result)
}
