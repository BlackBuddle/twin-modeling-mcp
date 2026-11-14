//! 시뮬레이션 기록기(reusable-parts 3.1c, 설계 46의 4.2절): 닫힌 고리에서 IL 프로그램과 상관없는 부분.
//! `positions`·`joints`·`changes` 표본과 결과 객체의 공통 부분, 공작물 사건 대본(`part_events`)을 가진다.
//! 고리 자체(스캔, 입력 사건, `plc_stopped`, `modules`)는 호스트가 돈다(plc-mcp: `src/twin/simrun.rs`). 시계와 OS 호출이 없다.

use super::bind::PlantIo;
use super::plant::Plant;
use super::scene::Scene;
use super::work::Act;
use serde_json::{json, Value};

pub const NOTE: &str = "Read-only. Does not touch the PLC. A simulation pass is not a test on real equipment.";

const MAX_CHANGES: usize = 500;
const POS_STEP_MS: i64 = 50;
/// 위치 표에 적는 부품 항목 수의 상한(결과 크기 보호)
const MAX_PART_ENTRIES: usize = 4000;
/// 결과 `joints`에 적는 관절 값 개수의 상한(결과 크기 보호)
const MAX_JOINT_VALUES: usize = 20000;

/// 쪽모이 사건 하나를 실행한다. 매거진이 아닌 원천은 형식 1의 `spawn`이다.
pub fn run_act(plant: &mut Plant, act: &Act) -> Result<(), String> {
    match act {
        Act::Source { source, .. } if plant.work().is_none_or(|r| r.stack_of(source).is_none()) => plant.spawn(source).map_err(|e| format!("part_events: {e}")),
        a => {
            if let Some(r) = plant.work_mut() {
                r.check_room(a).map_err(|e| format!("part_events: {e}"))?; // 매거진 20개 한도는 지금의 개수로 본다 (B8 BR3)
                r.apply(a);
            }
            Ok(())
        }
    }
}

/// 공작물 사건 대본: 시각 순서(같은 시각은 적은 순서)로 사건을 내어 준다.
pub struct PartScript {
    pev: Vec<(usize, i64, Act)>,
    pi: usize,
    later: Vec<Act>,
}

impl PartScript {
    /// 항목은 모두 실행 전에 이 장면에 맞는지 본다(원래 번호를 오류 글에 쓴다). 대본이 항목을 가진다(오래 사는 시뮬레이션이 쓴다).
    pub fn new(plant: &Plant, part_events: Vec<(i64, Act)>) -> Result<Self, String> {
        let mut pev: Vec<(usize, i64, Act)> = part_events.into_iter().enumerate().map(|(i, (t, a))| (i, t, a)).collect();
        pev.sort_by_key(|e| e.1);
        for (i, _, a) in &pev {
            match (plant.work(), a) {
                (Some(r), Act::Source { source, .. }) if r.stack_of(source).is_none() => {
                    if !plant.has_source(source) {
                        return Err(format!("part_events[{i}]: source {} is not a part source or stack of the scene", source.chars().take(24).collect::<String>()));
                    }
                }
                (Some(r), a) => r.check_act(a).map_err(|e| format!("part_events[{i}]: {e}"))?,
                (None, Act::Source { .. }) => {}
                (None, _) => return Err(format!("part_events[{i}]: the scene has no workpieces")),
            }
        }
        Ok(PartScript { pev, pi: 0, later: Vec::new() })
    }

    /// t = 0: 매거진 항목은 시작 내용을 바꾼다(첫 항목이 장면의 내용을 지운다). 나머지는 `plant.init` 뒤에 `after_init`가 실행한다.
    pub fn begin(&mut self, plant: &mut Plant) -> Result<(), String> {
        while self.pi < self.pev.len() && self.pev[self.pi].1 <= 0 {
            let a = self.pev[self.pi].2.clone();
            match (plant.work_mut(), &a) {
                (Some(r), Act::Source { source, ty, tone }) if r.stack_of(source).is_some() => {
                    let si = r.stack_of(source).unwrap_or(0);
                    let t = r.type_index(ty.as_deref(), r.refill_type(si))?;
                    r.start_item(si, t, tone.clone());
                }
                _ => self.later.push(a),
            }
            self.pi += 1;
        }
        Ok(())
    }

