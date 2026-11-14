//! 장면(scene) 형식, 검사, 종류 목록(catalog). 장면 파일은 믿지 않는 글이다: 크기, 개수, 숫자 범위, 주소를 모두 검사한다.
//! 오류 글은 필드와 장치 id를 말하고 파일 경로는 싣지 않는다. 계약은 `docs/twin-api.md`.

use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::OnceLock;

use super::bind::Addresses;
use super::geom;

pub const MAX_DEVICES: usize = 200;
pub const MAX_PHOTOS: usize = 32;
pub const MAX_SCENE_BYTES: usize = 256 * 1024;
/// 저장하는 장면·초안 파일의 가장 큰 크기(읽을 때도 같은 값). 요청은 압축한 JSON 256 KiB까지 받고(`MAX_JSON`), 파일은 읽기 좋게 줄을 바꿔 쓰므로
/// 약 4배가 된다: 항목 한도를 다 채운 장면(장치 200개, 외형 985개, 모양 934개; 압축 256,608바이트)이 1,050,615바이트였다(FR2).
/// 그 두 배인 2 MiB로 둔다. 이보다 큰 글은 쓰기 전에 거절하므로 저장소가 받은 장면은 늘 다시 읽힌다.
pub const MAX_STORED: usize = 2 * 1024 * 1024;
pub const MAX_LABEL: usize = 40;
const MAX_SIGNALS: usize = 32;
const MAX_MS: f64 = 60_000.0;
const MAX_POSE: f64 = 100_000.0;
/// 형식 2에서만 쓸 수 있는 장치 키
const S2_KEYS: [&str; 16] = ["on", "mount", "datum", "basis", "model", "shape", "frames", "skin", "skin_source", "indicators", "stack", "push", "stop", "sense", "position", "mech"];
/// 외형과 표시등의 한도(경고가 시험하지 않으므로 자기 개수를 가진다)
const MAX_SKIN_GROUPS: usize = 16;
const MAX_SKIN_ITEMS: usize = 64;
const MAX_SKIN_BYTES: usize = 16 * 1024;
const MAX_SKIN_SCENE: usize = 1000;
const MAX_IND_ROWS: usize = 8;
const MAX_IND_LAMPS: usize = 128;
const MAX_LAMP_SCENE: usize = 512;

static CATALOG: OnceLock<Value> = OnceLock::new();

const CATALOG_BAD: &str = "the device catalog of this build is damaged";

/// 종류 목록. 실행 파일에 들어 있는 데이터(`catalog.json`)이다. 모양은 `check_catalog` 시험이 보증하므로
/// 읽기에 실패하면 빈 목록이 되고, 요청 경로는 `CATALOG_BAD` 오류를 돌려준다(패닉하지 않는다).
pub fn catalog() -> &'static Value {
    CATALOG.get_or_init(|| serde_json::from_str(include_str!("catalog.json")).unwrap_or_else(|_| json!({"kinds": {}, "layout": {}})))
}

