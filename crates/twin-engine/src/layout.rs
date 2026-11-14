//! 부품, 포트, 연결(links), 배치 경고(작업 1.6). 그림과 흐름 표시용이다: 설비 모델(plant)은 이것을 읽지 않는다.
//! 경고는 저장을 막지 않는다.

use super::scene::{catalog, Dev, Scene};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};

pub const MAX_LINKS: usize = 400;
const TOL_MM: f64 = 5.0;
const LONG_LINK_MM: f64 = 2000.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    pub from: String,
    pub to: String,
    pub kind: String,
}

impl Link {
    pub fn to_json(&self) -> Value {
        json!({"from": self.from, "to": self.to, "kind": self.kind})
    }
}

fn kind_layout(kind: &str) -> Option<&'static Value> {
    catalog()["layout"].get(kind)
}

/// 종류의 포트 하나: (포트 종류, 부품 이름, 부품 안 위치)
fn port_of(kind: &str, port: &str) -> Option<(&'static str, &'static str, [f64; 3])> {
    let p = kind_layout(kind)?["ports"].as_array()?.iter().find(|p| p["name"] == port)?;
    let at = p["at"].as_array()?;
    Some((p["kind"].as_str()?, p["part"].as_str()?, [at[0].as_f64()?, at[1].as_f64()?, at[2].as_f64()?]))
}

/// 부품 이름이 종류에 있는지
pub fn has_part(kind: &str, part: &str) -> bool {
    kind_layout(kind).and_then(|l| l["parts"].as_array()).is_some_and(|a| a.iter().any(|p| p["name"] == part))
}

fn rot_y(deg: f64, x: f64, z: f64) -> (f64, f64) {
    let r = deg.to_radians();
    (x * r.cos() + z * r.sin(), -x * r.sin() + z * r.cos())
}

/// 부품의 바닥 중심(세계 좌표)과 크기
fn part_world(d: &Dev, name: &str, size: [f64; 3], default_pos: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let off = d.parts.get(name).copied().unwrap_or(default_pos);
    let (rx, rz) = rot_y(d.rot_y, off[0], off[2]);
    ([d.pos[0] + rx, d.pos[1] + off[1], d.pos[2] + rz], size)
}

/// 장치의 축 정렬 상자 (min, max). 회전은 y축만 본다(부품 상자를 감싸는 상자).
/// (plc-mcp의 골든 시험이 읽으므로 `pub`이다.)
pub fn aabb(d: &Dev) -> Option<([f64; 3], [f64; 3])> {
    let parts = kind_layout(&d.kind)?["parts"].as_array()?;
    let (s, c) = (d.rot_y.to_radians().sin().abs(), d.rot_y.to_radians().cos().abs());
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for p in parts {
        let g = |k: &str, i: usize| p[k][i].as_f64().unwrap_or(0.0);
        let (o, size) = part_world(d, p["name"].as_str()?, [g("size", 0), g("size", 1), g("size", 2)], [g("pos", 0), g("pos", 1), g("pos", 2)]);
        let (hx, hz) = ((c * size[0] + s * size[2]) / 2.0, (s * size[0] + c * size[2]) / 2.0);
        for (i, (a, b)) in [(o[0] - hx, o[0] + hx), (o[1], o[1] + size[1]), (o[2] - hz, o[2] + hz)].into_iter().enumerate() {
            lo[i] = lo[i].min(a);
            hi[i] = hi[i].max(b);
        }
    }
    Some((lo, hi))
}

/// 포트의 세계 좌표
fn port_pos(d: &Dev, port: &str) -> Option<[f64; 3]> {
    let (_, part, at) = port_of(&d.kind, port)?;
    let p = kind_layout(&d.kind)?["parts"].as_array()?.iter().find(|p| p["name"] == part)?;
    let g = |k: &str, i: usize| p[k][i].as_f64().unwrap_or(0.0);
    let (o, _) = part_world(d, part, [0.0; 3], [g("pos", 0), g("pos", 1), g("pos", 2)]);
    let (rx, rz) = rot_y(d.rot_y, at[0], at[2]);
    Some([o[0] + rx, o[1] + at[1], o[2] + rz])
}

/// `links` JSON을 읽고 검사한다. 오류 글은 링크 끝 이름을 말한다.
pub fn parse_links(v: Option<&Value>, devices: &[Dev]) -> Result<Vec<Link>, String> {
    let a = match v {
        None | Some(Value::Null) => return Ok(Vec::new()),
        Some(Value::Array(a)) if a.len() <= MAX_LINKS => a,
        Some(_) => return Err(format!("scene.links: must be an array of at most {MAX_LINKS}")),
    };
    let mut out = Vec::new();
    for (i, l) in a.iter().enumerate() {
        let o = l.as_object().ok_or(format!("scene.links[{i}]: must be an object"))?;
        if let Some(k) = o.keys().find(|k| !matches!(k.as_str(), "from" | "to" | "kind")) {
            return Err(format!("scene.links[{i}].{k}: unknown field"));
        }
        let kind = o.get("kind").and_then(Value::as_str).filter(|k| matches!(*k, "air" | "wire")).ok_or(format!("scene.links[{i}].kind: must be \"air\" or \"wire\""))?;
        let mut ends = Vec::new();
        for f in ["from", "to"] {
            let t = o.get(f).and_then(Value::as_str).filter(|t| t.len() <= 80).ok_or(format!("scene.links[{i}].{f}: must be \"<device>.<port>\""))?;
            let short: String = t.chars().take(64).collect();
            let (dev, port) = t.split_once('.').ok_or(format!("scene.links[{i}].{f}: `{short}` must be \"<device>.<port>\""))?;
            let d = devices.iter().find(|d| d.id == dev).ok_or(format!("scene.links[{i}].{f}: unknown device in `{short}`"))?;
            let (pk, _, _) = port_of(&d.kind, port).ok_or(format!("scene.links[{i}].{f}: unknown port `{short}` (ports of {}: {})", d.kind, port_names(&d.kind)))?;
            if pk != kind {
                return Err(format!("scene.links[{i}].{f}: port `{short}` is {pk}, but the link is {kind}"));
            }
            ends.push(t.to_string());
        }
        out.push(Link { from: ends[0].clone(), to: ends[1].clone(), kind: kind.into() });
    }
    Ok(out)
}