    /// `plant.init` 뒤: t = 0의 나머지 사건을 실행하고, 부품 센서를 한 번 센다.
    pub fn after_init(&mut self, plant: &mut Plant, io: &mut dyn PlantIo) -> Result<(), String> {
        for a in std::mem::take(&mut self.later) {
            run_act(plant, &a)?;
        }
        if let Some(r) = plant.work_mut() {
            r.sense_now(io);
        }
        Ok(())
    }

    /// 시각 `t0`까지 온 사건을 실행한다. 하나라도 있었으면 부품 센서를 다시 센다(프로그램은 다음 스캔에서 바뀐 부품 자리를 본다).
    pub fn due(&mut self, plant: &mut Plant, t0: i64, io: &mut dyn PlantIo) -> Result<(), String> {
        let before = self.pi;
        while self.pi < self.pev.len() && self.pev[self.pi].1 <= t0 {
            run_act(plant, &self.pev[self.pi].2)?;
            self.pi += 1;
        }
        if self.pi > before {
            if let Some(r) = plant.work_mut() {
                r.sense_now(io);
            }
        }
        Ok(())
    }
}

/// 표본 기록기: 50 ms마다 `positions`와 `joints`, 감시 값이 바뀔 때마다 `changes`.
pub struct Recorder {
    watch: Vec<String>,
    next_pos: i64,
    part_entries: usize,
    parts_truncated: bool,
    positions: Vec<Value>,
    joints: Vec<Value>,
    joint_values: usize,
    joints_truncated: bool,
    last: Option<Vec<i64>>,
    changes: Vec<Value>,
    truncated: bool,
}

/// 관절 값 한 항목(`t`와 관절 값들)
fn joint_item(t: i64, jv: Value) -> Value {
    let mut item = serde_json::Map::new();
    item.insert("t".into(), json!(t));
    if let Value::Object(m) = jv {
        item.extend(m);
    }
    Value::Object(item)
}

impl Recorder {
    /// `watch`: 감시 항목의 이름들(결과 `watch`와 `changes`의 `device`)
    pub fn new(watch: Vec<String>) -> Recorder {
        Recorder {
            watch,
            next_pos: 0,
            part_entries: 0,
            parts_truncated: false,
            positions: Vec::new(),
            joints: Vec::new(),
            joint_values: 0,
            joints_truncated: false,
            last: None,
            changes: Vec::new(),
            truncated: false,
        }
    }

    /// 틱 직전(시각 `t0`)의 표본: 50 ms 걸음마다 위치 표(와 관절 표)에 한 항목을 적는다.
    pub fn sample(&mut self, plant: &Plant, t0: i64) {
        if t0 < self.next_pos {
            return;
        }
        let mut o = serde_json::Map::new();
        o.insert("t".into(), json!(t0));
        for (id, p) in plant.axes() {
            o.insert(id.to_string(), json!((p * 1000.0).round() / 1000.0));
        }
        let r = |x: f64| (x * 10.0).round() / 10.0;
        let ps: Vec<Value> = plant.parts().take(MAX_PART_ENTRIES.saturating_sub(self.part_entries)).map(|(id, x, z)| json!([id, r(x), r(z)])).collect();
        self.part_entries += ps.len();
        self.parts_truncated |= plant.parts().count() > ps.len();
        if !ps.is_empty() {
            o.insert("parts".into(), Value::Array(ps));
        }
        self.positions.push(Value::Object(o));
        // 관절 값: `positions`와 같은 시각(틱이 끝난 뒤의 값). 값 개수가 상한을 넘으면 더 적지 않는다.
        if plant.has_mech() && !self.joints_truncated {
            let (jv, n) = plant.joints_json();
            if self.joint_values + n > MAX_JOINT_VALUES {
                self.joints_truncated = true;
            } else {
                self.joint_values += n;
                self.joints.push(joint_item(t0, jv));
            }
        }
        self.next_pos += POS_STEP_MS;
    }

