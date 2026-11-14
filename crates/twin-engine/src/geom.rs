//! 장면 형식 2의 기하 핵심(마일스톤 A, 작업 A1과 A2): 회전, 변환, 크기(dims), 표준 틀(frame), 장치 나무(`on`), 세계 좌표,
//! `facts`, `resolved`, `ask`, `not_simulated`. 규칙은 `docs/twin-api.md` 14절과 `docs/research/photo-twin/10-architecture.md` 2절.
//! 장면 파일은 믿지 않는 글이므로 재귀 없이(AD15) 개수를 먼저 검사하고, 모든 나눗셈과 색인을 확인한다. 이 폴더는 PLC에 쓰지 않는다.

use super::scene::{catalog, Scene};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

pub type V3 = [f64; 3];

pub const MAX_DEPTH: usize = 16;
pub const MAX_SHAPE_ITEMS: usize = 24;
pub const MAX_SHAPE_SCENE: usize = 1000;
pub const MAX_FRAMES: usize = 16;
pub const MAX_BASIS: usize = 32;
pub const MAX_REFS: usize = 16;
pub const MAX_POSE: f64 = 100_000.0;
const MAX_DIM: f64 = 20_000.0;
const MAX_FACT_LINES: usize = 60;

/// 틀 목록의 "형태(form)" 이름. 카탈로그의 `kinds[k].form`과 `mounts[].forms`는 이 이름만 쓴다(시험이 확인한다).
pub const FORMS: &[&str] = &["box", "cyl", "jaw", "linear", "frame", "belt", "robot6", "table", "solid", "mech"];
/// `basis` 값의 닫힌 목록
pub const BASIS_VALUES: &[&str] = &["user", "label", "measured", "ref", "guess"];
/// 사용자 틀 이름으로 쓸 수 없는 표준 틀 이름(다른 형태의 것도 포함: 뒤 작업이 같은 이름을 만든다)
const STANDARD: &[&str] = &["base", "top", "bottom", "front", "back", "left", "right", "mid", "cap_end", "head_end", "pivot", "out", "carriage", "seat", "face", "grip", "flange", "in", "end", "surface", "top_corner"];
/// 이 빌드에서 `rod_end`가 고정하는 막대의 길이(그리퍼)
const GRIPPER_ROD: f64 = 7.0;

pub const GEOMETRY_NOTE: &str = "Units mm, degrees, ms. Right-handed axes. World +y is up, the floor is y = 0. A frame is an origin and three axes; every device has the frame `base` (for a catalog kind the bottom centre of the main part, +y away from the surface it stands on). rot [rx, ry, rz] means R = Ry(ry) * Rz(rz) * Rx(rx): ry is the heading about the parent +y (a positive angle turns +x toward -z), rz tilts the own +x above the parent x-z plane, rx rolls about the own +x. Test vectors: [0,-90,0] maps +x to [0,0,1]; [0,0,-90] maps +x to [0,-1,0]; [90,0,0] maps +y to [0,0,1] and +z to [0,-1,0]. world(datum of the child) = world(frame `on`) * T(pose.pos) * R(pose.rot); the datum is a frame of the device itself (key `datum`, default from the mount style), so pose places that frame, not `base`. A `surface` frame has +y out of the surface; an `axial` frame has +x out along an axis. A face frame of a box: look at the face from outside with the body upright; +x points right, +z points down, +y points at you. A cylinder or gripper lies along its +x: frames mid, cap_end, head_end, pivot and `out` (the rod end; it moves with the stroke). A belt carries along +x: frames surface, in, end. A linear servo axis moves its carriage along +x: the frame `carriage` moves with the axis value. `seat` is the bottom centre of a box kind with +x = body +y. Sizes come from named params (x_mm, y_mm, z_mm, bore_mm, stroke_mm, body_len_mm, length_mm ...); a size that the scene does not give is derived and `facts.dims` says how (given, series, ratio, default). Schema 2 only: `on` (\"<id>\" or \"<id>.<frame>\"), `mount`, `datum`, `basis`, `model`, kind `solid` with `shape` (box or cyl items) and `frames`, and the scene key `refs` (known lengths between two frames).";

// ───────────── 변환 ─────────────

/// 변환 하나: `r[0..3]`은 자기 x, y, z축(부모 좌표), `p`는 원점. 열 순서 행렬과 같다.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Xf {
    pub r: [V3; 3],
    pub p: V3,
}

pub const IDENT: Xf = Xf { r: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], p: [0.0; 3] };

impl Xf {
    /// 방향 벡터를 부모 좌표로
    pub fn dir(&self, v: V3) -> V3 {
        let r = &self.r;
        [0, 1, 2].map(|k| r[0][k] * v[0] + r[1][k] * v[1] + r[2][k] * v[2])
    }
    pub fn pt(&self, v: V3) -> V3 {
        let d = self.dir(v);
        [d[0] + self.p[0], d[1] + self.p[1], d[2] + self.p[2]]
    }
    /// `self`가 바깥, `o`가 안쪽 변환이다: 점은 먼저 `o`, 다음 `self`를 거친다.
    pub fn mul(&self, o: &Xf) -> Xf {
        Xf { r: [self.dir(o.r[0]), self.dir(o.r[1]), self.dir(o.r[2])], p: self.pt(o.p) }
    }
    /// 역변환(직교 행렬이므로 전치)
    pub fn inv(&self) -> Xf {
        let t = Xf { r: [0, 1, 2].map(|i| [self.r[0][i], self.r[1][i], self.r[2][i]]), p: [0.0; 3] };
        let d = t.dir(self.p);
        Xf { r: t.r, p: [-d[0], -d[1], -d[2]] }
    }
    /// 12개 수: x축, y축, z축, 원점
    pub fn flat(&self) -> Value {
        let v: Vec<Value> = self.r.iter().chain([&self.p]).flatten().map(|x| n(*x)).collect();
        Value::Array(v)
    }
}

/// `rot: [rx, ry, rz]` 도 → R = Ry * Rz * Rx
pub fn rot(a: V3) -> Xf {
    let (cx, sx) = (a[0].to_radians().cos(), a[0].to_radians().sin());
    let (cy, sy) = (a[1].to_radians().cos(), a[1].to_radians().sin());
    let (cz, sz) = (a[2].to_radians().cos(), a[2].to_radians().sin());
    let rx = Xf { r: [[1.0, 0.0, 0.0], [0.0, cx, sx], [0.0, -sx, cx]], p: [0.0; 3] };
    let ry = Xf { r: [[cy, 0.0, -sy], [0.0, 1.0, 0.0], [sy, 0.0, cy]], p: [0.0; 3] };
    let rz = Xf { r: [[cz, sz, 0.0], [-sz, cz, 0.0], [0.0, 0.0, 1.0]], p: [0.0; 3] };
    ry.mul(&rz).mul(&rx)
}

/// 놓기: 위치 `pos` 다음 회전 `rot`(`pose`와 같은 뜻)
pub fn place(pos: V3, a: V3) -> Xf {
    Xf { p: pos, ..rot(a) }
}

/// 보고용 수: 0.001 단위로 반올림하고, 정수면 정수로 낸다(-0과 부동소수 잡음을 없앤다).
pub fn n(x: f64) -> Value {
    let r = (x * 1000.0).round() / 1000.0;
    if r.fract() == 0.0 && r.abs() < 1e12 {
        json!(r as i64)
    } else {
        json!(r)
    }
}

fn nv(v: V3) -> Value {
    json!([n(v[0]), n(v[1]), n(v[2])])
}

fn clean(v: V3) -> V3 {
    v.map(|c| if c.abs() < 1e-9 { 0.0 } else { c })
}

fn dist(a: V3, b: V3) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

// ───────────── 입력 읽기 ─────────────

fn num_in(v: &Value, min: f64, max: f64) -> Option<f64> {
    v.as_f64().filter(|x| x.is_finite() && *x >= min && *x <= max)
}

/// 숫자 3개짜리 배열(각 값이 -max~max)
pub fn vec3(v: &Value, max: f64) -> Option<V3> {
    let a = v.as_array().filter(|a| a.len() == 3)?;
    Some([num_in(&a[0], -max, max)?, num_in(&a[1], -max, max)?, num_in(&a[2], -max, max)?])
}