fn port_names(kind: &str) -> String {
    kind_layout(kind).and_then(|l| l["ports"].as_array()).map(|a| a.iter().filter_map(|p| p["name"].as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default()
}

/// 실제 연결 + 기본 연결. 밸브가 있는 장치에 밸브 포트 연결이 하나도 없으면 valve_a->cap, valve_b->rod를 더한다.
pub fn effective_links(s: &Scene) -> Vec<Link> {
    let mut out = s.links.clone();
    for d in s.devices.iter().filter(|d| d.has("valve")) {
        let prefix = format!("{}.valve_", d.id);
        if s.links.iter().any(|l| l.from.starts_with(&prefix) || l.to.starts_with(&prefix)) {
            continue;
        }
        for (a, b) in [("valve_a", "cap"), ("valve_b", "rod")] {
            if port_of(&d.kind, a).is_some() && port_of(&d.kind, b).is_some() {
                out.push(Link { from: format!("{}.{a}", d.id), to: format!("{}.{b}", d.id), kind: "air".into() });
            }
        }
    }
    out
}

fn overlap_mm(a: (f64, f64), b: (f64, f64)) -> f64 {
    a.1.min(b.1) - a.0.max(b.0)
}

/// 겹침 경고는 처음 이만큼만 담고 나머지는 개수로 알린다.
const MAX_OVERLAPS: usize = 100;

/// 배치 경고: overlap, floating, long_link. 저장을 막지 않는다.
pub fn warnings(s: &Scene) -> Vec<Value> {
    let boxes: Vec<Option<([f64; 3], [f64; 3])>> = s.devices.iter().map(aabb).collect();
    let mut out = Vec::new();
    let mut more = 0u64;
    for i in 0..boxes.len() {
        let Some(a) = boxes[i] else { continue };
        for j in i + 1..boxes.len() {
            let Some(b) = boxes[j] else { continue };
            if (0..3).all(|k| overlap_mm((a.0[k], a.1[k]), (b.0[k], b.1[k])) > TOL_MM) {
                if out.len() < MAX_OVERLAPS {
                    out.push(json!({"kind": "overlap", "devices": [s.devices[i].id, s.devices[j].id]}));
                } else {
                    more += 1;
                }
            }
        }
    }
    if more > 0 {
        out.push(json!({"kind": "overlap_more", "count": more}));
    }
    for (i, d) in s.devices.iter().enumerate() {
        let Some(a) = boxes[i] else { continue };
        if a.0[1] <= TOL_MM {
            continue;
        }
        // 다른 장치의 윗면 위에 있으면 떠 있는 것이 아니다.
        let on_top = boxes.iter().enumerate().any(|(j, b)| j != i && b.is_some_and(|b| (b.1[1] - a.0[1]).abs() <= TOL_MM && overlap_mm((a.0[0], a.1[0]), (b.0[0], b.1[0])) > 0.0 && overlap_mm((a.0[2], a.1[2]), (b.0[2], b.1[2])) > 0.0));
        if !on_top {
            out.push(json!({"kind": "floating", "devices": [d.id]}));
        }
    }
    let by_id: BTreeMap<&str, &Dev> = s.devices.iter().map(|d| (d.id.as_str(), d)).collect();
    let mut seen = HashSet::new();
    for l in effective_links(s) {
        let pos = |t: &str| t.split_once('.').and_then(|(d, p)| by_id.get(d).and_then(|d| port_pos(d, p)));
        if let (Some(a), Some(b)) = (pos(&l.from), pos(&l.to)) {
            let dist = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
            if dist > LONG_LINK_MM && seen.insert((l.from.clone(), l.to.clone())) {
                out.push(json!({"kind": "long_link", "link": {"from": l.from, "to": l.to}, "mm": dist.round()}));
            }
        }
    }
    out
}

/// 장면 JSON 하나의 `links`(기본 연결 포함)와 `warnings`. 검사에 실패하면 None.
/// 주소의 규칙은 호스트가 준다(plc-mcp의 `twin::layout::report`가 자기 규칙을 넣어 부른다).
pub fn report_with(name: &str, v: &Value, addrs: &'static dyn super::bind::Addresses) -> Option<Value> {
    let s = super::scene::parse_with(name, v, addrs).ok()?;
    Some(json!({"links": effective_links(&s).iter().map(Link::to_json).collect::<Vec<_>>(), "warnings": warnings(&s)}))
}