    /// 한 스캔이 읽은 입력(시각 `t0`의 센서)과 낸 출력의 감시 값 `now`(감시 순서대로). 처음이거나 바뀐 항목만 적는다.
    pub fn observe(&mut self, t0: i64, now: Vec<i64>) {
        if self.last.as_ref() != Some(&now) {
            for (k, name) in self.watch.iter().enumerate() {
                if self.last.as_ref().map_or(now[k] != 0, |l| l[k] != now[k]) {
                    if self.changes.len() < MAX_CHANGES {
                        self.changes.push(json!({"t": t0, "device": name, "value": now[k]}));
                    } else {
                        self.truncated = true;
                    }
                }
            }
            self.last = Some(now);
        }
    }

    /// 결과 객체의 공통 부분. `end_t`는 끝 시각이다. 호스트가 `plc_stopped`와 `modules` 같은 자기 키를 더한다.
    pub fn finish(mut self, scene: &Scene, plant: &Plant, scan: i64, total: i64, end_t: i64) -> Value {
        // 마지막 틱 뒤의 관절 값도 한 항목으로 적는다(시각은 끝 시각): 한 주기가 끝난 자리를 볼 수 있게
        if plant.has_mech() && !self.joints_truncated {
            let (jv, n) = plant.joints_json();
            if self.joint_values + n <= MAX_JOINT_VALUES {
                self.joints.push(joint_item(end_t, jv));
            } else {
                self.joints_truncated = true;
            }
        }
        let mut result = json!({
            "simulated": true,
            "scene": scene.name,
            "scan_ms": scan,
            "total_ms": total,
            "watch": self.watch,
            "changes": self.changes,
            "changes_truncated": self.truncated,
            "positions": self.positions,
            "alarms": plant.alarms().iter().map(|a| a.to_json()).collect::<Vec<_>>(),
            "alarms_dropped": plant.alarms_dropped(),
            "parts_final": plant.state_json()["parts"],
            "parts_dropped": plant.parts_dropped(),
            "parts_truncated": self.parts_truncated,
            "note": NOTE
        });
        // `mech`가 있는 장면만 관절 값을 낸다(없는 장면의 결과는 그대로이다)
        if let (true, Some(r)) = (plant.has_mech(), result.as_object_mut()) {
            r.insert("joints".into(), Value::Array(self.joints));
            if self.joints_truncated {
                r.insert("joints_truncated".into(), json!(true));
            }
        }
        // 형식 2 장면: 그려지기만 하는 장치와 규칙의 한계를 알린다(형식 1의 결과는 그대로이다)
        if scene.schema == 2 {
            if let (Some(r), Some(extra)) = (result.as_object_mut(), super::geom::sim_notes(scene).as_object()) {
                r.extend(extra.iter().map(|(k, v)| (k.clone(), v.clone())));
            }
        }
        result
    }

    /// 공작물 규칙의 결과를 더한다(`finish` 뒤에 부른다): 부품 기록, 남은 매거진, 규칙이 쓰지 않은 비트, 펄스가 스캔보다 짧다는 한계.
    pub fn add_part_result(result: &mut Value, scene: &Scene, plant: &Plant, scan: i64) {
        if let (Some(w), Some(r)) = (plant.work(), result.as_object_mut()) {
            r.insert("part_log".into(), json!(w.log));
            r.insert("part_log_truncated".into(), json!(w.log_truncated));
            r.insert("stacks_final".into(), w.stacks_json());
            r.insert("forced".into(), json!(w.forced_names()));
            let lines = w.pulse_limits(scan as f64, |b| scene.devices[b].num("speed_mm_s"));
            if let Some(l) = r.get_mut("limits").and_then(Value::as_array_mut) {
                l.extend(lines.into_iter().map(Value::from));
            }
        }
    }
}