/// 모양 항목 하나(종류 `solid`의 `shape`). 외형 항목도 이 함수 하나로 읽는다(`look`).
#[derive(Clone, Debug)]
pub struct Item {
    /// 색 이름(`catalog.tones`; 외형 항목만)
    pub tone: Option<String>,
    pub cyl: bool,
    /// 상자의 크기, 원기둥은 틀에서 본 바깥 크기(축 방향이 길이)
    pub size: V3,
    pub d: f64,
    pub d2: f64,
    pub len: f64,
    pub axis: char,
    pub pos: V3,
    pub rot: V3,
}

impl Item {
    pub fn xf(&self) -> Xf {
        place(self.pos, self.rot)
    }
    pub fn to_json(&self) -> Value {
        let mut o = json!({"t": if self.cyl { "cyl" } else { "box" }, "pos": nv(self.pos), "rot": nv(self.rot)});
        if self.cyl {
            o["d"] = n(self.d);
            o["d2"] = n(self.d2);
            o["len"] = n(self.len);
            o["axis"] = json!(self.axis.to_string());
        } else {
            o["size"] = nv(self.size);
        }
        o
    }
}

/// 오류는 (필드 경로, 글)이다. 호출한 쪽이 `device <id>: shape[i].<필드>: <글>`로 만든다.
pub fn parse_item(v: &Value, look: bool) -> Result<Item, (String, String)> {
    let e = |f: &str, m: &str| Err((f.to_string(), m.to_string()));
    let Some(o) = v.as_object() else { return e("", "must be an object") };
    let cyl = match o.get("t").and_then(Value::as_str) {
        Some("box") => false,
        Some("cyl") => true,
        _ => return e("t", "must be box or cyl"),
    };
    let allowed: &[&str] = match (cyl, look) {
        (true, false) => &["t", "d", "len", "d2", "axis", "pos", "rot"],
        (true, true) => &["t", "d", "len", "d2", "axis", "pos", "rot", "tone"],
        (false, false) => &["t", "size", "pos", "rot"],
        (false, true) => &["t", "size", "pos", "rot", "tone"],
    };
    if let Some(k) = o.keys().find(|k| !allowed.contains(&k.as_str())) {
        return e(k, "unknown field");
    }
    let dim = |k: &str| num_in(&o[k], 0.5, MAX_DIM);
    let mut it = Item { tone: None, cyl, size: [0.0; 3], d: 0.0, d2: 0.0, len: 0.0, axis: 'x', pos: [0.0; 3], rot: [0.0; 3] };
    if let Some(t) = o.get("tone") {
        let names: Vec<&str> = catalog()["tones"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        match t.as_str().filter(|t| names.contains(t)) {
            Some(t) => it.tone = Some(t.to_string()),
            None => return e("tone", &format!("must be one of {}", names.join(", "))),
        }
    }
    // 외형 항목의 위치는 틀 안의 값이므로 ±20000, `solid`의 모양 항목은 ±100000
    let pos_max = if look { MAX_DIM } else { MAX_POSE };
    if cyl {
        let (Some(d), Some(len)) = (o.get("d").and_then(|_| dim("d")), o.get("len").and_then(|_| dim("len"))) else {
            return e(if o.get("d").and_then(|_| dim("d")).is_none() { "d" } else { "len" }, "must be 0.5 to 20000");
        };
        let d2 = match o.get("d2") {
            None => d,
            Some(_) => dim("d2").ok_or(("d2".to_string(), "must be 0.5 to 20000".to_string()))?,
        };
        it.axis = match o.get("axis").map(Value::as_str) {
            None => 'x',
            Some(Some("x")) => 'x',
            Some(Some("y")) => 'y',
            Some(Some("z")) => 'z',
            _ => return e("axis", "must be x, y or z"),
        };
        (it.d, it.d2, it.len) = (d, d2, len);
        let w = d.max(d2);
        it.size = match it.axis {
            'x' => [len, w, w],
            'y' => [w, len, w],
            _ => [w, w, len],
        };
    } else {
        let s = o.get("size").and_then(|s| vec3(s, MAX_DIM)).filter(|s| s.iter().all(|c| *c >= 0.5));
        it.size = s.ok_or(("size".to_string(), "must be 3 numbers from 0.5 to 20000".to_string()))?;
    }
    if let Some(p) = o.get("pos") {
        it.pos = vec3(p, pos_max).ok_or(("pos".to_string(), format!("must be 3 numbers from -{pos_max} to {pos_max}")))?;
    }
    if let Some(r) = o.get("rot") {
        it.rot = vec3(r, 360.0).ok_or(("rot".to_string(), "must be 3 numbers from -360 to 360".to_string()))?;
    }
    Ok(it)
}

/// 사용자 틀 하나(`base` 안에 고정)
#[derive(Clone, Debug)]
pub struct Cf {
    pub name: String,
    pub xf: Xf,
    pub axial: bool,
}

fn plain_frame_name(s: &str) -> bool {
    (1..=16).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

fn is_corner(s: &str) -> bool {
    s.len() == 4 && s.starts_with('c') && s[1..].bytes().all(|b| b == b'0' || b == b'1')
}

/// `frames` 객체를 읽는다(개수를 먼저 본다). 오류는 (필드 경로, 글).
pub fn parse_frames(v: &Value) -> Result<Vec<Cf>, (String, String)> {
    let m = v.as_object().ok_or(("".to_string(), "must be an object".to_string()))?;
    if m.len() > MAX_FRAMES {
        return Err(("".into(), format!("at most {MAX_FRAMES}")));
    }
    let mut out = Vec::new();
    for (name, f) in m {
        let short: String = name.chars().take(24).collect();
        if !plain_frame_name(name) {
            return Err((format!("frames.{short}"), "name must be 1 to 16 of a-z 0-9 _".into()));
        }
        if STANDARD.contains(&name.as_str()) || is_corner(name) {
            return Err((format!("frames.{name}"), "a standard frame name".into()));
        }
        let o = f.as_object().ok_or((format!("frames.{name}"), "must be {pos, rot, type}".to_string()))?;
        if let Some(k) = o.keys().find(|k| !matches!(k.as_str(), "pos" | "rot" | "type")) {
            return Err((format!("frames.{name}.{k}"), "unknown field".into()));
        }
        let pos = match o.get("pos") {
            None => [0.0; 3],
            Some(p) => vec3(p, MAX_POSE).ok_or((format!("frames.{name}.pos"), "must be 3 numbers from -100000 to 100000".to_string()))?,
        };
        let ang = match o.get("rot") {
            None => [0.0; 3],
            Some(r) => vec3(r, 360.0).ok_or((format!("frames.{name}.rot"), "must be 3 numbers from -360 to 360".to_string()))?,
        };
        let axial = match o.get("type").map(Value::as_str) {
            None | Some(Some("surface")) => false,
            Some(Some("axial")) => true,
            _ => return Err((format!("frames.{name}.type"), "must be surface or axial".into())),
        };
        out.push(Cf { name: name.clone(), xf: place(pos, ang), axial });
    }
    Ok(out)
}

// ───────────── 크기(dims) ─────────────

/// 종류의 형태. 카탈로그에 `form`이 없으면 `box`.
pub fn form_of(kind: &str) -> &'static str {
    let f = catalog()["kinds"][kind]["form"].as_str().unwrap_or("box");
    FORMS.iter().find(|x| **x == f).copied().unwrap_or("box")
}

/// 크기 하나와 그 출처: `given`(장면이 줌), `series`(카탈로그 표), `ratio`(가정한 비율), `default`(카탈로그 기본값)
#[derive(Clone, Debug, Default)]
pub struct Dims(pub BTreeMap<String, (f64, &'static str)>);

impl Dims {
    pub fn v(&self, k: &str) -> f64 {
        self.0.get(k).map_or(0.0, |x| x.0)
    }
    fn put(&mut self, k: &str, v: f64, src: &'static str) {
        self.0.insert(k.to_string(), (v, src));
    }
    pub fn to_json(&self) -> Value {
        Value::Object(self.0.iter().map(|(k, (v, s))| (k.clone(), json!({"v": n(*v), "src": s}))).collect())
    }
}

fn series_row(series: &str, bore: f64) -> Option<&'static Value> {
    let key = if bore.fract() == 0.0 { format!("{}", bore as i64) } else { return None };
    catalog()["series"][series]["bores"].get(&key)
}

/// 종류의 크기를 구한다(형태마다 규칙이 다르다). `params`는 기본값을 채운 값, `given`은 장면이 적은 매개변수.
/// 오류는 (필드, 글)이다: 장면이 준 값이 규칙을 어길 때(`body_len_mm`가 행정보다 짧다).
pub fn dims_of(kind: &str, params: &Map<String, Value>, given: Option<&Map<String, Value>>) -> Result<Dims, (String, String)> {
    let gv = |k: &str| given.is_some_and(|m| m.get(k).is_some_and(|v| !v.is_null()));
    let pv = |k: &str| params.get(k).and_then(Value::as_f64);
    let src = |k: &str| if gv(k) { "given" } else { "default" };
    let mut d = Dims::default();
    match form_of(kind) {
        "cyl" => {
            let (bore, stroke) = (pv("bore_mm").unwrap_or(0.0), pv("stroke_mm").unwrap_or(0.0));
            d.put("bore_mm", bore, src("bore_mm"));
            d.put("stroke_mm", stroke, src("stroke_mm"));
            let series = params.get("series").and_then(Value::as_str).unwrap_or("generic");
            let row = series_row(series, bore);
            let r = |k: &str| row.and_then(|r| r[k].as_f64());
            let spring = if kind == "cylinder_single" { (stroke / 50.0).ceil() * catalog()["series"][series]["spring_mm_per_50"].as_f64().unwrap_or(0.0) } else { 0.0 };
            let pick = |d: &mut Dims, k: &str, series_v: Option<f64>, ratio: f64| {
                if gv(k) {
                    d.put(k, pv(k).unwrap_or(ratio), "given");
                } else if let Some(s) = series_v {
                    d.put(k, s, "series");
                } else {
                    d.put(k, ratio, "ratio");
                }
            };
            if gv("body_len_mm") {
                let v = pv("body_len_mm").unwrap_or(0.0);
                if v < stroke + 20.0 {
                    return Err(("params.body_len_mm".into(), format!("must be at least stroke_mm + 20 ({})", n(stroke + 20.0))));
                }
                d.put("body_len_mm", v, "given");
            } else {
                pick(&mut d, "body_len_mm", r("S").map(|s| s + spring + stroke), (3.0 * bore + stroke).max(stroke + 20.0));
            }
            pick(&mut d, "body_dia_mm", r("I"), 1.4 * bore);
            pick(&mut d, "rod_dia_mm", r("D"), 0.4 * bore);
            pick(&mut d, "rod_out_mm", r("H"), 2.0 * bore);
            pick(&mut d, "pivot_back_mm", r("L"), 1.5 * bore);
            d.put("rod_end_mm", pv("rod_end_mm").unwrap_or(0.0), src("rod_end_mm"));
            let dia = d.v("body_dia_mm");
            if gv("axis_h_mm") {
                d.put("axis_h_mm", pv("axis_h_mm").unwrap_or(dia / 2.0), "given");
            } else {
                d.put("axis_h_mm", dia / 2.0, "default");
            }
        }
        "linear" => {
            let stroke = pv("stroke_mm").unwrap_or(0.0);
            d.put("stroke_mm", stroke, src("stroke_mm"));
            for k in ["carriage_len_mm", "carriage_h_mm", "body_h_mm", "body_w_mm"] {
                d.put(k, pv(k).unwrap_or(0.0), src(k));
            }
            if gv("lead_mm") {
                d.put("lead_mm", pv("lead_mm").unwrap_or(0.0), "given");
            }
            let min = stroke + d.v("carriage_len_mm");
            if gv("body_len_mm") {
                let v = pv("body_len_mm").unwrap_or(0.0);
                if v < min {
                    return Err(("params.body_len_mm".into(), format!("must be at least stroke_mm + carriage_len_mm ({})", n(min))));
                }
                d.put("body_len_mm", v, "given");
            } else {
                d.put("body_len_mm", min + 120.0, "ratio");
            }
        }
        "belt" => {
            for k in ["length_mm", "width_mm", "height_mm"] {
                d.put(k, pv(k).unwrap_or(0.0), src(k));
            }
        }
        "box" | "table" | "jaw" => {
            let part = &catalog()["layout"][kind]["parts"][0]["size"];
            for (i, k) in ["x_mm", "y_mm", "z_mm"].iter().enumerate() {
                if gv(k) {
                    d.put(k, pv(k).unwrap_or(1.0), "given");
                } else {
                    d.put(k, part[i].as_f64().unwrap_or(1.0), "default");
                }
            }
            // 그리퍼의 행정은 기존 매개변수이다(축의 범위가 읽는다)
            if form_of(kind) == "jaw" {
                d.put("stroke_mm", pv("stroke_mm").unwrap_or(0.0), src("stroke_mm"));
            }
        }
        _ => {}
    }
    Ok(d)
}

/// 본체 상자의 (중심, 크기), `base` 좌표. 틀과 `box_mm`의 바탕이다. 상자가 없는 형태는 None.
pub fn main_box(kind: &str, d: &Dims) -> Option<(V3, V3)> {
    match form_of(kind) {
        "box" | "table" => {
            let off = &catalog()["layout"][kind]["parts"][0]["pos"];
            let g = |i: usize| off[i].as_f64().unwrap_or(0.0);
            let s = [d.v("x_mm"), d.v("y_mm"), d.v("z_mm")];
            Some(([g(0), g(1) + s[1] / 2.0, g(2)], s))
        }
        "jaw" => {
            let s = [d.v("x_mm"), d.v("y_mm"), d.v("z_mm")];
            Some(([0.0, s[1] / 2.0, 0.0], s))
        }
        "cyl" => {
            let (l, w, h) = (d.v("body_len_mm"), d.v("body_dia_mm"), d.v("axis_h_mm"));
            Some(([0.0, h, 0.0], [l, w, w]))
        }
        "linear" => {
            let s = [d.v("body_len_mm"), d.v("body_h_mm"), d.v("body_w_mm")];
            Some(([0.0, s[1] / 2.0, 0.0], s))
        }
        "belt" => {
            let s = [d.v("length_mm"), d.v("height_mm"), d.v("width_mm")];
            Some(([0.0, s[1] / 2.0, 0.0], s))
        }
        _ => None,
    }
}

// ───────────── 축 ─────────────

/// 축이 있는 장치: 움직이는 틀의 이름, 값의 범위, 선형 축의 방향과 원점 위치
#[derive(Clone, Debug)]
pub struct AxisInfo {
    pub range: [f64; 2],
    pub frame: &'static str,
    /// 선형 서보 축(`carriage`)인가
    pub linear: bool,
    /// 주소가 자기 +x 쪽으로 커지면 +1, 아니면 -1
    g: f64,
    x_home: f64,
    home: f64,
}

pub fn axis_info(kind: &str, d: &Dims, params: &Map<String, Value>) -> Option<AxisInfo> {
    let stroke = d.v("stroke_mm");
    match form_of(kind) {
        "cyl" | "jaw" if stroke > 0.0 => Some(AxisInfo { range: [0.0, stroke], frame: "out", linear: false, g: 1.0, x_home: 0.0, home: 0.0 }),
        "linear" => {
            let home = params.get("home_mm").and_then(Value::as_f64).unwrap_or(0.0);
            let minus = params.get("home_at").and_then(Value::as_str) != Some("+x");
            let away = params.get("addr_grows").and_then(Value::as_str) != Some("toward");
            let range = if away { [home, home + stroke] } else { [home - stroke, home] };
            Some(AxisInfo { range, frame: "carriage", linear: true, g: if minus == away { 1.0 } else { -1.0 }, x_home: if minus { -stroke / 2.0 } else { stroke / 2.0 }, home })
        }
        _ => None,
    }
}

/// 움직이는 틀(`out`, `carriage`)의 원점, 장치 `base` 좌표, 축 값 `a`에서
pub fn moving_pos(kind: &str, d: &Dims, ax: &AxisInfo, a: f64) -> V3 {
    if ax.linear {
        return [ax.x_home + (a - ax.home) * ax.g, d.v("body_h_mm") + d.v("carriage_h_mm"), 0.0];
    }
    let (l, h, rod, _) = cyl_geo(kind, d);
    [l / 2.0 + rod + a, h, 0.0]
}

/// 원통과 그리퍼: (몸 길이, 축 높이, 막대가 몸 밖으로 나온 길이, 힌지까지)
fn cyl_geo(kind: &str, d: &Dims) -> (f64, f64, f64, f64) {
    if form_of(kind) == "jaw" {
        return (d.v("x_mm"), d.v("y_mm") / 2.0, GRIPPER_ROD, 0.0);
    }
    (d.v("body_len_mm"), d.v("axis_h_mm"), d.v("rod_out_mm"), d.v("pivot_back_mm"))
}

// ───────────── 틀 목록 ─────────────

/// 상자 면 틀. `c`는 상자 중심, `s`는 크기. 축은 문서의 면 표(바깥에서 보아 +x 오른쪽, +z 아래, +y 보는 사람 쪽).
fn face(name: &str, c: V3, s: V3) -> Option<Xf> {
    const X: V3 = [1.0, 0.0, 0.0];
    const Y: V3 = [0.0, 1.0, 0.0];
    const Z: V3 = [0.0, 0.0, 1.0];
    let ng = |v: V3| v.map(|c| -c);
    let (o, x, y, z) = match name {
        "top" => ([0.0, s[1] / 2.0, 0.0], X, Y, Z),
        "bottom" => ([0.0, -s[1] / 2.0, 0.0], X, ng(Y), ng(Z)),
        "front" => ([0.0, 0.0, s[2] / 2.0], X, Z, ng(Y)),
        "back" => ([0.0, 0.0, -s[2] / 2.0], ng(X), ng(Z), ng(Y)),
        "right" => ([s[0] / 2.0, 0.0, 0.0], ng(Z), X, ng(Y)),
        "left" => ([-s[0] / 2.0, 0.0, 0.0], Z, ng(X), ng(Y)),
        _ => return None,
    };
    Some(Xf { r: [x, y, z], p: [c[0] + o[0], c[1] + o[1], c[2] + o[2]] })
}

pub const FACES: &[&str] = &["top", "bottom", "front", "back", "left", "right"];

/// 움직이는 틀인가: 원통과 그리퍼의 `out`, 선형 축의 `carriage`
pub fn is_moving(form: &str, frame: &str) -> bool {
    (matches!(form, "cyl" | "jaw") && frame == "out") || (form == "linear" && frame == "carriage")
}

/// 장치의 틀 목록(이름, 장치 `base` 안의 변환, axial 여부). 맨 앞이 `base`. 움직이는 틀은 축 값 0(선형 축은 `home_mm`)에 있다.
/// 형태마다: 상자(`box`, `table`)는 여섯 면과 `seat`; `cyl`과 `jaw`는 `mid`, `cap_end`, `head_end`, (`cyl`만) `pivot`, `out`;
/// `linear`는 본체 여섯 면과 `carriage`; `belt`는 `surface`, `in`, `end`, `seat`; `solid`는 첫 모양 항목(상자)의 여섯 면.
/// 사용자 틀이 마지막이다. `frame`, `robot6`는 뒤 작업이 더한다.
pub fn frame_list(kind: &str, d: &Dims, params: &Map<String, Value>, shape: &[Item], custom: &[Cf]) -> Vec<(String, Xf, bool)> {
    let mut out = vec![("base".to_string(), IDENT, false)];
    let form = form_of(kind);
    let at = |p: V3| Xf { r: IDENT.r, p };
    let seat = |c: V3, s: V3| Xf { r: [[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]], p: [c[0], c[1] - s[1] / 2.0, c[2]] };
    match form {
        "box" | "table" | "linear" | "belt" => {
            if let Some((c, s)) = main_box(kind, d) {
                if form == "belt" {
                    let h = c[1] + s[1] / 2.0;
                    out.extend([("surface", [c[0], h, c[2]]), ("in", [c[0] - s[0] / 2.0, h, c[2]]), ("end", [c[0] + s[0] / 2.0, h, c[2]])].map(|(nm, p)| (nm.to_string(), at(p), false)));
                } else {
                    out.extend(FACES.iter().filter_map(|f| Some((f.to_string(), face(f, c, s)?, false))));
                }
                if form == "linear" {
                    if let Some(ax) = axis_info(kind, d, params) {
                        out.push(("carriage".into(), at(moving_pos(kind, d, &ax, ax.home)), false));
                    }
                } else {
                    out.push(("seat".into(), seat(c, s), true));
                }
            }
        }
        "cyl" | "jaw" => {
            let (l, h, rod, pivot) = cyl_geo(kind, d);
            out.extend([("mid", 0.0), ("cap_end", -l / 2.0), ("head_end", l / 2.0)].map(|(nm, x)| (nm.to_string(), at([x, h, 0.0]), true)));
            if form == "cyl" {
                out.push(("pivot".into(), at([-l / 2.0 - pivot, h, 0.0]), true));
            }
            out.push(("out".into(), at([l / 2.0 + rod, h, 0.0]), true));
        }
        "solid" => {
            if let Some(it) = shape.first().filter(|i| !i.cyl) {
                let ix = it.xf();
                out.extend(FACES.iter().filter_map(|f| Some((f.to_string(), ix.mul(&face(f, [0.0; 3], it.size)?), false))));
            }
        }
        _ => {}
    }
    out.extend(custom.iter().map(|c| (c.name.clone(), c.xf, c.axial)));
    out
}

/// `twin_catalog`용: 형태별 표준 틀 이름(코드에서 만든다)
pub fn frames_table() -> Value {
    let mut m = Map::new();
    for f in FORMS {
        let names: Vec<&str> = match *f {
            "box" | "table" => ["base"].iter().chain(FACES).chain(&["seat"]).copied().collect(),
            "solid" => ["base"].iter().chain(FACES).copied().collect(),
            "linear" => ["base"].iter().chain(FACES).chain(&["carriage"]).copied().collect(),
            "belt" => vec!["base", "surface", "in", "end", "seat"],
            "cyl" => vec!["base", "mid", "cap_end", "head_end", "pivot", "out"],
            "jaw" => vec!["base", "mid", "cap_end", "head_end", "out"],
            _ => vec!["base"],
        };
        m.insert(f.to_string(), json!(names));
    }
    m.insert("moving".into(), json!("`out` (cyl, jaw) and `carriage` (linear) move with the axis value; every other frame is fixed in `base`"));
    m.insert("custom".into(), json!("a `solid` may add `frames` {name:{pos, rot, type}}; they are fixed in `base`"));
    Value::Object(m)
}

fn is_port(kind: &str, name: &str) -> bool {
    catalog()["layout"][kind]["ports"].as_array().is_some_and(|a| a.iter().any(|p| p["name"] == name))
}

/// 고정 방법의 기본 datum(카탈로그 `mounts`): 글 하나이거나 형태별 객체
pub fn default_datum(mount: &str, form: &str) -> String {
    let style = catalog()["mounts"].as_array().and_then(|a| a.iter().find(|s| s["style"] == mount));
    let d = style.map(|s| &s["datum"]);
    d.and_then(|d| d.as_str().or_else(|| d[form].as_str())).unwrap_or("base").to_string()
}

// ───────────── 나무 ─────────────

/// 장치 하나의 기하. 파싱이 채우고 `resolve`가 `world`와 `parent`를 채운다.
#[derive(Clone, Debug)]
pub struct DevGeom {
    pub form: &'static str,
    pub on: Option<String>,
    pub mount: String,
    /// 이 장치의 어느 틀을 `pose`가 놓는가(기본은 고정 방법의 datum)
    pub datum: String,
    pub datum_xf: Xf,
    pub shape: Vec<Item>,
    pub frames: Vec<(String, Xf, bool)>,
    pub basis: BTreeMap<String, String>,
    /// 장면이 적은 매개변수 이름(`ask`가 쓴다)
    pub given: BTreeSet<String>,
    pub dims: Dims,
    pub axis: Option<AxisInfo>,
    /// 부모 틀에서 datum까지(T(pos) R(rot))
    pub local: Xf,
    /// 부모 장치 번호와 그 장치 틀 번호
    pub parent: Option<(usize, usize)>,
    /// `base`의 세계 변환
    pub world: Xf,
    /// `mech`의 관절이 만든 틀 이름(시작 값에서의 자리로 놓인다; 움직이는 틀이다)
    pub jframes: Vec<String>,
}

/// `mech`의 관절 틀을 틀 목록 뒤에 붙인다. 틀은 관절 값이 시작 값일 때의 자리이다(설계 2.1).
/// 형태 관절(`carriage`)은 이미 틀이 있어 붙이지 않는다.
pub fn add_joint_frames(g: &mut DevGeom, m: &super::mech::Mech) {
    let mut xf: Vec<Xf> = Vec::new();
    for j in &m.joints {
        let parent = match j.on {
            Some(k) => xf[k],
            None => g.frames.iter().find(|(nm, _, _)| *nm == j.on_frame).map_or(IDENT, |f| f.1),
        };
        let (r, p) = super::mech::start_motion(j);
        let me = parent.mul(&place(j.pos, j.rot)).mul(&Xf { r, p });
        if j.ty == super::mech::JType::Form {
            xf.push(g.frames.iter().find(|(nm, _, _)| nm == "carriage").map_or(me, |f| f.1));
            continue;
        }
        xf.push(me);
        g.frames.push((j.name.clone(), me, false));
        g.jframes.push(j.name.clone());
    }
}

impl DevGeom {
    /// 벨트의 틀 크기의 반(x, z). 설비 모델이 벨트 끝을 정한다.
    pub fn belt_half(&self) -> Option<[f64; 2]> {
        (self.form == "belt").then(|| [self.dims.v("length_mm") / 2.0, self.dims.v("width_mm") / 2.0])
    }
    /// 원통과 그리퍼: 행정 0에서 막대 끝(`out`)이 `base` 원점에서 +x로 얼마나 떨어져 있는가
    pub fn rod_tip0(&self, kind: &str) -> f64 {
        match self.form {
            "cyl" | "jaw" => self.axis.as_ref().map_or(0.0, |a| moving_pos(kind, &self.dims, a, 0.0)[0]),
            _ => 0.0,
        }
    }
}

fn dev_err(id: &str, field: &str, msg: &str) -> String {
    format!("device {id}: {field}: {msg}")
}

fn short(s: &str) -> String {
    s.chars().take(40).collect()
}

/// `"<id>"` 또는 `"<id>.<frame>"`이 가리키는 (장치 번호, 틀 번호). 오류는 접두어 없는 글이다.
pub fn locate(ids: &[&str], kinds: &[&str], frames: &[&[(String, Xf, bool)]], on: &str) -> Result<(usize, usize), String> {
    if on.len() > 80 {
        return Err("must be \"<id>\" or \"<id>.<frame>\"".into());
    }
    let (did, frame) = on.split_once('.').map_or((on, "base"), |(a, b)| (a, b));
    let p = ids.iter().position(|x| *x == did).ok_or_else(|| format!("no device \"{}\"", short(did)))?;
    match frames[p].iter().position(|(nm, _, _)| nm == frame) {
        Some(f) => Ok((p, f)),
        None if is_port(kinds[p], frame) => {
            let hint = if frame == "rod" { " (the rod end frame is \"out\")" } else { "" };
            Err(format!("\"{}\" is a port of {did}, not a frame{hint}", short(frame)))
        }
        None => {
            let names: Vec<&str> = frames[p].iter().map(|(nm, _, _)| nm.as_str()).collect();
            Err(format!("{did} has no frame \"{}\" (frames: {})", short(frame), names.join(", ")))
        }
    }
}

/// 모든 장치의 `on`을 풀고 `base`의 세계 변환을 구한다. 재귀 없이 부모 사슬을 거슬러 오른다(깊이 16, 고리 검사).
pub fn resolve(ids: &[&str], kinds: &[&str], g: &mut [DevGeom]) -> Result<(), String> {
    let count = g.len();
    // 1) 부모 연결(장치와 틀 존재)
    for i in 0..count {
        let Some(on) = g[i].on.clone() else { continue };
        let fr: Vec<&[(String, Xf, bool)]> = g.iter().map(|x| x.frames.as_slice()).collect();
        let at = locate(ids, kinds, &fr, &on).map_err(|m| dev_err(ids[i], "on", &m))?;
        g[i].parent = Some(at);
    }
    // 2) 세계 변환: 부모가 풀린 곳까지 사슬을 모은 뒤 위에서 아래로 계산한다.
    let mut done = vec![false; count];
    let mut depth = vec![0usize; count];
    for i in 0..count {
        if done[i] {
            continue;
        }
        let mut chain = vec![i];
        let base_depth;
        loop {
            let cur = chain[chain.len() - 1];
            match g[cur].parent {
                None => {
                    base_depth = 0;
                    break;
                }
                Some((p, _)) if done[p] => {
                    base_depth = depth[p];
                    break;
                }
                Some((p, _)) => {
                    if chain.contains(&p) {
                        let path: Vec<&str> = chain.iter().map(|c| ids[*c]).chain([ids[p]]).collect();
                        return Err(dev_err(ids[i], "on", &format!("the chain of on makes a loop ({})", path.join(" -> "))));
                    }
                    chain.push(p);
                    if chain.len() > MAX_DEPTH {
                        return Err(dev_err(ids[i], "on", &format!("more than {MAX_DEPTH} levels")));
                    }
                }
            }
        }
        if chain.len() + base_depth > MAX_DEPTH {
            return Err(dev_err(ids[i], "on", &format!("more than {MAX_DEPTH} levels")));
        }
        for (k, &c) in chain.iter().enumerate().rev() {
            // 세계(datum) = 부모 틀 * pose; `base`는 datum의 역변환을 한 번 더 거친다
            let place_of_datum = match g[c].parent {
                None => g[c].local,
                Some((p, f)) => g[p].world.mul(&g[p].frames[f].1).mul(&g[c].local),
            };
            g[c].world = place_of_datum.mul(&g[c].datum_xf.inv());
            depth[c] = base_depth + chain.len() - k;
            done[c] = true;
        }
    }
    Ok(())
}

/// 부모가 먼저 오는 장치 순서(나무에서 위에서 아래로). 재귀 없이 깊이를 세어 정렬한다(깊이는 `MAX_DEPTH`로 막혀 있다).
pub fn base_order(g: &[DevGeom]) -> Vec<usize> {
    let depth = |mut i: usize| {
        let mut n = 0;
        while let Some((p, _)) = g[i].parent {
            i = p;
            n += 1;
            if n > MAX_DEPTH {
                break;
            }
        }
        n
    };
    let mut o: Vec<usize> = (0..g.len()).collect();
    o.sort_by_key(|&i| depth(i));
    o
}

/// 장치 `g`의 틀 `f`의 변환(`base` 안). 움직이는 틀은 축 값 `a`에서의 자리를 쓴다.
pub fn frame_at(kind: &str, g: &DevGeom, f: usize, a: f64) -> Xf {
    let (nm, xf, _) = &g.frames[f];
    match &g.axis {
        Some(ax) if is_moving(g.form, nm) => Xf { r: xf.r, p: moving_pos(kind, &g.dims, ax, a) },
        _ => *xf,
    }
}

/// 틀 `f`의 변환(`base` 안). 관절 틀이면 `jx`(관절 값으로 구한 지금 자리, `DevGeom::jframes` 순서)에서, 아니면 `frame_at`에서 읽는다.
pub fn frame_now(kind: &str, g: &DevGeom, f: usize, a: f64, jx: &[Xf]) -> Xf {
    if !jx.is_empty() {
        if let Some(i) = g.jframes.iter().position(|n| *n == g.frames[f].0) {
            if let Some(x) = jx.get(i) {
                return *x;
            }
        }
    }
    frame_at(kind, g, f, a)
}

/// `mech`의 관절 틀을 관절 값 `vals`(`Machine::vals`)에서 다시 구한다. 순서는 `g.jframes`(형태 관절은 틀이 이미 있어 뺀다).
/// 값이 시작 값이면 `add_joint_frames`가 붙인 틀과 같다(형태 관절이 시작 값에 있는 한). 부모가 움직이는 틀(`carriage`)이면 지금 축 값에서의 자리를 쓴다.
pub fn joint_frames_now(kind: &str, g: &DevGeom, m: &super::mech::Mech, vals: &[V3]) -> Vec<Xf> {
    use super::mech::JType;
    let home = g.axis.as_ref().map_or(0.0, |ax| ax.home);
    let a = m.joints.iter().position(|j| j.ty == JType::Form).map_or(home, |k| vals[k][0]);
    let mut xf: Vec<Xf> = Vec::with_capacity(m.joints.len());
    let mut out = Vec::with_capacity(g.jframes.len());
    for (k, j) in m.joints.iter().enumerate() {
        let parent = match j.on {
            Some(p) => xf[p],
            None => g.frames.iter().position(|(nm, _, _)| *nm == j.on_frame).map_or(IDENT, |i| frame_at(kind, g, i, a)),
        };
        if j.ty == JType::Form {
            xf.push(g.frames.iter().position(|(nm, _, _)| nm == "carriage").map_or(parent, |i| frame_at(kind, g, i, a)));
            continue;
        }
        let (r, p) = super::mech::motion(j, vals[k]);
        let me = parent.mul(&place(j.pos, j.rot)).mul(&Xf { r, p });
        xf.push(me);
        out.push(me);
    }
    out
}

/// 축 값 `vals`(장치마다 하나; 축이 없는 장치는 쓰지 않는다)와 관절 틀 `jx`(장치마다 `joint_frames_now`의 결과, 없으면 빈 목록)에서
/// 모든 장치 `base`의 세계 변환을 `out`에 쓴다. `order`는 `base_order`의 결과이다. 할당하지 않는다.
pub fn worlds_at(kinds: &[String], g: &[DevGeom], order: &[usize], vals: &[f64], jx: &[Vec<Xf>], out: &mut [Xf]) {
    for &c in order {
        let datum_world = match g[c].parent {
            None => g[c].local,
            Some((p, f)) => out[p].mul(&frame_now(&kinds[p], &g[p], f, vals[p], &jx[p])).mul(&g[c].local),
        };
        out[c] = datum_world.mul(&g[c].datum_xf.inv());
    }
}

/// 세계 좌표의 +x가 y축 회전(heading)으로 몇 도인지(수직이면 0)와 기울기(수평 위로 몇 도)
fn heading_tilt(axis: V3) -> (f64, f64) {
    let a = clean(axis);
    let tilt = a[1].clamp(-1.0, 1.0).asin().to_degrees();
    let heading = if tilt.abs() > 89.5 { 0.0 } else { (-a[2]).atan2(a[0]).to_degrees() };
    (heading, tilt)
}

/// 방향의 말: `up`, `down`, `level toward +x`, `30.0 deg below level, toward +z`, `level, heading 56.3 deg`
pub fn axis_word(axis: V3) -> String {
    let (h, t) = heading_tilt(axis);
    if t > 89.5 {
        return "up".into();
    }
    if t < -89.5 {
        return "down".into();
    }
    let tilt = if t.abs() < 0.05 {
        "level".to_string()
    } else if t < 0.0 {
        format!("{:.1} deg below level", -t)
    } else {
        format!("{t:.1} deg above level")
    };
    let named = [(0.0, "toward +x"), (180.0, "toward -x"), (-180.0, "toward -x"), (-90.0, "toward +z"), (90.0, "toward -z")].iter().find(|(a, _)| (h - a).abs() < 0.05).map(|(_, w)| *w);
    match (named, tilt.as_str()) {
        (Some(w), "level") => format!("level {w}"),
        (Some(w), _) => format!("{tilt}, {w}"),
        (None, _) => format!("{tilt}, heading {h:.1} deg"),
    }
}

/// 설비 모델에 주는 `Dev.pos`와 `rot_y`(다리 값): 세계의 `base` 원점과 +x의 heading
pub fn bridge(world: &Xf) -> (V3, f64) {
    (world.p, heading_tilt(world.r[0]).0)
}

// ───────────── 장면의 known length(refs) ─────────────

/// 장면 키 `refs`의 항목 하나와 이 장면에서의 거리
#[derive(Clone, Debug)]
pub struct RefItem {
    pub a: String,
    pub b: String,
    pub mm: f64,
    pub basis: String,
    pub scene_mm: f64,
}

/// `refs`를 읽는다(개수를 먼저 본다). 틀 이름은 `on`과 같은 규칙으로 풀고, 거리는 틀 원점 사이(축 값 0)이다.
pub fn parse_refs(v: &Value, ids: &[&str], kinds: &[&str], g: &[DevGeom]) -> Result<Vec<RefItem>, String> {
    let a = v.as_array().filter(|a| a.len() <= MAX_REFS).ok_or(format!("scene.refs: at most {MAX_REFS} items"))?;
    let fr: Vec<&[(String, Xf, bool)]> = g.iter().map(|x| x.frames.as_slice()).collect();
    let mut out = Vec::new();
    for (i, it) in a.iter().enumerate() {
        let o = it.as_object().ok_or(format!("scene.refs[{i}]: must be an object"))?;
        if let Some(k) = o.keys().find(|k| !matches!(k.as_str(), "a" | "b" | "mm" | "basis")) {
            return Err(format!("scene.refs[{i}].{}: unknown field", k.chars().take(24).collect::<String>()));
        }
        let mm = o.get("mm").and_then(|m| num_in(m, 1.0, 100_000.0)).ok_or(format!("scene.refs[{i}].mm: must be 1 to 100000"))?;
        let basis = o.get("basis").and_then(Value::as_str).filter(|b| matches!(*b, "measured" | "label")).ok_or(format!("scene.refs[{i}].basis: must be measured or label"))?;
        let mut origin = [[0.0; 3]; 2];
        let mut names = Vec::new();
        for (k, key) in ["a", "b"].iter().enumerate() {
            let t = o.get(*key).and_then(Value::as_str).ok_or(format!("scene.refs[{i}].{key}: must be \"<id>.<frame>\""))?;
            let (p, f) = locate(ids, kinds, &fr, t).map_err(|m| format!("scene.refs[{i}].{key}: {m}"))?;
            origin[k] = g[p].world.mul(&g[p].frames[f].1).p;
            names.push(t.to_string());
        }
        out.push(RefItem { a: names[0].clone(), b: names[1].clone(), mm, basis: basis.into(), scene_mm: dist(origin[0], origin[1]) });
    }
    Ok(out)
}

// ───────────── 답에 붙이는 것 ─────────────

/// `facts`: 종류별 개수, 장치마다 놓인 곳과 축(말과 숫자), 크기, `basis` 개수, `refs`. 형식 2 장면만.
/// id와 숫자만 싣는다(이름표와 model은 싣지 않는다).
pub fn facts(s: &Scene) -> Value {
    let mut counts: BTreeMap<&str, u64> = BTreeMap::new();
    let mut basis_counts: BTreeMap<&str, u64> = BTreeMap::new();
    let mut lines = Vec::new();
    for d in &s.devices {
        *counts.entry(d.kind.as_str()).or_default() += 1;
        let Some(g) = &d.geom else { continue };
        for v in g.basis.values() {
            *basis_counts.entry(v.as_str()).or_default() += 1;
        }
        if lines.len() >= MAX_FACT_LINES {
            continue;
        }
        let (heading, tilt) = heading_tilt(g.world.r[0]);
        let mut l = json!({"id": d.id, "kind": d.kind});
        if let Some(on) = &g.on {
            l["on"] = json!(on);
        }
        l["mount"] = json!(g.mount);
        l["datum"] = json!(g.datum);
        l["at_world"] = nv(g.world.pt(g.datum_xf.p));
        l["axis_world"] = nv(g.world.r[0]);
        l["up_world"] = nv(g.world.r[1]);
        l["axis_word"] = json!(axis_word(g.world.r[0]));
        l["heading_deg"] = n(heading);
        l["tilt_deg"] = n(tilt);
        if let Some((_, size)) = main_box(&d.kind, &g.dims) {
            l["box_mm"] = nv(size);
        }
        if !g.dims.0.is_empty() {
            l["dims"] = g.dims.to_json();
        }
        if let Some(ax) = &g.axis {
            let tip = |a: f64| nv(g.world.pt(moving_pos(&d.kind, &g.dims, ax, a)));
            let mut a = json!({"type": "slide", "range": [n(ax.range[0]), n(ax.range[1])], "tip_at_0": tip(ax.range[0]), "tip_at_max": tip(ax.range[1])});
            if ax.linear {
                a["frame"] = json!(ax.frame);
                a["addr_world"] = nv(g.world.dir([ax.g, 0.0, 0.0]));
            }
            l["axis"] = a;
        }
        lines.push(l);
    }
    let mut f = json!({"counts": counts, "devices": lines, "basis_counts": basis_counts});
    if s.devices.len() > MAX_FACT_LINES {
        f["devices_not_listed"] = json!(s.devices.len() - MAX_FACT_LINES);
    }
    if let Some(w) = &s.work {
        let wf = super::work::facts(&s.devices, w);
        (f["points"], f["workpieces"], f["push"]) = (wf.points, wf.workpieces, wf.push);
    }
    f["refs"] = json!(s.refs.iter().map(|r| json!({"a": r.a, "b": r.b, "mm": n(r.mm), "basis": r.basis, "scene_mm": n(r.scene_mm)})).collect::<Vec<_>>());
    let mf = mech_facts(s);
    if !mf.is_empty() {
        f["mech"] = Value::Array(mf);
    }
    f
}

/// `facts.mech`: 관절 틀의 자리(시작 값), 결합, 구동기, 신호와 주소. 숫자와 이름만 싣는다(설계 3.4).
fn mech_facts(s: &Scene) -> Vec<Value> {
    use super::mech::{Coupling, Driver, JType};
    let mut out = Vec::new();
    for d in s.devices.iter().filter(|d| d.mech.is_some()) {
        let (Some(m), Some(g)) = (&d.mech, &d.geom) else { continue };
        let joints: Vec<Value> = m
            .joints
            .iter()
            .map(|j| {
                let ty = j.ty.name();
                let fr = g.frames.iter().find(|(nm, _, _)| *nm == j.name).map(|f| f.1);
                let start = if j.ty == JType::Planar { json!([n(j.start[0]), n(j.start[1]), n(j.start[2])]) } else { n(j.start[0]) };
                json!({
                    "name": j.name, "type": ty, "range": j.lim.map(|l| json!([n(l[0]), n(l[1])])), "start": start,
                    "at_world": fr.map(|x| nv(g.world.pt(x.p))), "axis_world": fr.filter(|_| j.axis != [0.0; 3]).map(|x| nv(g.world.dir(x.dir(j.axis))))
                })
            })
            .collect();
        let couplings: Vec<Value> = m
            .couplings
            .iter()
            .map(|c| match c {
                Coupling::Ratio { from, to, k } => json!({"type": "ratio", "from": m.joints[*from].name, "to": m.joints[*to].name, "k": n(*k)}),
                Coupling::Table { from, to, .. } => json!({"type": "table", "from": m.joints[*from].name, "to": m.joints[*to].name}),
                Coupling::Diff { left, right, base, wheel_d, .. } => json!({"type": "diff_drive", "left": m.joints[*left].name, "right": m.joints[*right].name, "base": m.joints[*base].name, "mm_per_deg": n(std::f64::consts::PI * wheel_d / 360.0)}),
            })
            .collect();
        let drivers: Vec<Value> = m
            .drivers
            .iter()
            .map(|dr| match dr {
                Driver::Coils { joint, .. } => json!({"type": "coils", "joint": m.joints[*joint].name}),
                Driver::Follow { joint, .. } => json!({"type": "follow", "joint": m.joints[*joint].name}),
                Driver::Step { joint, .. } => json!({"type": "step", "joint": m.joints[*joint].name}),
                Driver::Playback { joints, programs, .. } => json!({"type": "playback", "joints": joints.iter().map(|j| m.joints[*j].name.clone()).collect::<Vec<_>>(), "programs": programs.iter().map(|q| q.no).collect::<Vec<_>>()}),
            })
            .collect();
        let signals: Map<String, Value> = d.slots.iter().chain(d.signals.iter()).map(|sl| (sl.name.clone(), json!(if sl.bound { Value::String(sl.addr.clone()) } else { Value::Null }))).collect();
        let mut item = json!({"device": d.id, "preset": m.preset, "joints": joints, "couplings": couplings, "drivers": drivers, "signals": signals});
        if let (Some(ex), Some(o)) = (&m.expanded, item.as_object_mut()) {
            o.insert("expanded".into(), ex.clone());
        }
        out.push(item);
    }
    out
}

/// 장치 `i`가 움직이는 틀 위에 있는가(조상 사슬을 올라가며 본다; 깊이는 `MAX_DEPTH`로 막혀 있다)
fn on_moving_frame(s: &Scene, i: usize) -> bool {
    let mut cur = i;
    for _ in 0..=MAX_DEPTH {
        let Some((p, f)) = s.devices[cur].geom.as_ref().and_then(|g| g.parent) else { return false };
        let pg = s.devices[p].geom.as_ref();
        // 관절 틀은 공작물 규칙이 따라간다(작업 2.2): 움직인다고 적지 않는다
        if pg.is_some_and(|pg| is_moving(pg.form, &pg.frames[f].0)) {
            return true;
        }
        cur = p;
    }
    false
}

/// `ask`, `notes`, `not_simulated`, `limits`(형식 2 장면만; 설계 2.8). 마일스톤 A는 형식 2 겹침 경고가 없다고 `notes`가 말한다.
pub fn extras(s: &Scene) -> Value {
    let simulated = |d: &super::scene::Dev| !d.has("source") && !d.has("sink") && !d.has("zone");
    let mut ask: Vec<Value> = s.devices.iter().filter(|d| d.geom.as_ref().is_some_and(|g| g.mount == "unknown") && simulated(d)).map(|d| json!({"device": d.id, "what": "mount"})).collect();
    let ask_params: Vec<&str> = catalog()["ask_params"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    for d in &s.devices {
        let Some(g) = &d.geom else { continue };
        let names: Vec<&str> = ask_params.iter().copied().filter(|p| g.given.contains(*p) && g.basis.get(*p).is_none_or(|b| b == "guess")).collect();
        if !names.is_empty() {
            ask.push(json!({"device": d.id, "what": "dims", "names": names}));
        }
    }
    // mech: 값을 준 필드 중 근거가 없거나 guess인 것(장치의 basis도 본다: lead_mm 같은 매개변수)
    for d in &s.devices {
        let (Some(m), Some(g)) = (&d.mech, &d.geom) else { continue };
        let names: Vec<&str> = super::mech::ASK_KEYS.iter().copied().filter(|k| m.given.contains(*k) && m.basis.get(*k).or_else(|| g.basis.get(*k)).is_none_or(|b| b == "guess")).collect();
        if !names.is_empty() {
            ask.push(json!({"device": d.id, "what": "mech", "names": names}));
        }
    }
    if !s.refs.iter().any(|r| r.basis == "measured") {
        ask.push(json!({"what": "one reference dimension measured on the real rig"}));
    }
    let has_parts = s.devices.iter().any(|d| d.has("source"));
    let mut not_sim = Vec::new();
    for (i, d) in s.devices.iter().enumerate() {
        let Some(g) = &d.geom else { continue };
        // 부품 형식의 `sense`가 있는 센서와 `position`이 있는 서보 축은 모델이 돌리므로 빼는다 (B8 BR1)
        let has_sense = s.work.as_ref().is_some_and(|w| w.senses.iter().any(|x| x.dev == i));
        let has_pos = s.work.as_ref().is_some_and(|w| w.positions.iter().any(|x| x.dev == i));
        let why = if !d.simulated && !has_pos && d.mech.is_none() {
            Some("drawn, not simulated")
        } else if d.has("sensor") && !has_sense {
            Some("no sense key")
        } else if has_parts && g.world.r[1][1] < 0.5f64.to_radians().cos() {
            Some("not upright")
        } else if has_parts && on_moving_frame(s, i) {
            Some("on a moving frame")
        } else {
            None
        };
        if let Some(w) = why {
            not_sim.push(json!({"device": d.id, "why": w}));
        }
    }
    let mut limits: Vec<&str> = Vec::new();
    if has_parts {
        let any = |k: &str| s.devices.iter().any(|d| d.kind == k);
        limits.extend(["heights are not tested (schema 2, before phase 5)", "no part contact", "no part rotation"]);
        if any("solid") {
            limits.push("a solid moves no part");
        }
        if any("conveyor") || any("chute") {
            limits.extend(["no stop rule", "a part does not cross a gap between two surfaces"]);
        }
        if any("index_table") {
            limits.push("an index table carries no part");
        }
        // `mech`가 있는 로봇의 관절 틀은 규칙이 따라간다(작업 2.2). 이 줄은 `mech`가 없는 로봇(움직이지 않는다)에만 남는다
        if s.devices.iter().any(|d| d.kind == "robot_arm" && d.mech.is_none()) {
            limits.push("a robot carries no part");
        }
    }
    let mut notes = vec![json!("schema 2: overlap, gap, sunk and unattached are not checked in this build")];
    for d in &s.devices {
        let Some(m) = &d.mech else { continue };
        // 주소가 없는 신호(절차의 5단계가 이름을 먼저 쓴다)
        for sl in d.slots.iter().chain(&d.signals).filter(|sl| !sl.bound) {
            notes.push(json!(format!("{}: signals: {} has no device yet (bind it in step 8 of guide.modeling)", d.id, sl.name)));
        }
        // 차동 구동: 두 바퀴 관절 원점 사이 거리가 track_mm와 1 mm보다 더 다르면 알린다
        for (i, c) in m.couplings.iter().enumerate() {
            if let super::mech::Coupling::Diff { left, right, track, .. } = c {
                let (l, r) = (&m.joints[*left], &m.joints[*right]);
                if l.on_frame == r.on_frame {
                    let dist = ((l.pos[0] - r.pos[0]).powi(2) + (l.pos[1] - r.pos[1]).powi(2) + (l.pos[2] - r.pos[2]).powi(2)).sqrt();
                    if (dist - track).abs() > 1.0 {
                        notes.push(json!(format!("{}: mech.couplings[{i}]: the wheel joints are {} mm apart, track_mm is {}", d.id, dist, track)));
                    }
                }
            }
        }
    }
    if let Some(w) = &s.work {
        let wf = super::work::facts(&s.devices, w);
        ask.extend(wf.ask.into_iter().map(Value::from));
        notes.extend(wf.notes.into_iter().map(Value::from));
    }
    json!({"ask": ask, "notes": notes, "not_simulated": not_sim, "limits": limits})
}

/// `not_simulated`와 `limits`만(`twin_sim_run`의 결과용)
pub fn sim_notes(s: &Scene) -> Value {
    let e = extras(s);
    json!({"not_simulated": e["not_simulated"], "limits": e["limits"]})
}

/// 이 빌드의 상자들(`base` 좌표): `solid`는 모양 항목마다, 다른 형태는 본체 상자(움직이는 부분은 `moving`).
fn boxes(kind: &str, g: &DevGeom) -> Vec<Value> {
    if g.form == "solid" {
        return g.shape.iter().enumerate().map(|(i, it)| json!({"role": format!("shape{i}"), "shape": if it.cyl { "cyl" } else { "box" }, "size": nv(it.size), "pos": nv(it.pos), "rot": nv(it.rot), "moving": false})).collect();
    }
    let mut out = Vec::new();
    let Some((c, s)) = main_box(kind, &g.dims) else { return out };
    out.push(json!({"role": "body", "shape": "box", "size": nv(s), "pos": nv(c), "rot": [0, 0, 0], "moving": false}));
    if let Some(ax) = &g.axis {
        if ax.linear {
            let at = moving_pos(kind, &g.dims, ax, ax.home);
            let (cl, ch) = (g.dims.v("carriage_len_mm"), g.dims.v("carriage_h_mm"));
            out.push(json!({"role": "carriage", "shape": "box", "size": nv([cl, ch, s[2]]), "pos": nv([at[0], at[1] - ch / 2.0, 0.0]), "rot": [0, 0, 0], "moving": true}));
        } else {
            let (l, h, rod, _) = cyl_geo(kind, &g.dims);
            let dia = if g.form == "jaw" { g.dims.v("z_mm") } else { g.dims.v("rod_dia_mm") };
            out.push(json!({"role": "rod", "shape": "cyl", "size": nv([rod, dia, dia]), "pos": nv([l / 2.0 + rod / 2.0, h, 0.0]), "rot": [0, 0, 0], "moving": true}));
        }
    }
    out
}

/// `resolved`(화면용; 형식 2 장면만): 장치마다 부모 틀, 부모 틀에서의 변환, 세계 변환, 틀, 상자, 모양 항목, 크기.
/// 변환은 12개 수(x축, y축, z축, 원점). `world`는 `base`의 세계 변환이다.
pub fn resolved(s: &Scene) -> Value {
    let devices: Vec<Value> = s.devices.iter().filter_map(|d| {
        let g = d.geom.as_ref()?;
        let parent = g.parent.map(|(p, f)| {
            let pg = s.devices[p].geom.as_ref();
            let fr = pg.and_then(|pg| pg.frames.get(f));
            json!({"device": s.devices[p].id, "frame": fr.map(|x| x.0.clone()), "frame_local": fr.map_or(Value::Null, |x| x.1.flat())})
        });
        let mut frames: Map<String, Value> = g.frames.iter().map(|(nm, x, axial)| (nm.clone(), json!({"xf": x.flat(), "type": if *axial { "axial" } else { "surface" }, "moving": is_moving(g.form, nm)}))).collect();
        let mut out = json!({"id": d.id, "kind": d.kind, "form": g.form, "datum": g.datum, "parent": parent, "local": g.local.flat(), "world": g.world.flat(), "boxes": boxes(&d.kind, g), "shape": g.shape.iter().map(Item::to_json).collect::<Vec<_>>(), "dims": g.dims.to_json()});
        // 엔진 관절: 쪽이 관절 값으로 틀을 옮기는 데 쓴다(2.1b). `mech`가 없는 장치는 아무 키도 더하지 않는다.
        if let Some(m) = &d.mech {
            for j in &m.joints {
                use super::mech::JType;
                if j.ty == JType::Form {
                    continue;
                }
                let ty = j.ty.name();
                // 앞 관절이 형태 관절이면 그 틀(carriage)은 쉬는 자리에 있으므로 부모 없음으로 적는다. 고정 관절은 부모를 따라간다
                let parent = j.on.map(|k| &m.joints[k]).filter(|p| p.ty != JType::Form).map(|p| p.name.clone());
                if let Some(f) = frames.get_mut(&j.name).and_then(Value::as_object_mut) {
                    f.insert("joint".into(), json!({"type": ty, "axis": j.axis, "zero": j.zero, "start": j.start, "wrap": j.wrap, "min": j.lim.map(|l| l[0]), "max": j.lim.map(|l| l[1]), "parent": parent, "on_form": j.on.is_some_and(|k| m.joints[k].ty == JType::Form)}));
                }
            }
            let form = g.axis.as_ref().filter(|a| a.linear && m.joints.iter().any(|j| j.ty == super::mech::JType::Form)).map(|a| json!({"home": a.home, "g": a.g}));
            out["mech"] = json!({"form": form});
        } else if let Some(a) = g.axis.as_ref().filter(|a| a.linear && s.work.as_ref().is_some_and(|w| w.positions.iter().any(|p| s.devices[p.dev].id == d.id))) {
            // 짧은 형태 `position`도 엔진의 형태 관절이다(2.2): 쪽이 carriage를 축 값으로 옮긴다(RR1)
            out["mech"] = json!({"form": {"home": a.home, "g": a.g}});
        }
        out["frames"] = Value::Object(frames);
        Some(out)
    }).collect();
    json!({"devices": devices})
}