/// 종류 목록의 모양을 검사한다(빌드 시험이 부른다): 모든 종류에 동작 목록, io 객체, 매개변수 규칙이 있고
/// 매개변수의 기본값이 자기 범위나 목록 안에 있으며 모든 종류에 배치(layout)가 있다.
/// (호스트의 시험도 부르므로 `cfg(test)`가 아니다.)
pub fn check_catalog(c: &Value) -> Result<(), String> {
    let kinds = c["kinds"].as_object().filter(|k| !k.is_empty()).ok_or("kinds must be a non-empty object")?;
    for (k, v) in kinds {
        let e = |m: &str| format!("kind {k}: {m}");
        v["class"].as_str().ok_or_else(|| e("class"))?;
        v["simulated"].as_bool().ok_or_else(|| e("simulated"))?;
        if !v["behaviors"].as_array().is_some_and(|a| a.iter().all(Value::is_string)) {
            return Err(e("behaviors must be an array of text"));
        }
        let io = v["io"].as_object().ok_or_else(|| e("io must be an object"))?;
        if !io.values().all(|s| matches!(s["dir"].as_str(), Some("in" | "out"))) {
            return Err(e("each io slot needs dir in or out"));
        }
        let extra = v["params2"].as_object().into_iter().flatten();
        for (pn, p) in v["params"].as_object().ok_or_else(|| e("params must be an object"))?.iter().chain(extra) {
            let pe = |m: &str| format!("kind {k} param {pn}: {m}");
            match p["type"].as_str() {
                Some("number") => {
                    // `derive`와 `optional`은 기본값이 없다(크기는 `geom.rs`가 구한다)
                    let (lo, hi, d) = (p["min"].as_f64(), p["max"].as_f64(), p["default"].as_f64());
                    let open = p["derive"] == true || p["optional"] == true;
                    if !matches!((lo, hi, d), (Some(lo), Some(hi), Some(d)) if lo <= d && d <= hi) && !(open && d.is_none() && lo.is_some() && hi.is_some()) {
                        return Err(pe("min, max and default must be numbers with min <= default <= max"));
                    }
                }
                Some("enum") => {
                    if !p["values"].as_array().is_some_and(|a| a.contains(&p["default"])) {
                        return Err(pe("default must be one of values"));
                    }
                }
                Some("bool") => {
                    if !p["default"].is_boolean() {
                        return Err(pe("default must be true or false"));
                    }
                }
                _ => return Err(pe("type must be number, enum or bool")),
            }
        }
        let form = v["form"].as_str().unwrap_or("box");
        if !geom::FORMS.contains(&form) {
            return Err(e("form is not a form of geom::FORMS"));
        }
        // `solid`은 배치(layout)가 없는 유일한 종류다(모양은 장면의 `shape`이다)
        if !matches!(form, "solid" | "mech") && c["layout"][k]["parts"].as_array().is_none_or(|a| a.is_empty()) {
            return Err(e("layout needs at least one part"));
        }
    }
    // 시리즈 표: 모든 행에 출처, 이름은 원통 종류의 `series` 열거와 같다
    let series = c["series"].as_object().filter(|s| !s.is_empty()).ok_or("series must be a non-empty object")?;
    for (sn, s) in series {
        if s["source"].as_str().is_none_or(|t| t.is_empty()) {
            return Err(format!("series {sn}: needs a source text"));
        }
    }
    let mut names: Vec<&str> = series.keys().map(String::as_str).collect();
    names.sort_unstable();
    for kn in ["cylinder_double", "cylinder_single"] {
        let mut vals: Vec<&str> = kinds[kn]["params2"]["series"]["values"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        vals.sort_unstable();
        if vals != names {
            return Err(format!("kind {kn}: params2.series.values must be the keys of series"));
        }
    }
    // 색 이름: 닫힌 목록, `mark`가 있어야 한다. 안내(guide)는 순서와 규칙과 예를 가진다.
    let tones: Vec<&str> = c["tones"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    if tones.len() != 11 || !tones.contains(&"mark") || !tones.contains(&"metal") {
        return Err("tones must be the 11 names of the design (metal ... mark)".into());
    }
    if c["guide"]["order"].as_array().is_none_or(|o| o.len() < 7) || c["guide"]["skin_rules"].as_str().is_none() || !c["guide"]["skin_recipe"]["device"].is_object() {
        return Err("guide needs order, skin_rules and skin_recipe.device".into());
    }
    let mounts = c["mounts"].as_array().filter(|m| !m.is_empty()).ok_or("mounts must be a non-empty array")?;
    for m in mounts {
        let ok = m["style"].as_str().is_some() && m["forms"].as_array().is_some_and(|f| !f.is_empty() && f.iter().all(|x| x == "all" || x.as_str().is_some_and(|x| geom::FORMS.contains(&x))));
        if !ok {
            return Err(format!("mount {}: forms must be \"all\" or names of geom::FORMS", m["style"]));
        }
    }
    Ok(())
}

/// 장면 이름: 영문, 숫자, `-`, `_` 1~32자. Windows 장치 이름은 거부한다.
pub fn valid_scene_name(n: &str) -> bool {
    let lower = n.to_ascii_lowercase();
    let reserved = matches!(lower.as_str(), "con" | "prn" | "aux" | "nul") || ["com", "lpt"].iter().any(|p| lower.strip_prefix(p).is_some_and(|d| d.len() == 1 && d.as_bytes()[0].is_ascii_digit()));
    (1..=32).contains(&n.len()) && n.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') && !reserved
}

/// 신호 한 개(칸 이름과 정규화한 주소). `out`은 PLC가 내보내는 쪽(코일)이다.
#[derive(Clone, Debug)]
pub struct Slot {
    pub name: String,
    pub addr: String,
    pub out: bool,
    pub word: bool,
    /// 주소가 있는가. 형식 2의 `signals`는 주소 없이 이름만 둘 수 있다(`addr`은 빈 글). 읽기 계획과 쓰기 검사는 이런 신호를 건너뛴다.
    pub bound: bool,
    /// BCD 워드(`type: bcd`)와 낮은 활성(`active: low`; 비트를 뒤집어 읽고 쓴다)
    pub bcd: bool,
    pub low: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Timing {
    pub lag_ms: f64,
    pub fwd_ms: f64,
    pub back_ms: f64,
}

#[derive(Clone, Debug)]
pub struct Dev {
    pub id: String,
    pub kind: String,
    pub behaviors: Vec<String>,
    pub simulated: bool,
    /// 기본값을 채운 매개변수
    pub params: Map<String, Value>,
    pub slots: Vec<Slot>,
    pub signals: Vec<Slot>,
    pub timing: Timing,
    /// 형식 2: 기계 모델 엔진 키 `mech`(관절, 결합, 구동기, 센서)
    pub mech: Option<super::mech::Mech>,
    /// 놓인 위치(mm), y축 회전(도), 부품별 위치 덮어쓰기(작업 1.6)
    pub pos: [f64; 3],
    pub rot_y: f64,
    pub parts: BTreeMap<String, [f64; 3]>,
    /// 형식 2 장면만: 나무, 변환, 틀(`geom.rs`). 형식 1에서는 None이다.
    pub geom: Option<super::geom::DevGeom>,
}

impl Dev {
    pub fn num(&self, k: &str) -> f64 {
        self.params.get(k).and_then(Value::as_f64).unwrap_or(0.0)
    }
    pub fn text(&self, k: &str) -> &str {
        self.params.get(k).and_then(Value::as_str).unwrap_or("")
    }
    pub fn slot(&self, k: &str) -> Option<&str> {
        self.slots.iter().find(|s| s.name == k).map(|s| s.addr.as_str())
    }
    pub fn has(&self, b: &str) -> bool {
        self.behaviors.iter().any(|x| x == b)
    }
}

#[derive(Clone, Debug)]
pub struct Scene {
    pub schema: i64,
    /// 형식 2: 장면이 적은 알려진 길이(`refs`)와 이 장면에서의 거리
    pub refs: Vec<geom::RefItem>,
    /// 형식 2: 외형이나 표시등이 있는 장치의 개수와 어두운 표시등(`facts.skin_counts`와 `notes`가 읽는다)
    pub skins: Vec<SkinInfo>,
    /// 형식 2: 공작물 키(`points`, `workpieces`, 장치의 `stack` 등). 키가 없으면 None이다.
    pub work: Option<super::work::Work>,
    pub name: String,
    pub devices: Vec<Dev>,
    pub links: Vec<super::layout::Link>,
    /// 이 장면을 읽은 호스트의 주소 규칙(설비 모델이 32비트 읽기와 고장 주소에 쓴다)
    pub addrs: &'static dyn Addresses,
}

fn dev_err(id: &str, field: &str, msg: &str) -> String {
    format!("device {id}: {field}: {msg}")
}

/// 글 검사. 실패하면 까닭의 글을 돌려준다: 길이가 넘었는지, 아니면 제어 문자와 서식·방향 문자(Cf)와 공백처럼 보이는 글자가 있는지.
/// 뒤의 말은 래더 초안 글("has control characters")과 같다 (AR8, FR3).
fn text_problem(v: &Value, max: usize) -> Option<String> {
    let Some(s) = v.as_str() else { return Some(format!("must be text of at most {max} characters")) };
    if s.chars().count() > max {
        return Some(format!("must be text of at most {max} characters"));
    }
    s.chars().any(crate::text::refused_char).then(|| "has control characters".to_string())
}

fn plain_id(s: &str) -> bool {
    (1..=32).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// 장치 id로 못 쓰는 이름: 일반 JavaScript 객체(`Object.prototype`)의 속성 이름. 화면이 id로 객체를 찾을 때 엉뚱한 값이 나온다 (AR2).
/// 이 목록은 한 곳에만 둔다. `prototype`은 `Object.prototype`의 속성은 아니지만 함수 객체의 것이라 함께 막는다.
pub const RESERVED_IDS: &[&str] = &[
    "__proto__", "constructor", "prototype", "toString", "valueOf", "hasOwnProperty", "isPrototypeOf", "propertyIsEnumerable", "toLocaleString",
    "__defineGetter__", "__defineSetter__", "__lookupGetter__", "__lookupSetter__",
];

fn num_in(v: &Value, min: f64, max: f64) -> Option<f64> {
    v.as_f64().filter(|n| n.is_finite() && *n >= min && *n <= max)
}

fn triple(v: &Value, max: f64) -> bool {
    v.as_array().is_some_and(|a| a.len() == 3 && a.iter().all(|x| num_in(x, -max, max).is_some()))
}

/// 이 종류의 매개변수 규칙. 형식 2는 `params2`(종류별 크기 매개변수)와, 상자 형태의 `box_params`(x_mm, y_mm, z_mm)를 더한다.
fn param_spec(kname: &str, kind: &Value, schema: i64) -> Result<Map<String, Value>, String> {
    let mut spec = kind["params"].as_object().ok_or(CATALOG_BAD)?.clone();
    if schema == 2 {
        if let Some(p2) = kind["params2"].as_object() {
            spec.extend(p2.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
        if matches!(geom::form_of(kname), "box" | "table" | "jaw") {
            if let Some(b) = catalog()["box_params"].as_object() {
                spec.extend(b.iter().map(|(k, v)| (k.clone(), v.clone())));
            }
        }
    }
    Ok(spec)
}

fn check_params(id: &str, spec: &Map<String, Value>, given: Option<&Value>) -> Result<Map<String, Value>, String> {
    let empty = Map::new();
    let given = match given {
        None | Some(Value::Null) => &empty,
        Some(Value::Object(m)) => m,
        Some(_) => return Err(dev_err(id, "params", "must be an object")),
    };
    for k in given.keys() {
        if !spec.contains_key(k) {
            return Err(dev_err(id, &format!("params.{k}"), "unknown parameter for this kind"));
        }
    }
    let mut out = Map::new();
    for (k, s) in spec {
        let f = format!("params.{k}");
        let ty = s["type"].as_str().unwrap_or("");
        let v = match given.get(k) {
            // 값이 없으면 정해지지 않은 채 둔다: `derive`는 `geom.rs`가 구하고, `optional`은 기록용이다
            None if s["derive"] == true || s["optional"] == true => continue,
            None => s["default"].clone(),
            Some(v) => match ty {
                "number" => json!(num_in(v, s["min"].as_f64().unwrap_or(f64::MIN), s["max"].as_f64().unwrap_or(f64::MAX)).ok_or_else(|| dev_err(id, &f, &format!("must be a number from {} to {}", s["min"], s["max"])))?),
                "enum" => v.as_str().filter(|t| s["values"].as_array().is_some_and(|a| a.iter().any(|x| x == t))).map(|t| json!(t)).ok_or_else(|| dev_err(id, &f, &format!("must be one of {}", s["values"])))?,
                _ => v.as_bool().map(|b| json!(b)).ok_or_else(|| dev_err(id, &f, "must be true or false"))?,
            },
        };
        out.insert(k.clone(), v);
    }
    Ok(out)
}

fn check_signals(id: &str, v: &Value, schema: i64, addrs: &dyn Addresses) -> Result<Vec<Slot>, String> {
    let a = v.as_array().filter(|a| a.len() <= MAX_SIGNALS).ok_or_else(|| dev_err(id, "signals", &format!("must be an array of at most {MAX_SIGNALS}")))?;
    let mut out = Vec::new();
    for (i, s) in a.iter().enumerate() {
        let f = format!("signals[{i}]");
        let o = s.as_object().ok_or_else(|| dev_err(id, &f, "must be an object"))?;
        if let Some(k) = o.keys().find(|k| !matches!(k.as_str(), "name" | "dir" | "type" | "device" | "active")) {
            return Err(dev_err(id, &format!("{f}.{k}"), "unknown field"));
        }
        let name = o.get("name").and_then(Value::as_str).filter(|n| (1..=24).contains(&n.len()) && n.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')).ok_or_else(|| dev_err(id, &format!("{f}.name"), "must be 1 to 24 characters of A-Z a-z 0-9 _ -"))?;
        let outdir = match o.get("dir").and_then(Value::as_str) {
            Some("out") => true,
            Some("in") => false,
            _ => return Err(dev_err(id, &format!("{f}.dir"), "must be \"in\" or \"out\"")),
        };
        let (word, bcd) = match o.get("type").and_then(Value::as_str) {
            Some("bit") => (false, false),
            Some("word") => (true, false),
            Some("bcd") => (true, true),
            _ => return Err(dev_err(id, &format!("{f}.type"), "must be bit, word or bcd")),
        };
        if let Some(a) = o.get("active") {
            if !matches!(a.as_str(), Some("high" | "low")) {
                return Err(dev_err(id, &format!("{f}.active"), "must be \"high\" or \"low\""));
            }
        }
        // 형식 2: `device`가 없으면 주소 없는 신호이다(절차가 이름을 먼저 쓰고 주소는 나중에 묶는다)
        let bound = !(schema == 2 && o.get("device").is_none_or(Value::is_null));
        let addr = if bound { addrs.check(o.get("device").and_then(Value::as_str).unwrap_or(""), outdir, word).map_err(|m| dev_err(id, &format!("{f}.device"), &m))? } else { String::new() };
        out.push(Slot { name: name.into(), addr, out: outdir, word, bound, bcd, low: o.get("active").and_then(Value::as_str) == Some("low") });
    }
    Ok(out)
}

/// 핀 이름 `<장치 id>.<칸 또는 신호 이름>`을 둘로 가른다(장치 id는 1~32자, 이름은 1~24자, 둘 다 `A-Z a-z 0-9 _ -`). 모양만 본다.
pub fn split_pin(s: &str) -> Option<(&str, &str)> {
    let (id, n) = s.split_once('.')?;
    let ok = |t: &str, max: usize| (1..=max).contains(&t.len()) && t.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    (ok(id, 32) && ok(n, 24)).then_some((id, n))
}

/// 신호 이름이 그 종류의 칸 이름과 같으면 핀 이름이 둘이 된다(설계 46의 2.3절).
pub fn pin_clash(id: &str, kind: &str, i: usize, name: &str) -> String {
    dev_err(id, &format!("signals[{i}].name"), &format!("{name} is a slot of {kind}; a pin name is used once"))
}

/// 주소 없는 호스트: 종류의 모든 `io` 칸과 모든 신호가 핀 이름(`<id>.<이름>`)을 주소로 가진다.
fn fill_pins(id: &str, kind: &str, spec: &Map<String, Value>, slots: &mut Vec<Slot>, signals: &mut [Slot]) -> Result<(), String> {
    if let Some((i, s)) = signals.iter().enumerate().find(|(_, s)| spec.contains_key(&s.name)) {
        return Err(pin_clash(id, kind, i, &s.name));
    }
    for (k, s) in spec {
        slots.push(Slot { name: k.clone(), addr: format!("{id}.{k}"), out: s["dir"] == "out", word: s["type"] == "word", bound: true, bcd: false, low: false });
    }
    for s in signals {
        s.addr = format!("{id}.{}", s.name);
        s.bound = true;
    }
    Ok(())
}

/// 장면의 핀과 그 주소(묶인 것만): `<id>.<이름>` → 정규화한 주소. 호스트가 쪽에 `show`·`indicators`의 핀을 주소로 풀어 줄 때 쓴다.
pub fn pin_addresses(s: &Scene) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    for d in &s.devices {
        for x in d.slots.iter().chain(&d.signals).filter(|x| x.bound) {
            m.insert(format!("{}.{}", d.id, x.name), x.addr.clone());
        }
    }
    m
}

/// JSON 값에서 장면을 읽고 모든 검사를 한다. `name`은 폴더 이름(이미 `valid_scene_name`이어야 한다).
/// 주소의 규칙은 호스트가 `addrs`로 준다(plc-mcp는 `twin::scene::parse`가 자기 규칙을 넣어 부른다).
pub fn parse_with(name: &str, v: &Value, addrs: &'static dyn Addresses) -> Result<Scene, String> {
    if !valid_scene_name(name) {
        return Err("scene name must be 1 to 32 characters of A-Z a-z 0-9 - _ (and not a Windows device name)".into());
    }
    if v.to_string().len() > MAX_SCENE_BYTES {
        return Err(format!("scene is larger than {MAX_SCENE_BYTES} bytes"));
    }
    let o = v.as_object().ok_or("scene must be a JSON object")?;
    if let Some(k) = o.keys().find(|k| !matches!(k.as_str(), "schema" | "name" | "photos" | "devices" | "links" | "refs" | "points" | "workpieces" | "modules")) {
        return Err(format!("scene.{k}: unknown field"));
    }
    let schema = match o.get("schema").and_then(Value::as_i64) {
        Some(s @ (1 | 2)) => s,
        _ => return Err("scene.schema: must be 1 or 2".into()),
    };
    if schema == 1 && o.get("refs").is_some_and(|r| !r.is_null()) {
        return Err("scene.refs: needs scene.schema 2".into());
    }
    // 공작물 키(작업 B1)는 형식 2에서만 쓴다
    if schema == 1 {
        if let Some(k) = ["points", "workpieces", "modules"].iter().find(|k| o.get(**k).is_some_and(|v| !v.is_null())) {
            return Err(format!("scene.{k}: needs scene.schema 2"));
        }
    }
    if let Some(n) = o.get("name") {
        if let Some(why) = text_problem(n, MAX_LABEL) {
            return Err(format!("scene.name: {why}"));
        }
    }
    match o.get("photos") {
        None | Some(Value::Null) => {}
        Some(p) => {
            let a = p.as_array().filter(|a| a.len() <= MAX_PHOTOS).ok_or(format!("scene.photos: must be an array of at most {MAX_PHOTOS}"))?;
            for (i, ph) in a.iter().enumerate() {
                let f = ph.get("file").and_then(Value::as_str).filter(|f| (1..=40).contains(&f.len()) && f.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-')) && !f.contains(".."));
                if f.is_none() {
                    return Err(format!("scene.photos[{i}].file: must be a plain file name"));
                }
                if schema == 2 {
                    // 형식 2: 사진 항목은 file과 state만 가진다. 카메라는 화면에서 정한다.
                    if ph.get("camera").is_some_and(|c| !c.is_null()) {
                        return Err(format!("scene.photos[{i}].camera: the camera of a photo is set on the page (schema 2)"));
                    }
                    if let Some(k) = ph.as_object().and_then(|m| m.keys().find(|k| !matches!(k.as_str(), "file" | "state" | "camera"))) {
                        return Err(format!("scene.photos[{i}].{}: unknown field", k.chars().take(24).collect::<String>()));
                    }
                }
                if let Some(c) = ph.get("camera").filter(|c| !c.is_null()) {
                    if !(triple(&c["pos"], MAX_POSE) && triple(&c["target"], MAX_POSE) && num_in(&c["fov"], 1.0, 179.0).is_some()) {
                        return Err(format!("scene.photos[{i}].camera: needs pos, target (3 numbers) and fov (1 to 179)"));
                    }
                }
            }
        }
    }
    let devs = o.get("devices").and_then(Value::as_array).filter(|a| a.len() <= MAX_DEVICES).ok_or(format!("scene.devices: must be an array of at most {MAX_DEVICES}"))?;
    // 종류 목록은 `check_catalog` 시험이 모양을 보증한다. 그래도 요청 경로에서는 패닉하지 않는다.
    let kinds = catalog()["kinds"].as_object().ok_or(CATALOG_BAD)?;
    let (mut ids, mut outs) = (HashSet::new(), HashSet::new());
    let mut shape_total = 0usize;
    let (mut skin_total, mut lamp_total) = (0usize, 0usize);
    let mut pending: Vec<SkinParsed> = Vec::new();
    let mut refs = Vec::new();
    let mut work = None;
    let mut skins: Vec<SkinInfo> = Vec::new();
    // 장치, 관절, 결합, 구동기, 센서, 표 점, 쪽모이 구간의 장면 합계(`mech`의 한도)
    let mut mech_totals = [0usize; 7];
    let mut raws: Vec<(usize, super::work::RawDev)> = Vec::new();
    let mut devices = Vec::with_capacity(devs.len());
    for (i, d) in devs.iter().enumerate() {
        let d = d.as_object().ok_or(format!("scene.devices[{i}]: must be an object"))?;
        let id = d.get("id").and_then(Value::as_str).filter(|s| plain_id(s)).ok_or(format!("scene.devices[{i}].id: must be 1 to 32 characters of A-Z a-z 0-9 - _"))?;
        if RESERVED_IDS.contains(&id) {
            return Err(format!("scene.devices[{i}].id: {id} is reserved (it is a property name of a JavaScript object); use another id"));
        }
        if !ids.insert(id.to_string()) {
            return Err(dev_err(id, "id", "is used twice"));
        }
        // 거부할 키가 여럿이면 `mech`를 먼저 부른다(형식 1 장면의 `mech`가 가장 알아야 할 까닭이다)
        if let Some(k) = d.keys().filter(|k| !matches!(k.as_str(), "id" | "kind" | "label" | "pose" | "parts" | "params" | "io" | "timing" | "signals" | "checked") && !(schema == 2 && S2_KEYS.contains(&k.as_str()))).min_by_key(|k| k.as_str() != "mech") {
            return Err(dev_err(id, k, if S2_KEYS.contains(&k.as_str()) { "needs scene.schema 2" } else { "unknown field" }));
        }
        if schema == 2 && d.get("parts").is_some_and(|p| !p.is_null()) {
            return Err(dev_err(id, "parts", "use params and valve_on (schema 2)"));
        }
        let kname = d.get("kind").and_then(Value::as_str).ok_or_else(|| dev_err(id, "kind", "is needed"))?;
        let kind = kinds.get(kname).ok_or_else(|| dev_err(id, "kind", &format!("`{}` is not in the catalog", kname.chars().take(40).collect::<String>())))?;
        if schema == 1 && kind["form"] == "mech" {
            return Err(format!("device {id}: kind {kname} needs scene.schema 2"));
        }
        // 이름표는 검사만 한다(화면이 JSON에서 읽는다)
        if let Some(l) = d.get("label").filter(|l| !l.is_null()) {
            if let Some(why) = text_problem(l, MAX_LABEL) {
                return Err(dev_err(id, "label", &why));
            }
        }
        if let Some(p) = d.get("pose").filter(|p| !p.is_null()) {
            if !(p.is_object() && (p.get("pos").is_none() || triple(&p["pos"], MAX_POSE)) && (p.get("rot").is_none() || triple(&p["rot"], 360.0)) && p.as_object().is_some_and(|m| m.keys().all(|k| k == "pos" || k == "rot"))) {
                return Err(dev_err(id, "pose", "must be {pos:[x,y,z], rot:[x,y,z]} with numbers in range"));
            }
        }
        let tri = |v: &Value| -> [f64; 3] { [v[0].as_f64().unwrap_or(0.0), v[1].as_f64().unwrap_or(0.0), v[2].as_f64().unwrap_or(0.0)] };
        let pose = d.get("pose").filter(|p| !p.is_null());
        let pos = pose.map(|p| tri(&p["pos"])).unwrap_or([0.0; 3]);
        let rot_y = pose.and_then(|p| p["rot"][1].as_f64()).unwrap_or(0.0);
        // 부품 위치 덮어쓰기: 종류가 가진 부품만, {pos, rot} 범위 검사
        let mut part_pos = BTreeMap::new();
        match d.get("parts").filter(|p| !p.is_null()) {
            None => {}
            Some(Value::Object(m)) => {
                for (pn, p) in m {
                    if !super::layout::has_part(kname, pn) {
                        return Err(dev_err(id, &format!("parts.{pn}"), &format!("`{}` is not a part of {kname}", pn.chars().take(40).collect::<String>())));
                    }
                    let ok = p.as_object().is_some_and(|o| o.keys().all(|k| k == "pos" || k == "rot")) && (p.get("pos").is_none() || triple(&p["pos"], MAX_POSE)) && (p.get("rot").is_none() || triple(&p["rot"], 360.0));
                    if !ok {
                        return Err(dev_err(id, &format!("parts.{pn}"), "must be {pos:[x,y,z], rot:[x,y,z]} with numbers in range"));
                    }
                    if p.get("pos").is_some() {
                        part_pos.insert(pn.clone(), tri(&p["pos"]));
                    }
                }
            }
            Some(_) => return Err(dev_err(id, "parts", "must be an object")),
        }
        let pspec = param_spec(kname, kind, schema)?;
        let params = check_params(id, &pspec, d.get("params"))?;
        // io 칸: 종류가 가진 칸만, 방향에 맞는 주소만
        let spec = kind["io"].as_object().ok_or(CATALOG_BAD)?;
        if kname == "solid" && d.get("io").is_some_and(|v| !v.is_null()) {
            return Err(dev_err(id, "io", "a solid has no I/O"));
        }
        let mut slots = Vec::new();
        match d.get("io") {
            None | Some(Value::Null) => {}
            Some(Value::Object(m)) => {
                for (k, a) in m {
                    let s = spec.get(k).ok_or_else(|| dev_err(id, &format!("io.{k}"), &format!("`{k}` is not a slot of {kname}; slots: {}", spec.keys().cloned().collect::<Vec<_>>().join(", "))))?;
                    let (out, word) = (s["dir"] == "out", s["type"] == "word");
                    let addr = addrs.check(a.as_str().unwrap_or(""), out, word).map_err(|m| dev_err(id, &format!("io.{k}"), &m))?;
                    slots.push(Slot { name: k.clone(), addr, out, word, bound: true, bcd: false, low: false });
                }
            }
            Some(_) => return Err(dev_err(id, "io", "must be an object")),
        }
        let mut signals = match d.get("signals").filter(|s| !s.is_null()) {
            None => Vec::new(),
            Some(s) if kind["signals"] == true => check_signals(id, s, schema, addrs)?,
            Some(_) => return Err(dev_err(id, "signals", &format!("{kname} has no signal list; use io slots"))),
        };
        // 주소 없는 호스트: 모든 칸과 신호가 핀 이름을 주소로 가진다. `mech`와 부품 규칙은 그 뒤에 이 칸들을 본다.
        if addrs.virtual_pins() {
            fill_pins(id, kname, spec, &mut slots, &mut signals)?;
        }
        for s in slots.iter().chain(&signals).filter(|s| s.out && !s.word && s.bound) {
            if !outs.insert(s.addr.clone()) {
                return Err(dev_err(id, &format!("io.{}", s.name), &format!("output address {} is already used by another output slot", s.addr)));
            }
        }
        // 시간표: 시간이 있는 종류만
        let mut timing = Timing { lag_ms: 50.0, fwd_ms: 400.0, back_ms: 400.0 };
        match d.get("timing").filter(|t| !t.is_null()) {
            None => {}
            Some(t) if kind["timing"] == true => {
                let m = t.as_object().ok_or_else(|| dev_err(id, "timing", "must be an object"))?;
                for (k, v) in m {
                    let f = format!("timing.{k}");
                    match k.as_str() {
                        "lag_ms" => timing.lag_ms = num_in(v, 0.0, MAX_MS).ok_or_else(|| dev_err(id, &f, "must be 0 to 60000"))?,
                        "fwd_ms" => timing.fwd_ms = num_in(v, 0.0, MAX_MS).ok_or_else(|| dev_err(id, &f, "must be 0 to 60000"))?,
                        "back_ms" => timing.back_ms = num_in(v, 0.0, MAX_MS).ok_or_else(|| dev_err(id, &f, "must be 0 to 60000"))?,
                        "count" => {
                            if !v.as_u64().is_some_and(|n| (1..=100_000).contains(&n)) {
                                return Err(dev_err(id, &f, "must be an integer from 1 to 100000"));
                            }
                        }
                        "measured_at" => {
                            if !v.as_str().is_some_and(|s| !s.is_empty() && s.len() <= 32 && s.bytes().all(|b| b.is_ascii_digit() || matches!(b, b'-' | b':' | b'T' | b'Z' | b'.' | b'+'))) {
                                return Err(dev_err(id, &f, "must be an ISO date string of at most 32 characters"));
                            }
                        }
                        "source" => {
                            if !v.as_str().is_some_and(|s| s.len() <= 16 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')) {
                                return Err(dev_err(id, &f, "must be a short word (draft, measured, manual)"));
                            }
                        }
                        _ => return Err(dev_err(id, &f, "unknown field")),
                    }
                }
            }
            Some(_) => return Err(dev_err(id, "timing", &format!("{kname} has no timing"))),
        }
        if timing.fwd_ms == 0.0 || timing.back_ms == 0.0 {
            // 0 ms는 0으로 나누지 않게 1 ms로 본다(순간 이동에 가깝다).
            (timing.fwd_ms, timing.back_ms) = (timing.fwd_ms.max(1.0), timing.back_ms.max(1.0));
        }
        let behaviors: Vec<String> = kind["behaviors"].as_array().ok_or(CATALOG_BAD)?.iter().filter_map(|b| b.as_str().map(String::from)).collect();
        // `checked`는 검사만 한다(값은 화면이 JSON에서 읽는다)
        match d.get("checked") {
            None | Some(Value::Null) => {}
            Some(c) if behaviors.iter().any(|b| b == "valve") => {
                c.as_bool().ok_or_else(|| dev_err(id, "checked", "must be true or false"))?;
            }
            Some(_) => return Err(dev_err(id, "checked", &format!("{kname} has no valve"))),
        }
        if schema == 2 {
            if let Some(r) = super::work::parse_dev(id, kname, &behaviors, d, addrs)? {
                raws.push((devices.len(), r));
            }
        }
        let mut mech = None;
        let geom = if schema == 2 {
            let rot3 = pose.map(|p| tri(&p["rot"])).unwrap_or([0.0; 3]);
            let mut g = parse_geom(id, kname, &pspec, d, pos, rot3, &params)?;
            // 기계 모델 엔진 키(`mech`): 종류가 허용해야 하고, 관절 틀은 외형이 쓰기 전에 틀 목록에 붙는다
            match d.get("mech").filter(|v| !v.is_null()) {
                None if kind["mech"] == true && kname == "machine" => return Err(dev_err(id, "mech", "kind machine needs mech")),
                None => {}
                Some(_) if kind["mech"] != true => {
                    let mut ok: Vec<&str> = kinds.iter().filter(|(_, k)| k["mech"] == true).map(|(n, _)| n.as_str()).collect();
                    ok.sort_unstable();
                    return Err(dev_err(id, "mech", &format!("kind {kname} takes no mech (kinds: {})", ok.join(", "))));
                }
                Some(_) if d.get("position").is_some_and(|v| !v.is_null()) => return Err(dev_err(id, "mech", "give position or mech, not both (position is the short form of a follow driver)")),
                Some(mv) => {
                    // 트윈 모델은 `io`가 없다(설계 46의 2.5절): `mech`가 이름으로 부르는 종류의 칸이 `io`에 없으면 주소 없는 칸으로 둔다.
                    // 그러면 모델은 그대로 장면이고, `mech`를 돌리는 쪽(`twin_sim_run`)이 주소를 묶으라고 거절한다(주소 없는 신호와 같은 규칙).
                    if !addrs.virtual_pins() {
                        let text = mv.to_string();
                        let add: Vec<Slot> = spec
                            .iter()
                            .filter(|(k, _)| !slots.iter().chain(&signals).any(|x| &x.name == *k) && text.contains(&format!("\"{k}\"")))
                            .map(|(k, s)| Slot { name: k.clone(), addr: String::new(), out: s["dir"] == "out", word: s["type"] == "word", bound: false, bcd: false, low: false })
                            .collect();
                        slots.extend(add);
                    }
                    let sigs: Vec<super::mech::Sig> = slots.iter().chain(&signals).map(|s| super::mech::Sig { name: s.name.clone(), out: s.out, word: s.word, bound: s.bound, bcd: s.bcd }).collect();
                    let frames: Vec<String> = g.frames.iter().map(|f| f.0.clone()).collect();
                    let ctx = super::mech::Ctx { frames: &frames, params: &params, sigs: &sigs, form_range: g.axis.as_ref().filter(|a| a.linear).map(|a| a.range), kind: kname, presets: &catalog()["presets"] };
                    let m = super::mech::parse(id, mv, &ctx)?;
                    mech_totals[0] += 1;
                    for (k, n) in [m.joints.len(), m.couplings.len(), m.drivers.len(), m.sensors.len(), m.n_points, m.n_segments].into_iter().enumerate() {
                        mech_totals[k + 1] += n;
                    }
                    geom::add_joint_frames(&mut g, &m);
                    mech = Some(m);
                }
            }
            shape_total += g.shape.len();
            if shape_total > geom::MAX_SHAPE_SCENE {
                return Err(format!("scene: more than {} shape items", geom::MAX_SHAPE_SCENE));
            }
            if ["skin", "skin_source", "indicators"].iter().any(|k| d.get(*k).is_some_and(|v| !v.is_null())) {
                let p = parse_skin(id, d, &g.frames, addrs)?;
                (skin_total, lamp_total) = (skin_total + p.info.items, lamp_total + p.info.lamps);
                if skin_total > MAX_SKIN_SCENE {
                    return Err(format!("scene: more than {MAX_SKIN_SCENE} skin items"));
                }
                if lamp_total > MAX_LAMP_SCENE {
                    return Err(format!("scene: more than {MAX_LAMP_SCENE} indicator lamps"));
                }
                pending.push(p);
            }
            Some(g)
        } else {
            None
        };
        devices.push(Dev { id: id.into(), kind: kname.into(), behaviors, simulated: kind["simulated"] == true, params, slots, signals, timing, mech, pos, rot_y, parts: part_pos, geom });
    }
    check_mech(&devices, mech_totals)?;
    if schema == 2 {
        // 장치 나무: `on`을 풀어 세계 변환을 구한다. 설비 모델에 주는 pos와 rot_y는 세계의 `base` 원점과 heading이다.
        let ids: Vec<String> = devices.iter().map(|d| d.id.clone()).collect();
        let kinds: Vec<String> = devices.iter().map(|d| d.kind.clone()).collect();
        let mut gs: Vec<geom::DevGeom> = devices.iter_mut().filter_map(|d| d.geom.take()).collect();
        let (idr, kr): (Vec<&str>, Vec<&str>) = (ids.iter().map(String::as_str).collect(), kinds.iter().map(String::as_str).collect());
        geom::resolve(&idr, &kr, &mut gs)?;
        if let Some(r) = o.get("refs").filter(|r| !r.is_null()) {
            refs = geom::parse_refs(r, &idr, &kr, &gs)?;
        }
        for (d, g) in devices.iter_mut().zip(gs) {
            (d.pos, d.rot_y) = geom::bridge(&g.world);
            d.geom = Some(g);
        }
        check_photo_states(o.get("photos"), &devices)?;
        skins = finish_skins(pending, &devices)?;
        work = super::work::build(o, &devices, raws, addrs)?;
    }
    let links = super::layout::parse_links(o.get("links"), &devices)?;
    Ok(Scene { schema, refs, skins, work, name: name.into(), devices, links, addrs })
}

/// 외형이나 표시등이 있는 장치 하나(`facts.skin_counts`, 어두운 표시등의 `notes`)
#[derive(Clone, Debug, Default)]
pub struct SkinInfo {
    pub id: String,
    pub has_skin: bool,
    /// 사람이 확인했다고 적힌 외형(`skin_source` = checked)
    pub checked: bool,
    pub items: usize,
    pub lamps: usize,
    /// 장면의 어느 장치도 갖지 않은 표시등 주소(어둡게 그려진다)
    pub dark: Vec<String>,
    /// 호스트가 주소를 묶지 않은 핀을 `show`나 `hide`가 가리키는 경우의 핀 이름(그 그룹은 핀이 꺼진 것처럼 그려진다)
    pub unbound: Vec<String>,
}

/// `show`·`hide`·표시등이 가리키는 것: 정규화한 비트 주소, 또는 핀(`<장치 id>`, `<이름>`)
#[derive(Clone, Debug)]
enum SkinRef {
    Addr(String),
    Pin(String, String),
}

/// 읽은 결과: `show`·`hide`(필드 경로, 가리키는 것)와 표시등 줄(필드 경로, 줄마다 가리키는 것들). 장면의 주소와 핀은 모든 장치를 읽은 뒤에 본다.
struct SkinParsed {
    info: SkinInfo,
    refs: Vec<(String, SkinRef)>,
    rows: Vec<(String, Vec<SkinRef>)>,
}

/// 주소 없는 호스트가 `show`·`hide`·`indicators`에 주는 오류 글
const PIN_ONLY: &str = "a twin model holds no PLC address; use a pin \"<device id>.<signal>\"";
const PINS_ONLY: &str = "a twin model holds no PLC address; use pins: [\"<device id>.<signal>\", ...]";

fn group_name_ok(s: &str) -> bool {
    (1..=16).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// 장치 틀 이름 검사(외형 그룹과 표시등 줄이 같다). 오류 글은 `on`과 같은 모양이다.
fn check_frame(id: &str, field: &str, v: Option<&Value>, frames: &[(String, geom::Xf, bool)]) -> Result<(), String> {
    let Some(v) = v.filter(|v| !v.is_null()) else { return Ok(()) };
    let name = v.as_str().ok_or_else(|| dev_err(id, field, "must be text"))?;
    if frames.iter().any(|(n, _, _)| n == name) {
        return Ok(());
    }
    let names: Vec<&str> = frames.iter().map(|(n, _, _)| n.as_str()).collect();
    Err(dev_err(id, field, &format!("{id} has no frame \"{}\" (frames: {})", name.chars().take(24).collect::<String>(), names.join(", "))))
}

/// `skin`, `skin_source`, `indicators`를 읽고 검사한다(개수를 먼저 본다). `frames`는 이 장치의 틀 목록이다.
fn parse_skin(id: &str, d: &Map<String, Value>, frames: &[(String, geom::Xf, bool)], addrs: &dyn Addresses) -> Result<SkinParsed, String> {
    let mut p = SkinParsed { info: SkinInfo { id: id.into(), ..Default::default() }, refs: Vec::new(), rows: Vec::new() };
    let virt = addrs.virtual_pins();
    let skin = d.get("skin").filter(|v| !v.is_null());
    if let Some(v) = d.get("skin_source").filter(|v| !v.is_null()) {
        let s = v.as_str().filter(|s| matches!(*s, "model" | "checked")).ok_or_else(|| dev_err(id, "skin_source", "must be model or checked"))?;
        if skin.is_none() {
            return Err(dev_err(id, "skin_source", "the device has no skin"));
        }
        p.info.checked = s == "checked";
    }
    if let Some(s) = skin {
        let o = s.as_object().ok_or_else(|| dev_err(id, "skin", "must be an object"))?;
        if let Some(k) = o.keys().find(|k| *k != "groups") {
            return Err(dev_err(id, &format!("skin.{}", k.chars().take(24).collect::<String>()), "unknown field"));
        }
        let groups = o.get("groups").and_then(Value::as_array).filter(|a| (1..=MAX_SKIN_GROUPS).contains(&a.len())).ok_or_else(|| dev_err(id, "skin.groups", &format!("must be 1 to {MAX_SKIN_GROUPS} groups")))?;
        // 개수와 크기는 항목을 읽기 전에 본다
        let total: usize = groups.iter().filter_map(|g| g.get("items").and_then(Value::as_array)).map(Vec::len).sum();
        if total > MAX_SKIN_ITEMS {
            return Err(dev_err(id, "skin", &format!("more than {MAX_SKIN_ITEMS} items")));
        }
        if s.to_string().len() > MAX_SKIN_BYTES {
            return Err(dev_err(id, "skin", &format!("more than {MAX_SKIN_BYTES} bytes")));
        }
        let mut names: Vec<&str> = Vec::new();
        for (i, g) in groups.iter().enumerate() {
            let f = format!("skin.groups[{i}]");
            let g = g.as_object().ok_or_else(|| dev_err(id, &f, "must be an object"))?;
            if let Some(k) = g.keys().find(|k| !matches!(k.as_str(), "name" | "frame" | "show" | "hide" | "items")) {
                return Err(dev_err(id, &format!("{f}.{}", k.chars().take(24).collect::<String>()), "unknown field"));
            }
            let name = g.get("name").and_then(Value::as_str).filter(|n| group_name_ok(n)).ok_or_else(|| dev_err(id, &format!("{f}.name"), "must be 1 to 16 of a-z 0-9 _"))?;
            if names.contains(&name) {
                return Err(dev_err(id, &format!("{f}.name"), &format!("\"{name}\" is used twice")));
            }
            names.push(name);
            check_frame(id, &format!("{f}.frame"), g.get("frame"), frames)?;
            let (show, hide) = (g.get("show").filter(|v| !v.is_null()), g.get("hide").filter(|v| !v.is_null()));
            if show.is_some() && hide.is_some() {
                return Err(dev_err(id, &f, "give show or hide, not both"));
            }
            for (key, v) in [("show", show), ("hide", hide)] {
                if let Some(v) = v {
                    // 주소 형식이 먼저이고(오늘의 동작 그대로), 아니면 핀 형식 `<장치 id>.<이름>`
                    let t = v.as_str();
                    let r = match (t.and_then(|t| addrs.bit(t)), t.and_then(split_pin)) {
                        (Some(a), _) => SkinRef::Addr(a),
                        (None, Some((pid, pn))) => SkinRef::Pin(pid.into(), pn.into()),
                        (None, None) => return Err(dev_err(id, &format!("{f}.{key}"), if virt { PIN_ONLY } else { "must be an X, Y or M bit address" })),
                    };
                    p.refs.push((format!("{f}.{key}"), r));
                }
            }
            let items = g.get("items").and_then(Value::as_array).filter(|a| (1..=MAX_SKIN_ITEMS).contains(&a.len())).ok_or_else(|| dev_err(id, &format!("{f}.items"), &format!("must be 1 to {MAX_SKIN_ITEMS} items")))?;
            for (j, it) in items.iter().enumerate() {
                let item = geom::parse_item(it, true).map_err(|(fld, m)| dev_err(id, &if fld.is_empty() { format!("{f}.items[{j}]") } else { format!("{f}.items[{j}].{fld}") }, &m))?;
                if item.tone.as_deref() == Some("mark") && show.is_none() {
                    return Err(dev_err(id, &format!("{f}.items[{j}].tone"), "mark needs a group with show"));
                }
            }
        }
        p.info.has_skin = true;
        p.info.items = total;
    }
    if let Some(v) = d.get("indicators").filter(|v| !v.is_null()) {
        let rows = v.as_array().ok_or_else(|| dev_err(id, "indicators", "must be an array"))?;
        if rows.len() > MAX_IND_ROWS {
            return Err(dev_err(id, "indicators", &format!("at most {MAX_IND_ROWS} rows")));
        }
        for (i, r) in rows.iter().enumerate() {
            let f = format!("indicators[{i}]");
            let r = r.as_object().ok_or_else(|| dev_err(id, &f, "must be an object"))?;
            if let Some(k) = r.keys().find(|k| !matches!(k.as_str(), "from" | "count" | "pins" | "at" | "step" | "frame" | "d")) {
                return Err(dev_err(id, &format!("{f}.{}", k.chars().take(24).collect::<String>()), "unknown field"));
            }
            // 줄의 램프는 `from` + `count`(주소 형식, 오늘의 동작 그대로) 또는 `pins`(1~64개, 목록 순서가 램프 순서)이다. 둘을 같이 주지 못한다.
            let pins = r.get("pins").filter(|v| !v.is_null());
            let has_fc = ["from", "count"].iter().any(|k| r.get(*k).is_some_and(|v| !v.is_null()));
            if pins.is_some() && has_fc {
                return Err(dev_err(id, &f, "give pins or from and count, not both"));
            }
            if virt && has_fc {
                return Err(dev_err(id, &format!("{f}.from"), PINS_ONLY));
            }
            let (row, count): (Vec<SkinRef>, u32) = if virt || pins.is_some() {
                let a = pins.and_then(Value::as_array).filter(|a| (1..=64).contains(&a.len())).ok_or_else(|| dev_err(id, &format!("{f}.pins"), "must be 1 to 64 pins"))?;
                let row = a.iter().enumerate().map(|(k, x)| x.as_str().and_then(split_pin).map(|(pid, pn)| SkinRef::Pin(pid.into(), pn.into())).ok_or_else(|| dev_err(id, &format!("{f}.pins[{k}]"), "must be a pin \"<device id>.<signal>\""))).collect::<Result<Vec<_>, _>>()?;
                (row, a.len() as u32)
            } else {
                let from = r.get("from").and_then(Value::as_str).filter(|t| addrs.bit(t).is_some()).ok_or_else(|| dev_err(id, &format!("{f}.from"), "must be an X, Y or M bit address"))?;
                let count = r.get("count").and_then(Value::as_u64).filter(|c| (1..=64).contains(c)).ok_or_else(|| dev_err(id, &format!("{f}.count"), "must be 1 to 64"))? as u32;
                if p.info.lamps + count as usize > MAX_IND_LAMPS {
                    return Err(dev_err(id, "indicators", &format!("more than {MAX_IND_LAMPS} lamps")));
                }
                // 줄이 마지막 주소를 넘는지는 호스트의 점수가 정한다
                let names = match addrs.bit_run(from, count) {
                    Some(Ok(names)) => names,
                    Some(Err(m)) => return Err(dev_err(id, &format!("{f}.from"), &m)),
                    None => return Err(dev_err(id, &format!("{f}.from"), "must be an X, Y or M bit address")),
                };
                (names.into_iter().map(SkinRef::Addr).collect(), count)
            };
            if p.info.lamps + count as usize > MAX_IND_LAMPS {
                return Err(dev_err(id, "indicators", &format!("more than {MAX_IND_LAMPS} lamps")));
            }
            r.get("at").and_then(|a| geom::vec3(a, 20_000.0)).ok_or_else(|| dev_err(id, &format!("{f}.at"), "must be 3 numbers from -20000 to 20000"))?;
            match r.get("step").filter(|v| !v.is_null()) {
                None if count > 1 => return Err(dev_err(id, &format!("{f}.step"), "must be 3 numbers from -1000 to 1000 (needed when count is above 1)")),
                None => {}
                Some(s) => {
                    let st = geom::vec3(s, 1000.0).ok_or_else(|| dev_err(id, &format!("{f}.step"), "must be 3 numbers from -1000 to 1000"))?;
                    if count > 1 && st.iter().map(|c| c * c).sum::<f64>().sqrt() < 0.5 {
                        return Err(dev_err(id, &format!("{f}.step"), "must be at least 0.5 mm long"));
                    }
                }
            }
            check_frame(id, &format!("{f}.frame"), r.get("frame"), frames)?;
            if let Some(dia) = r.get("d").filter(|v| !v.is_null()) {
                dia.as_f64().filter(|x| x.is_finite() && (1.0..=50.0).contains(x)).ok_or_else(|| dev_err(id, &format!("{f}.d"), "must be 1 to 50"))?;
            }
            p.info.lamps += count as usize;
            p.rows.push((f, row));
        }
    }
    Ok(p)
}

/// 모든 장치를 읽은 뒤: `show`·`hide`가 장면의 주소인지 보고, 장면에 없는 표시등 주소를 모은다.
fn finish_skins(pending: Vec<SkinParsed>, devices: &[Dev]) -> Result<Vec<SkinInfo>, String> {
    let known: HashSet<&str> = devices.iter().flat_map(|d| d.slots.iter().chain(d.signals.iter())).filter(|s| !s.word && s.bound).map(|s| s.addr.as_str()).collect();
    let mut out = Vec::new();
    for mut p in pending {
        for (f, r) in &p.refs {
            match r {
                SkinRef::Addr(a) if !known.contains(a.as_str()) => return Err(dev_err(&p.info.id, f, &format!("{a} is not an address of the scene"))),
                SkinRef::Addr(_) => {}
                // 핀: 장면에 있는 비트 핀이어야 하고, 호스트가 주소를 묶은 핀이어야 한다(묶임은 호스트의 `io`와 `signals[].device`가 정한다)
                SkinRef::Pin(pid, pn) => match pin_state(devices, pid, pn).map_err(|m| dev_err(&p.info.id, f, &m))? {
                    PinState::Bit => {}
                    PinState::Word => return Err(dev_err(&p.info.id, f, &format!("{pid}.{pn} is a word pin; show and hide take a bit pin"))),
                    // 주소 없는 신호처럼 받아들이고 알린다: 그룹은 핀이 꺼진 것처럼 그려진다
                    PinState::Unbound => {
                        // 같은 핀을 여러 그룹이나 show와 hide가 가리켜도 한 번만 센다(R3-9)
                        let n = format!("{pid}.{pn}");
                        if !p.info.unbound.contains(&n) {
                            p.info.unbound.push(n);
                        }
                    }
                },
            }
        }
        let id = p.info.id.clone();
        for (f, row) in &p.rows {
            for (k, r) in row.iter().enumerate() {
                match r {
                    SkinRef::Addr(a) if !known.contains(a.as_str()) => p.info.dark.push(a.clone()),
                    SkinRef::Addr(_) => {}
                    // 없는 핀은 오류이다(이름은 모델이 정한다). 묶이지 않은 핀의 램프는 주소를 모르는 램프처럼 어둡다(`notes`가 알린다).
                    SkinRef::Pin(pid, pn) => match pin_state(devices, pid, pn).map_err(|m| dev_err(&id, &format!("{f}.pins[{k}]"), &m))? {
                        PinState::Bit => {}
                        PinState::Word => return Err(dev_err(&id, &format!("{f}.pins[{k}]"), &format!("{pid}.{pn} is a word pin; a lamp takes a bit pin"))),
                        PinState::Unbound => p.info.dark.push(format!("{pid}.{pn}")),
                    },
                }
            }
        }
        out.push(p.info);
    }
    Ok(out)
}

/// 핀 하나의 장면에서의 상태
enum PinState {
    Bit,
    Word,
    /// 이름은 있지만 호스트가 주소를 묶지 않았다(plc-mcp: 그 칸이 `io`에, 그 신호가 `device`에 없다)
    Unbound,
}

/// 핀 `<id>.<이름>`을 찾는다: 장치의 묶인 칸과 신호, 그 종류가 가진 칸(`io`에 안 적었으면 묶이지 않은 핀). 없으면 오류 글(앞말 없음).
fn pin_state(devices: &[Dev], pid: &str, pn: &str) -> Result<PinState, String> {
    let Some(d) = devices.iter().find(|d| d.id == pid) else {
        let ids: Vec<&str> = devices.iter().map(|d| d.id.as_str()).take(16).collect();
        return Err(format!("no device \"{}\" (devices: {})", pid.chars().take(32).collect::<String>(), ids.join(", ")));
    };
    if let Some(s) = d.slots.iter().chain(&d.signals).find(|s| s.name == pn) {
        return Ok(match (s.bound, s.word) {
            (true, false) => PinState::Bit,
            (true, true) => PinState::Word,
            (false, _) => PinState::Unbound,
        });
    }
    let spec = catalog()["kinds"][d.kind.as_str()]["io"].as_object();
    match spec.and_then(|m| m.get(pn)) {
        Some(s) if s["type"] == "word" => Ok(PinState::Word),
        Some(_) => Ok(PinState::Unbound),
        None => {
            let mut v: Vec<String> = d.slots.iter().chain(&d.signals).map(|s| format!("{pid}.{}", s.name)).chain(spec.into_iter().flat_map(|m| m.keys().map(move |k| format!("{pid}.{k}")))).collect();
            v.sort();
            v.dedup();
            v.truncate(16);
            Err(format!("no pin \"{pid}.{}\" (pins of {pid}: {})", pn.chars().take(24).collect::<String>(), v.join(", ")))
        }
    }
}

/// 형식 2 장치 키(`on`, `mount`, `basis`, `model`, `shape`, `frames`)를 읽고 검사한다. `on`은 나중에 `geom::resolve`가 푼다.
fn parse_geom(id: &str, kname: &str, spec: &Map<String, Value>, d: &Map<String, Value>, pos: [f64; 3], rot: [f64; 3], params: &Map<String, Value>) -> Result<geom::DevGeom, String> {
    let form = geom::form_of(kname);
    let given_obj = d.get("params").and_then(Value::as_object);
    let on = match d.get("on").filter(|v| !v.is_null()) {
        None => None,
        Some(v) => Some(v.as_str().ok_or_else(|| dev_err(id, "on", "must be \"<id>\" or \"<id>.<frame>\""))?.to_string()),
    };
    // 모양 항목: 개수를 먼저 본다
    let mut shape = Vec::new();
    match d.get("shape").filter(|v| !v.is_null()) {
        None if kname != "solid" => {}
        Some(_) if kname != "solid" => return Err(dev_err(id, "shape", "only kind solid")),
        Some(Value::Array(a)) if (1..=geom::MAX_SHAPE_ITEMS).contains(&a.len()) => {
            for (i, it) in a.iter().enumerate() {
                shape.push(geom::parse_item(it, false).map_err(|(f, m)| dev_err(id, &if f.is_empty() { format!("shape[{i}]") } else { format!("shape[{i}].{f}") }, &m))?);
            }
        }
        _ => return Err(dev_err(id, "shape", &format!("a solid needs 1 to {} items", geom::MAX_SHAPE_ITEMS))),
    }
    let custom = match d.get("frames").filter(|v| !v.is_null()) {
        None => Vec::new(),
        Some(_) if kname != "solid" => return Err(dev_err(id, "frames", "only kind solid (other kinds: a later task)")),
        Some(v) => geom::parse_frames(v).map_err(|(f, m)| dev_err(id, if f.is_empty() { "frames" } else { &f }, &m))?,
    };
    // 고정 방법: 카탈로그 `mounts`의 이름이고 종류의 형태에 맞아야 한다
    let mount = match d.get("mount").filter(|v| !v.is_null()) {
        None => "unknown".to_string(),
        Some(v) => v.as_str().map(String::from).ok_or_else(|| dev_err(id, "mount", "must be text"))?,
    };
    let styles = catalog()["mounts"].as_array().ok_or(CATALOG_BAD)?;
    let Some(style) = styles.iter().find(|s| s["style"] == mount.as_str()) else {
        let names: Vec<&str> = styles.iter().filter_map(|s| s["style"].as_str()).collect();
        return Err(dev_err(id, "mount", &format!("unknown style \"{}\" (styles: {})", mount.chars().take(24).collect::<String>(), names.join(", "))));
    };
    if !style["forms"].as_array().is_some_and(|f| f.iter().any(|x| x == "all" || x == form)) {
        return Err(dev_err(id, "mount", &format!("{mount} is not for the form {form}")));
    }
    if mount == "floor" && on.is_some() {
        return Err(dev_err(id, "mount", "floor needs the world as parent (no on)"));
    }
    // 값의 출처(basis): 매개변수 이름, pose, shape
    let mut basis = BTreeMap::new();
    match d.get("basis").filter(|v| !v.is_null()) {
        None => {}
        Some(Value::Object(m)) if m.len() <= geom::MAX_BASIS => {
            for (k, v) in m {
                let key: String = k.chars().take(24).collect();
                if !(k == "pose" || k == "shape" || k == "position" || spec.contains_key(k)) {
                    return Err(dev_err(id, &format!("basis.{key}"), "not a param, pose or shape"));
                }
                let w = v.as_str().filter(|w| geom::BASIS_VALUES.contains(w)).ok_or_else(|| dev_err(id, &format!("basis.{key}"), "must be user, label, measured, ref or guess"))?;
                basis.insert(k.clone(), w.to_string());
            }
        }
        Some(Value::Object(_)) => return Err(dev_err(id, "basis", &format!("at most {} keys", geom::MAX_BASIS))),
        Some(_) => return Err(dev_err(id, "basis", "must be an object")),
    }
    // model은 검사만 한다(글은 장면 JSON에 남고, facts에는 싣지 않는다)
    if let Some(v) = d.get("model").filter(|v| !v.is_null()) {
        if let Some(why) = text_problem(v, 40) {
            return Err(dev_err(id, "model", &why));
        }
    }
    // 크기와 축, 틀 목록, datum(`pose`가 놓는 이 장치의 틀)
    let dims = geom::dims_of(kname, params, given_obj).map_err(|(f, m)| dev_err(id, &f, &m))?;
    let axis = geom::axis_info(kname, &dims, params);
    let frames = geom::frame_list(kname, &dims, params, &shape, &custom);
    let datum = match d.get("datum").filter(|v| !v.is_null()) {
        None => geom::default_datum(&mount, form),
        Some(v) => v.as_str().map(String::from).ok_or_else(|| dev_err(id, "datum", "must be text"))?,
    };
    let Some((_, datum_xf, _)) = frames.iter().find(|(nm, _, _)| *nm == datum) else {
        let names: Vec<&str> = frames.iter().map(|(nm, _, _)| nm.as_str()).collect();
        let msg = if d.get("datum").is_some_and(|v| !v.is_null()) { format!("no frame \"{}\" (frames: {})", datum.chars().take(24).collect::<String>(), names.join(", ")) } else { format!("the mount {mount} needs the frame {datum}, which the form {form} has not got in this build") };
        return Err(dev_err(id, if d.get("datum").is_some_and(|v| !v.is_null()) { "datum" } else { "mount" }, &msg));
    };
    if geom::is_moving(form, &datum) {
        return Err(dev_err(id, "datum", &format!("{datum} moves with the axis of {kname}")));
    }
    let datum_xf = *datum_xf;
    let given: std::collections::BTreeSet<String> = given_obj.map(|m| m.keys().cloned().collect()).unwrap_or_default();
    let local = geom::place(pos, rot);
    Ok(geom::DevGeom { form, on, mount, datum, datum_xf, shape, frames, basis, given, dims, axis, local, parent: None, world: geom::IDENT, jframes: Vec::new() })
}

/// 장면 전체의 `mech` 검사: 개수 한도와, 엔진이 쓰는 비트에 다른 쓰는 이가 없는지(단일 쓰기).
fn check_mech(devices: &[Dev], t: [usize; 7]) -> Result<(), String> {
    use super::mech as m;
    for (n, max, what) in [(t[0], m::MAX_DEV_SCENE, "devices with mech"), (t[1], m::MAX_JOINTS_SCENE, "joints"), (t[2], m::MAX_COUPLINGS_SCENE, "couplings"), (t[3], m::MAX_DRIVERS_SCENE, "drivers"), (t[4], m::MAX_SENSORS_SCENE, "window sensors"), (t[5], m::MAX_POINTS_SCENE, "table points"), (t[6], m::MAX_SEGMENTS_SCENE, "playback segments")] {
        if n > max {
            return Err(format!("scene: more than {max} {what}"));
        }
    }
    for d in devices {
        let Some(mc) = &d.mech else { continue };
        let sigs: Vec<&Slot> = d.slots.iter().chain(&d.signals).collect();
        for (pin, path) in &mc.writes {
            let me = sigs[*pin];
            if !me.bound {
                continue;
            }
            // 같은 주소를 입력으로 가진 다른 신호(다른 장치의 센서 칸, 이 장치의 다른 이름)
            for o in devices {
                for s in o.slots.iter().chain(&o.signals).filter(|s| s.bound && !s.out && !s.word && s.addr == me.addr) {
                    if !(o.id == d.id && s.name == me.name) {
                        return Err(dev_err(&d.id, &format!("mech.{path}"), &format!("{} is also written by {} ({})", me.addr, o.id, s.name)));
                    }
                }
            }
        }
    }
    Ok(())
}

/// 형식 2 사진의 `state`: 축이 있는 장치의 위치(범위 안), 로봇은 숫자 6개. 개수를 먼저 본다.
fn check_photo_states(photos: Option<&Value>, devices: &[Dev]) -> Result<(), String> {
    for (i, ph) in photos.and_then(Value::as_array).into_iter().flatten().enumerate() {
        let Some(st) = ph.get("state").filter(|s| !s.is_null()) else { continue };
        let m = st.as_object().ok_or(format!("scene.photos[{i}].state: must be an object"))?;
        if m.len() > 64 {
            return Err(format!("scene.photos[{i}].state: at most 64 keys"));
        }
        for (k, v) in m {
            let f = format!("scene.photos[{i}].state.{}", k.chars().take(32).collect::<String>());
            let d = devices.iter().find(|d| d.id == *k);
            match d {
                Some(d) if d.mech.is_some() => return Err(format!("{f}: a device with mech takes no photo state in this build")),
                Some(d) if d.kind == "robot_arm" => {
                    if !v.as_array().is_some_and(|a| a.len() == 6 && a.iter().all(|x| num_in(x, -360.0, 360.0).is_some())) {
                        return Err(format!("{f}: must be 6 numbers from -360 to 360"));
                    }
                }
                Some(d) if d.has("linear") || d.has("rotary") => {
                    // 서보 축(형태 linear)의 값은 지령 주소(mm)라 범위는 home_mm ± 행정이다. 파라미터가 없는 종류(모터, 인덱스 테이블)는 한도가 0이 아니라
                    // 넓은 한도다: 직선 5000 mm(카탈로그 최대), 회전 360 도 (AR4).
                    let given = |k: &str, wide: f64| Some(d.num(k)).filter(|x| *x > 0.0).unwrap_or(wide);
                    let (lo, hi) = if geom::form_of(&d.kind) == "linear" {
                        // 지령 주소의 범위: 주소가 원점에서 멀어지며 커지면(away, 기본) home..home+행정, 아니면(toward) home-행정..home (FR7)
                        let (h, s) = (d.num("home_mm"), given("stroke_mm", 5000.0));
                        if d.text("addr_grows") == "toward" { (h - s, h) } else { (h, h + s) }
                    } else if d.has("linear") {
                        (0.0, given("stroke_mm", 5000.0))
                    } else {
                        (0.0, given("angle_deg", 360.0))
                    };
                    if num_in(v, lo, hi).is_none() {
                        return Err(format!("{f}: must be {lo} to {hi}"));
                    }
                }
                _ => return Err(format!("{f}: not a device with an axis")),
            }
        }
    }
    Ok(())
}

/// 호스트마다 다른 `twin_catalog` 결과의 글(plc-mcp는 `twin::scene::catalog_tool_result`가 자기 글을 넣어 부른다).
pub struct CatalogTexts {
    /// 모델링 절차의 본문(`guide.modeling`)과 그 버전
    pub modeling: String,
    pub modeling_version: u64,
    /// `scene_schema.devices`: 장치 키의 설명(주소 칸은 호스트의 규칙이다)
    pub devices: String,
    /// 결과의 `note`
    pub note: &'static str,
}

/// 모델이 초안을 쓰는 데 필요한 것: 종류 목록과 장면 JSON 요약.
pub fn catalog_tool_result(host: &CatalogTexts) -> Value {
    let c = catalog();
    // 안내문은 한 번만 싣는다(`guide`): 카탈로그 사본에서는 뺀다(보고서 45의 F2). 모델링 절차의 본문은 `guide.modeling`이다(RP8).
    let mut cat = c.clone();
    if let Some(o) = cat.as_object_mut() {
        o.remove("guide");
    }
    let mut guide = c["guide"].clone();
    guide["modeling"] = json!(host.modeling);
    guide["modeling_version"] = json!(host.modeling_version);
    json!({
        "catalog": cat,
        "geometry_note": geom::GEOMETRY_NOTE,
        "frames": geom::frames_table(),
        "mounts": c["mounts"],
        "series": c["series"],
        "tones": c["tones"],
        "guide": guide,
        "scene_schema": {
            "schema": "1 or 2. Schema 2 adds, per device: on (\"<id>\" or \"<id>.<frame>\"; default the world), mount (a style of `mounts` that allows the form of the kind; default unknown; floor needs no `on`), datum (the frame of this device that pose places; default from the mount style, see `mounts[].datum`; never a moving frame), size params per kind (cylinders: series, body_len_mm, body_dia_mm, rod_dia_mm, rod_out_mm, pivot_back_mm, axis_h_mm: absent = derived, see `series`; servo_axis: stroke_mm, home_at, addr_grows, home_mm, lead_mm, carriage_len_mm, carriage_h_mm, body_h_mm, body_w_mm, body_len_mm; conveyor: length_mm, width_mm, height_mm, motor_at; box kinds: x_mm, y_mm, z_mm), basis ({param|pose|shape: user|label|measured|ref|guess}, at most 32), model (text, at most 40), and for kind `solid` shape (1 to 24 items {t:box,size,pos,rot} or {t:cyl,d,len,d2,axis,pos,rot}) and frames ({name:{pos, rot:[rx,ry,rz], type:surface|axial}}, at most 16). pose.rot has three angles in schema 2. Device keys `skin` (the look: groups of box/cyl items bound to a frame of the device, with show/hide by a bit address of the scene and a tone from `tones`), `skin_source` (the server sets it: a model's skin is always `model`; only the person's page sets `checked`) and `indicators` (rows of lamps that repeat bits of other devices); the rules are in `guide.skin_rules`, the work order in `guide.order`. Scene key `refs` (at most 16 {a:\"<id>.<frame>\", b:\"<id>.<frame>\", mm, basis: measured|label}): a length that a person measured; the reply gives `scene_mm` for each. The reply of a schema 2 draft has `facts` (where each device stands in the world, its axis, `box_mm`, `dims` with their source, the axis tips), `ask` (what to put to the person), `notes`, `not_simulated` and `limits`; read them, do not trust your own arithmetic. See `geometry_note` and `frames`.",
            "name": format!("optional text, at most {MAX_LABEL} characters"),
            "photos": format!("optional array of at most {MAX_PHOTOS}: {{file, camera:{{pos:[x,y,z], target:[x,y,z], fov}} | null}}"),
            "devices": host.devices,
            "parts_and_links": "device `parts`:{part_name:{pos:[x,y,z], rot:[x,y,z]}} (part names from catalog.layout[kind].parts); scene `links` (at most 400):[{from:\"<device>.<port>\", to:\"<device>.<port>\", kind:\"air\"|\"wire\"}] (ports from catalog.layout[kind].ports; both ends must have the link kind). See catalog.layout_note.",
            "limits": {"max_bytes": MAX_SCENE_BYTES, "max_devices": MAX_DEVICES, "no_two_output_slots_on_one_address": true},
            "simulated_false": "kinds with simulated:false are drawn and mapped, but the plant does not move them yet"
        },
        "note": host.note
    })
}

/// 장면의 코일(출력) 주소 목록(감시 기본값에 쓴다).
pub fn out_addrs(s: &Scene) -> Vec<String> {
    let set: BTreeSet<&str> = s.devices.iter().flat_map(|d| d.slots.iter()).filter(|x| x.out && !x.word && x.bound).map(|x| x.addr.as_str()).collect();
    set.into_iter().map(String::from).collect()
}
