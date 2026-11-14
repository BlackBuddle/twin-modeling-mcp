//! 장면 저장소(작업 1.2): `<출력 폴더>\twin\<장면>\`의 scene.json, scene.draft.json, history, 사진, 표시, 카메라.
//! 모든 파일은 `.tmp`에 쓴 뒤 이름을 바꾼다. 오류 글과 결과에는 경로를 싣지 않는다. PLC에는 닿지 않는다.

use crate::stamp::utc_stamp;
use serde_json::{json, Map, Value};
use twin_engine::bind::Addresses;
use twin_engine::scene;
use twin_engine::scene::MAX_STORED;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

pub const MAX_JPEG: usize = 1024 * 1024;
pub const MAX_JSON: usize = 256 * 1024;
/// 장면 수의 상한(새 장면을 만드는 길 모두).
pub const MAX_SCENES: usize = 32;
const MAX_HISTORY: usize = 20;
const MAX_MARKS: usize = 500;
const MAX_LABEL: usize = 40;

pub type Res<T> = Result<T, (u16, String)>;

/// 장면을 읽고 검사한다. 주소가 없는 호스트(트윈 모델 서버)에서는 트윈 모델이 두지 않는 키를 `model::check`가 먼저 고정된 글로 거절한다.
fn parse(name: &str, v: &Value, addrs: &'static dyn Addresses) -> Result<scene::Scene, String> {
    if addrs.virtual_pins() {
        twin_engine::model::check(v)?;
    }
    scene::parse_with(name, v, addrs)
}

fn bad<T>(m: impl Into<String>) -> Res<T> {
    Err((400, m.into()))
}

fn io_err<T>(_: std::io::Error) -> Res<T> {
    Err((500, "the file could not be written".into()))
}

pub struct Store {
    dir: PathBuf,
    /// 호스트의 주소 규칙: 장면 검사가 쓴다(plc-mcp는 PLC 주소, 다른 호스트는 자기 규칙).
    addrs: &'static dyn Addresses,
    /// 자동 적용 토글. 시작할 때 꺼져 있고 화면 API만 바꾼다(`set_auto_apply`는 http.rs에서만 부른다).
    auto: AtomicBool,
    /// 장면 이름 → (scene.json의 도장, 장치 수). 목록을 읽을 때마다 파일을 다시 읽지 않는다 (B8 BR9).
    counts: Mutex<BTreeMap<String, (u64, usize)>>,
}

static SHARED: OnceLock<Arc<Store>> = OnceLock::new();

/// 호스트가 시작할 때 한 번 부른다: 출력 폴더와 주소 규칙을 주면 서버 공용 저장소가 선다(먼저 부른 쪽이 이긴다).
/// 옛 코드는 `shared()`가 `outdir::output_dir()`를 읽어 늦게 세웠다. 호스트가 폴더를 정한 바로 뒤에 부르면 같다.
pub fn init_shared(dir: PathBuf, addrs: &'static dyn Addresses) {
    let _ = SHARED.set(Arc::new(Store::new(dir, addrs)));
}

/// 서버 전체가 쓰는 저장소(MCP 도구와 화면 API가 같은 토글을 본다). 출력 폴더가 없으면(`init_shared`를 아직 안 불렀으면) None.
pub fn shared() -> Option<Arc<Store>> {
    SHARED.get().cloned()
}

/// 이 출력 폴더의 저장소: 서버 공용 저장소가 같은 폴더면 그것(자동 적용 토글이 같다), 아니면 새로 만든다.
pub fn for_dir(dir: &Path, addrs: &'static dyn Addresses) -> Arc<Store> {
    shared().filter(|s| s.dir == dir).unwrap_or_else(|| Arc::new(Store::new(dir.to_path_buf(), addrs)))
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Res<()> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).or_else(io_err)?;
    }
    let tmp = path.with_extension(format!("{}.tmp", path.extension().and_then(|e| e.to_str()).unwrap_or("")));
    std::fs::write(&tmp, bytes).or_else(io_err)?;
    std::fs::rename(&tmp, path).or_else(|e| {
        let _ = std::fs::remove_file(&tmp);
        io_err(e)
    })
}

fn read_json(path: &Path) -> Res<Option<Value>> {
    match std::fs::metadata(path) {
        Err(_) => Ok(None),
        Ok(m) if m.len() > MAX_STORED as u64 => bad(format!("a stored file is larger than {MAX_STORED} bytes")),
        Ok(_) => {
            let t = std::fs::read(path).or_else(io_err)?;
            serde_json::from_slice(&t).map(Some).or_else(|_| bad("a stored file is not valid JSON"))
        }
    }
}

/// 저장할 JSON 글(읽기 좋은 모양)을 만들고, `MAX_STORED`를 넘으면 쓰기 전에 거절한다. 그러면 저장한 파일은 늘 다시 읽힌다.
pub fn stored_bytes(v: &Value, what: &str) -> Res<Vec<u8>> {
    let bytes = serde_json::to_vec_pretty(v).or_else(|_| bad(format!("{what} cannot be written")))?;
    if bytes.len() > MAX_STORED {
        return bad(format!("the {what} is larger than {MAX_STORED} bytes when it is stored ({} bytes); make it smaller", bytes.len()));
    }
    Ok(bytes)
}

/// 읽을 수 없는 저장 파일(너무 크거나 JSON이 아님)은 "없음"으로 본다: 새로 쓰는 길(초안, 고친 장면, 확정)이 막히지 않게 한다.
/// 옛 파일은 새 파일로 바뀐다(역사 사본에는 남지 않는다).
fn read_or_none(path: &Path) -> Option<Value> {
    read_json(path).ok().flatten()
}

/// 사진 번호 `n`의 JPEG 시작 바이트와 크기를 본다.
pub fn check_jpeg(b: &[u8]) -> Res<()> {
    if b.len() > MAX_JPEG {
        return Err((413, format!("JPEG is larger than {MAX_JPEG} bytes")));
    }
    if !b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return bad("body is not a JPEG (must start with FF D8 FF)");
    }
    Ok(())
}

/// 표시(marks)를 검사하고 정규화한다. 배열이나 `{"marks":[...]}`를 받는다.
fn check_marks(v: &Value) -> Res<Value> {
    let a = v.get("marks").unwrap_or(v).as_array().ok_or((400, "marks must be an array".to_string()))?;
    if a.len() > MAX_MARKS {
        return bad(format!("at most {MAX_MARKS} marks"));
    }
    for (i, m) in a.iter().enumerate() {
        let o = m.as_object().ok_or((400, format!("marks[{i}] must be an object")))?;
        for (k, x) in o {
            match k.as_str() {
                "type" => {
                    if !x.as_str().is_some_and(|t| (1..=16).contains(&t.len()) && t.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')) {
                        return bad(format!("marks[{i}].type must be a short word"));
                    }
                }
                "label" => {
                    if !x.as_str().is_some_and(|t| t.chars().count() <= MAX_LABEL && !t.chars().any(char::is_control)) {
                        return bad(format!("marks[{i}].label must be text of at most {MAX_LABEL} characters"));
                    }
                }
                "x" | "y" | "w" | "h" | "x2" | "y2" => {
                    if !x.as_f64().is_some_and(|n| (0.0..=1.0).contains(&n)) {
                        return bad(format!("marks[{i}].{k} must be a number from 0 to 1"));
                    }
                }
                _ => return bad(format!("marks[{i}].{k}: unknown field")),
            }
        }
        if !o.contains_key("type") {
            return bad(format!("marks[{i}].type is needed"));
        }
    }
    Ok(json!(a))
}

fn check_camera(c: &Value) -> Res<()> {
    let t = |v: &Value| v.as_array().is_some_and(|a| a.len() == 3 && a.iter().all(|x| x.as_f64().is_some_and(|n| n.is_finite() && n.abs() <= 100_000.0)));
    let o = c.as_object().filter(|o| o.keys().all(|k| matches!(k.as_str(), "pos" | "target" | "fov")));
    if o.is_some() && t(&c["pos"]) && t(&c["target"]) && c["fov"].as_f64().is_some_and(|f| (1.0..=179.0).contains(&f)) {
        Ok(())
    } else {
        bad("camera needs pos, target (3 numbers) and fov (1 to 179)")
    }
}

/// 두 장면 JSON의 장치 차이: 늘어난 것, 사라진 것, 바뀐 것(id와 바뀐 필드 이름).
/// 배치 경고와 연결(기본 연결 포함)을 답에 붙인다. 장면이 없거나 검사에 실패하면 붙이지 않는다.
fn with_report(mut r: Value, name: &str, scene: Option<&Value>, wkey: &str, lkey: &str, addrs: &'static dyn Addresses) -> Value {
    if let Some(rep) = scene.and_then(|v| twin_engine::layout::report_with(name, v, addrs)) {
        r[wkey] = rep["warnings"].clone();
        r[lkey] = rep["links"].clone();
    }
    // 형식 2: 기하 사실(facts). 형식 1의 겹침·뜸 규칙은 형식 2 장치에 맞지 않으므로 long_link만 남긴다(마일스톤 A: 형식 2 경고는 뒤 작업).
    if let Some(s) = scene.and_then(|v| scene::parse_with(name, v, addrs).ok()).filter(|s| s.schema == 2) {
        // 키 이름: 장면은 `facts`, `ask`..., 초안은 `draft_facts`, `draft_ask`...
        let prefix = wkey.strip_suffix("warnings").unwrap_or("");
        let mut facts = twin_engine::geom::facts(&s);
        facts["skin_counts"] = skin_counts(&s);
        r[format!("{prefix}facts")] = facts;
        if let Some(e) = twin_engine::geom::extras(&s).as_object() {
            for (k, v) in e {
                r[format!("{prefix}{k}")] = v.clone();
            }
        }
        // 어두운 표시등 주소의 note(장면의 어느 장치도 갖지 않은 주소)
        if let Some(n) = r[format!("{prefix}notes")].as_array_mut() {
            n.extend(s.skins.iter().filter(|k| !k.dark.is_empty()).map(|k| json!(dark_note(k))));
            // 주소가 묶이지 않은 핀을 `show`나 `hide`가 가리키면 그 그룹은 핀이 꺼진 것처럼 그려진다
            n.extend(s.skins.iter().filter(|k| !k.unbound.is_empty()).map(|k| json!(unbound_note(k))));
        }
        if let Some(w) = r[wkey].as_array_mut() {
            w.retain(|x| x["kind"] == "long_link");
        }
    }
    r
}

/// `facts.skin_counts`: 외형이 있는 장치 수, 외형 항목 수, 표시등 수, 사람이 아직 확인하지 않은(`model`) 외형의 장치 수. 숫자만 싣는다.
fn skin_counts(s: &scene::Scene) -> Value {
    let skinned: Vec<&scene::SkinInfo> = s.skins.iter().filter(|k| k.has_skin).collect();
    json!({
        "devices": skinned.len(),
        "items": skinned.iter().map(|k| k.items).sum::<usize>(),
        "lamps": s.skins.iter().map(|k| k.lamps).sum::<usize>(),
        "unchecked": skinned.iter().filter(|k| !k.checked).count(),
    })
}

/// `<id>: indicators: X1D, X1E and 26 more are not addresses of the scene (drawn dark)`(주소는 16개까지 적고 나머지는 개수)
fn dark_note(k: &scene::SkinInfo) -> String {
    let shown = k.dark.iter().take(16).cloned().collect::<Vec<_>>().join(", ");
    let more = if k.dark.len() > 16 { format!(" and {} more", k.dark.len() - 16) } else { String::new() };
    format!("{}: indicators: {shown}{more} are not addresses of the scene (drawn dark)", k.id)
}

/// `<id>: skin: cyl.coil_back has no PLC address yet; its group is drawn as if the pin is OFF (bind it with io or signals[].device)`
fn unbound_note(k: &scene::SkinInfo) -> String {
    let shown = k.unbound.iter().take(16).cloned().collect::<Vec<_>>().join(", ");
    let more = if k.unbound.len() > 16 { format!(" and {} more", k.unbound.len() - 16) } else { String::new() };
    format!("{}: skin: {shown}{more} has no PLC address yet; its group is drawn as if the pin is OFF (bind it with io or signals[].device)", k.id)
}

/// 화면이 쓰는 장면 글(`GET /api/twin/scene`)에서 핀 형식을 주소로 푼다(화면은 `show`·`hide`·`from`을 주소로 읽는다: 화면 글을 바꾸지 않는다).
/// 묶인 핀의 `show`·`hide`는 그 주소로, 묶인 핀 목록 `pins`가 한 줄로 이어진 주소이면 `from`과 `count`로 바뀐다.
/// 묶이지 않았거나 이어지지 않은 핀은 그대로 둔다(화면은 주소가 아닌 글을 꺼짐으로 읽고, 램프는 그리지 않는다).
/// 화면이 이 글을 초안으로 되돌려 보내면 묶인 핀이 주소 형식으로 저장된다(같은 뜻이다).
fn pins_to_addrs(v: &mut Value, map: &BTreeMap<String, String>, addrs: &dyn Addresses) {
    let Some(devs) = v.get_mut("devices").and_then(Value::as_array_mut) else { return };
    for d in devs {
        for g in d.get_mut("skin").and_then(|s| s.get_mut("groups")).and_then(Value::as_array_mut).into_iter().flatten() {
            for k in ["show", "hide"] {
                if let Some(a) = g.get(k).and_then(Value::as_str).and_then(|p| map.get(p)).cloned() {
                    g[k] = json!(a);
                }
            }
        }
        for r in d.get_mut("indicators").and_then(Value::as_array_mut).into_iter().flatten() {
            let Some(pins) = r.get("pins").and_then(Value::as_array) else { continue };
            let Some(a) = pins.iter().map(|p| p.as_str().and_then(|p| map.get(p))).collect::<Option<Vec<_>>>().filter(|a| !a.is_empty()) else { continue };
            if let Some(Ok(run)) = addrs.bit_run(a[0], a.len() as u32) {
                if run.iter().zip(&a).all(|(x, y)| x == *y) {
                    let (from, count) = (a[0].clone(), a.len());
                    if let Some(o) = r.as_object_mut() {
                        o.remove("pins");
                        o.insert("from".into(), json!(from));
                        o.insert("count".into(), json!(count));
                    }
                }
            }
        }
    }
}

/// 외형의 표시(`skin_source`)를 서버가 정한다. 장면을 쓸 때마다 외형이 있는 장치는 `model`이 되고, 예외는 하나이다:
/// 지금 `scene.json`에 같은 id의 장치가 있고 그 외형이 같으며 표시가 `checked`이면 `checked`가 남는다.
/// 그래서 모델이 자기가 쓴 외형에 `checked`를 줄 수 없고, 한 장치만 바꾼 초안이 다른 장치의 표시를 지우지 않는다.
/// `checked`를 실어 온 새 외형이나 바뀐 외형은 note로 알린다.
fn mark_skins(new: &mut Value, old: Option<&Value>) -> Vec<String> {
    let mut notes = Vec::new();
    let Some(devs) = new["devices"].as_array_mut() else { return notes };
    for d in devs.iter_mut().filter(|d| d["skin"].is_object()) {
        let id = d["id"].as_str().unwrap_or("").to_string();
        let kept = old.and_then(|o| o["devices"].as_array()).and_then(|a| a.iter().find(|x| x["id"] == d["id"])).is_some_and(|x| x["skin"] == d["skin"] && x["skin_source"] == "checked");
        if d["skin_source"] == "checked" && !kept {
            notes.push(format!("{id}: skin_source: saved as model (only the page sets checked)"));
        }
        d["skin_source"] = json!(if kept { "checked" } else { "model" });
    }
    notes
}

pub fn diff(old: Option<&Value>, new: &Value) -> Value {
    let map = |v: Option<&Value>| -> Vec<(String, Value)> { v.and_then(|v| v["devices"].as_array()).map(|a| a.iter().filter_map(|d| Some((d["id"].as_str()?.to_string(), d.clone()))).collect()).unwrap_or_default() };
    let (o, n) = (map(old), map(Some(new)));
    let added: Vec<&str> = n.iter().filter(|(i, _)| !o.iter().any(|(j, _)| i == j)).map(|(i, _)| i.as_str()).collect();
    let removed: Vec<&str> = o.iter().filter(|(i, _)| !n.iter().any(|(j, _)| i == j)).map(|(i, _)| i.as_str()).collect();
    let mut changed = Vec::new();
    for (id, nd) in &n {
        if let Some((_, od)) = o.iter().find(|(j, _)| j == id) {
            let keys: BTreeSet<&String> = od.as_object().into_iter().chain(nd.as_object()).flat_map(Map::keys).collect();
            let f: Vec<&String> = keys.into_iter().filter(|k| od.get(k.as_str()) != nd.get(k.as_str())).collect();
            if !f.is_empty() {
                changed.push(json!({"id": id, "fields": f}));
            }
        }
    }
    let mut r = json!({"added": added, "removed": removed, "changed": changed});
    // 사진 카메라가 달라진 사진 파일 이름
    let cams = |v: Option<&Value>| -> Vec<(String, Value)> { v.and_then(|v| v["photos"].as_array()).map(|a| a.iter().filter_map(|p| Some((p["file"].as_str()?.to_string(), p["camera"].clone()))).collect()).unwrap_or_default() };
    let (oc, nc) = (cams(old), cams(Some(new)));
    let cameras: Vec<&str> = nc.iter().filter(|(f, c)| !c.is_null() && oc.iter().find(|(g, _)| g == f).is_none_or(|(_, o)| o != c)).map(|(f, _)| f.as_str()).collect();
    if !cameras.is_empty() {
        r["cameras"] = json!(cameras);
    }
    r
}

impl Store {
    pub fn new(dir: PathBuf, addrs: &'static dyn Addresses) -> Store {
        Store { dir, addrs, auto: AtomicBool::new(false), counts: Mutex::new(BTreeMap::new()) }
    }

    pub fn auto_apply(&self) -> bool {
        self.auto.load(Ordering::SeqCst)
    }

    /// 화면 API만 부른다(시험이 소스를 읽어 확인한다).
    pub fn set_auto_apply(&self, on: bool) {
        self.auto.store(on, Ordering::SeqCst);
    }

    fn root(&self) -> PathBuf {
        self.dir.join("twin")
    }

    /// 장면 폴더(이름만 검사한다. 만들지 않는다).
    fn sdir(&self, name: &str) -> Res<PathBuf> {
        if !scene::valid_scene_name(name) {
            return bad("scene name must be 1 to 32 characters of A-Z a-z 0-9 - _ (and not a Windows device name)");
        }
        Ok(self.root().join(name))
    }

    /// 새 장면 폴더를 만들기 전에: 장면이 `MAX_SCENES`개이면 거절한다(이미 있는 폴더는 세지 않는다).
    fn room_for(&self, d: &Path) -> Res<()> {
        if d.exists() {
            return Ok(());
        }
        let n = std::fs::read_dir(self.root()).map_or(0, |r| r.flatten().filter(|e| e.path().is_dir()).count());
        if n >= MAX_SCENES {
            return bad(format!("at most {MAX_SCENES} scenes"));
        }
        Ok(())
    }

    /// 있는 장면의 폴더. 없으면 404.
    fn existing(&self, name: &str) -> Res<PathBuf> {
        let d = self.sdir(name)?;
        if d.join("scene.json").is_file() || d.join("scene.draft.json").is_file() {
            Ok(d)
        } else {
            Err((404, format!("scene {name} not found")))
        }
    }

    fn history_copy(&self, d: &Path, bytes: &[u8]) -> Res<()> {
        let h = d.join("history");
        let mut names: Vec<String> = std::fs::read_dir(&h).into_iter().flatten().flatten().filter_map(|e| e.file_name().into_string().ok()).filter(|n| n.ends_with(".json")).collect();
        names.sort();
        // 같은 초(또는 이미 앞선 이름)가 있으면 다음 초의 이름을 쓴다. 가장 새 사본이 지워지지 않게 늘 가장 큰 이름보다 크다.
        let mut secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |t| t.as_secs());
        let newest = names.last().cloned().unwrap_or_default();
        while format!("{}.json", utc_stamp(secs)) <= newest {
            secs += 1;
        }
        let new = format!("{}.json", utc_stamp(secs));
        write_atomic(&h.join(&new), bytes)?;
        names.push(new);
        while names.len() > MAX_HISTORY {
            let _ = std::fs::remove_file(h.join(names.remove(0)));
        }
        Ok(())
    }

    fn put_scene(&self, d: &Path, v: &Value) -> Res<()> {
        let bytes = stored_bytes(v, "scene")?;
        write_atomic(&d.join("scene.json"), &bytes)?;
        // 장면에 적힌 사진 카메라를 사진 옆 파일로 옮긴다(화면과 twin_scene_get은 그 파일만 읽는다). 없는 사진은 건너뛴다.
        for ph in v["photos"].as_array().into_iter().flatten() {
            let n = ph["file"].as_str().and_then(|f| f.strip_suffix(".jpg")).and_then(|n| n.parse::<u32>().ok());
            if let (Some(n), Some(c)) = (n, ph.get("camera").filter(|c| c.is_object())) {
                if d.join("photos").join(format!("{n}.jpg")).is_file() {
                    let cam = json!({"pos": c["pos"], "target": c["target"], "fov": c["fov"]});
                    write_atomic(&d.join("photos").join(format!("{n}.camera.json")), cam.to_string().as_bytes())?;
                }
            }
        }
        self.history_copy(d, &bytes)
    }

    /// 모델의 초안. 자동 적용이 꺼져 있으면 scene.json은 그대로다.
    pub fn draft(&self, name: &str, data: &Value) -> Res<Value> {
        parse(name, data, self.addrs).map_err(|e| (400, e))?;
        let d = self.sdir(name)?;
        self.room_for(&d)?;
        // 외형의 표시는 서버가 정한다: 지금 장면과 비교한다
        let mut data = data.clone();
        let notes = mark_skins(&mut data, read_or_none(&d.join("scene.json")).as_ref());
        let data = &data;
        let head = if self.auto_apply() {
            self.put_scene(&d, data)?;
            let _ = std::fs::remove_file(d.join("scene.draft.json"));
            json!({"saved": "scene", "applied": true})
        } else {
            write_atomic(&d.join("scene.draft.json"), &stored_bytes(data, "draft")?)?;
            json!({"saved": "draft", "applied": false})
        };
        let mut r = with_report(head, name, Some(data), "warnings", "links", self.addrs);
        if let Some(n) = r["notes"].as_array_mut() {
            n.extend(notes.into_iter().map(Value::from));
        }
        Ok(r)
    }

    /// 고칠 바탕: 초안이 있으면 초안(사람이 고친 것을 지키려고), 없으면 scene.json.
    pub fn base_json(&self, name: &str) -> Res<Value> {
        let d = self.existing(name)?;
        read_json(&d.join("scene.draft.json"))?.or(read_json(&d.join("scene.json"))?).ok_or((404, format!("scene {name} not found")))
    }

    /// 고칠 바탕과 그 종류(`"draft"` 또는 `"scene"`): 시뮬레이션이 어느 쪽을 돌렸는지 알리는 데 쓴다.
    pub fn base_kind(&self, name: &str) -> Res<(Value, &'static str)> {
        let d = self.existing(name)?;
        match read_json(&d.join("scene.draft.json"))? {
            Some(v) => Ok((v, "draft")),
            None => read_json(&d.join("scene.json"))?.map(|v| (v, "scene")).ok_or((404, format!("scene {name} not found"))),
        }
    }

    /// 원리표 글의 한도(64 KiB)와 `sheet.md`: 모델 옆에 글로 둔다. 줄바꿈과 탭 말고 제어 문자는 거절한다.
    pub const MAX_SHEET: usize = 64 * 1024;

    /// 글의 검사(한도와 글자). 모델을 저장하기 전에 부를 수 있다: 거절된 글이 모델만 저장한 채 오류를 내지 않게 한다.
    pub fn sheet_check(text: &str) -> Res<()> {
        if text.len() > Self::MAX_SHEET {
            return bad(format!("sheet is larger than {} bytes", Self::MAX_SHEET));
        }
        // 모델 글의 규칙: 서식·방향 문자와 공백처럼 보이는 글자도 거절한다(화면이 읽는 글이 저장한 글과 달라 보이지 않게)
        if text.chars().any(|c| twin_engine::text::refused_char(c) && !matches!(c, '\n' | '\r' | '\t')) {
            return bad("sheet has control characters");
        }
        Ok(())
    }

    pub fn sheet_set(&self, name: &str, text: &str) -> Res<()> {
        Self::sheet_check(text)?;
        write_atomic(&self.existing(name)?.join("sheet.md"), text.as_bytes())
    }

    /// 저장한 원리표 글(없으면 None)
    pub fn sheet_get(&self, name: &str) -> Res<Option<String>> {
        let p = self.sdir(name)?.join("sheet.md");
        match std::fs::metadata(&p) {
            Err(_) => Ok(None),
            Ok(m) if m.len() > Self::MAX_SHEET as u64 => bad("the stored sheet is too large"),
            Ok(_) => std::fs::read_to_string(&p).map(Some).or_else(|_| bad("the stored sheet is not text")),
        }
    }

    pub fn confirm(&self, name: &str) -> Res<Value> {
        let d = self.sdir(name)?;
        let mut v = read_json(&d.join("scene.draft.json"))?.ok_or((404, format!("scene {name} has no draft")))?;
        parse(name, &v, self.addrs).map_err(|e| (400, e))?;
        // 초안을 만든 뒤 장면이 바뀌었을 수 있으니 표시를 다시 정한다(같은 규칙)
        mark_skins(&mut v, read_or_none(&d.join("scene.json")).as_ref());
        self.put_scene(&d, &v)?;
        let _ = std::fs::remove_file(d.join("scene.draft.json"));
        Ok(json!({"ok": true}))
    }

    /// 초안 버리기(화면의 "초안 버리기"). scene.json은 그대로다.
    pub fn discard(&self, name: &str) -> Res<Value> {
        match std::fs::remove_file(self.sdir(name)?.join("scene.draft.json")) {
            Ok(()) => Ok(json!({"ok": true})),
            Err(_) => Err((404, format!("scene {name} has no draft"))),
        }
    }

    /// 사람이 화면에서 직접 고친 장면.
    pub fn put(&self, name: &str, data: &Value) -> Res<Value> {
        parse(name, data, self.addrs).map_err(|e| (400, e))?;
        let d = self.existing(name)?;
        let mut data = data.clone();
        mark_skins(&mut data, read_or_none(&d.join("scene.json")).as_ref());
        self.put_scene(&d, &data)?;
        Ok(json!({"ok": true}))
    }

    /// 화면의 "외형 확인"(`POST /api/twin/skin/check`): 장치들의 외형 표시를 `checked`(또는 `model`)로 정한다.
    /// `scene.json`만 고치고(초안이 있으면 409), `.tmp`와 이름 바꾸기, 역사 사본 하나를 만든다. 이 함수를 부르는 곳은 이 파일과 `http.rs`뿐이다.
    pub fn skin_check(&self, name: &str, ids: &[String], checked: bool) -> Res<Value> {
        let d = self.existing(name)?;
        if d.join("scene.draft.json").is_file() {
            return Err((409, "confirm or discard the draft first".into()));
        }
        if ids.len() > 200 {
            return bad("at most 200 devices");
        }
        let mut v = read_json(&d.join("scene.json"))?.ok_or((404, format!("scene {name} not found")))?;
        let devs = v["devices"].as_array_mut().ok_or((400, "scene has no devices".to_string()))?;
        for id in ids {
            let short: String = id.chars().take(32).collect();
            let Some(dev) = devs.iter_mut().find(|x| x["id"] == id.as_str()) else { return bad(format!("device {short}: no such device")) };
            if !dev["skin"].is_object() {
                return bad(format!("device {short}: the device has no skin"));
            }
            dev["skin_source"] = json!(if checked { "checked" } else { "model" });
        }
        parse(name, &v, self.addrs).map_err(|e| (400, e))?;
        self.put_scene(&d, &v)?;
        // 표시를 거둔 장치의 키는 `unchecked`다 (`checked`라고 하면 방금 `model`로 돌린 id를 확인했다는 말로 읽힌다)
        Ok(json!({"scene": name, (if checked { "checked" } else { "unchecked" }): ids}))
    }

    pub fn new_scene(&self, name: &str) -> Res<Value> {
        let d = self.sdir(name)?;
        if d.exists() {
            return Err((409, format!("scene {name} exists")));
        }
        self.room_for(&d)?;
        write_atomic(&d.join("scene.json"), br#"{"devices":[],"schema":1}"#)?;
        Ok(json!({"ok": true}))
    }

    fn photo_numbers(&self, d: &Path) -> Vec<u32> {
        let mut v: Vec<u32> = std::fs::read_dir(d.join("photos")).into_iter().flatten().flatten().filter_map(|e| e.file_name().into_string().ok()).filter_map(|n| n.strip_suffix(".jpg").and_then(|s| s.parse().ok())).collect();
        v.sort_unstable();
        v
    }

    pub fn list(&self) -> Res<Value> {
        let mut out = Vec::new();
        for e in std::fs::read_dir(self.root()).into_iter().flatten().flatten() {
            let Ok(name) = e.file_name().into_string() else { continue };
            if !scene::valid_scene_name(&name) || !e.path().is_dir() {
                continue;
            }
            let d = e.path();
            // 변경 도장: 파일의 수정 시각(ms). 파일이 없으면 null. 사람이 손으로 복사한 파일도 바뀐 것으로 보인다 (화면이 새로고침 없이 따라가게)
            let stamp = |f: &str| std::fs::metadata(d.join(f)).ok().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|t| t.as_millis() as u64);
            // 장치 수는 scene.json의 도장이 그대로면 앞의 값을 쓴다 (2초마다 오는 읽기가 파일을 다시 열지 않게)
            let count = match stamp("scene.json") {
                Some(ms) => {
                    let hit = self.counts.lock().ok().and_then(|c| c.get(&name).filter(|x| x.0 == ms).map(|x| x.1));
                    hit.unwrap_or_else(|| {
                        let n = read_json(&d.join("scene.json")).ok().flatten().and_then(|v| v["devices"].as_array().map(Vec::len)).unwrap_or(0);
                        if let Ok(mut c) = self.counts.lock() {
                            c.insert(name.clone(), (ms, n));
                        }
                        n
                    })
                }
                None => 0,
            };
            out.push(json!({"name": name, "devices": count, "photos": self.photo_numbers(&d).len(), "has_draft": d.join("scene.draft.json").is_file(), "scene_ms": stamp("scene.json"), "draft_ms": stamp("scene.draft.json")}));
        }
        out.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
        Ok(json!({"scenes": out}))
    }

    pub fn get(&self, name: &str) -> Res<Value> {
        self.get_as(name, false)
    }

    /// 화면 API의 읽기(`GET /api/twin/scene`): `get`과 같고, 주소를 가진 호스트(plc-mcp)에서는 핀 형식을 화면이 읽는 주소 형식으로 푼다.
    pub fn get_page(&self, name: &str) -> Res<Value> {
        self.get_as(name, true)
    }

    fn get_as(&self, name: &str, page: bool) -> Res<Value> {
        let d = self.existing(name)?;
        // 읽을 수 없는 파일은 오류로 막지 않고 `unreadable`에 까닭을 싣는다: 화면이 버리거나 바꿀 길을 보여 줄 수 있게 한다 (FR2)
        let mut unreadable = serde_json::Map::new();
        let mut read = |file: &str, key: &str| match read_json(&d.join(file)) {
            Ok(v) => v,
            Err((_, m)) => {
                unreadable.insert(key.to_string(), json!(m));
                None
            }
        };
        let scene = read("scene.json", "scene");
        let draft = read("scene.draft.json", "draft");
        let diff = draft.as_ref().map(|dr| diff(scene.as_ref(), dr));
        let photos: Vec<Value> = self.photo_numbers(&d).into_iter().map(|n| json!({"photo": n, "has_marks": d.join("photos").join(format!("{n}.marks.json")).is_file(), "camera": read_json(&d.join("photos").join(format!("{n}.camera.json"))).ok().flatten()})).collect();
        let mut r = json!({"scene": scene, "draft": draft, "diff": diff, "photos": photos});
        if !unreadable.is_empty() {
            r["unreadable"] = Value::Object(unreadable);
        }
        let mut r = with_report(r, name, scene.as_ref(), "warnings", "links", self.addrs);
        // `resolved`는 형식 2 장면과 초안만(형식 1의 답에는 이 키가 없다)
        // 형식 2 파일이 검사에 실패하면 `resolved` 없이 `scene_error`/`draft_error`에 검사 글을 싣는다: 화면이 형식 1 코드로 그리지 못하게 한다.
        // 형식 1 파일의 검사 실패는 오늘처럼 아무 키도 없다(화면이 형식 1 데이터를 그대로 그린다).
        for (v, key, ekey, vkey) in [(&scene, "resolved", "scene_error", "scene"), (&draft, "draft_resolved", "draft_error", "draft")] {
            let Some(v) = v.as_ref().filter(|v| v["schema"].as_i64() == Some(2)) else { continue };
            match parse(name, v, self.addrs) {
                Ok(s) => {
                    r[key] = twin_engine::geom::resolved(&s);
                    if page && !self.addrs.virtual_pins() {
                        pins_to_addrs(&mut r[vkey], &scene::pin_addresses(&s), self.addrs);
                    }
                }
                Err(e) => r[ekey] = json!(e),
            }
        }
        Ok(with_report(r, name, draft.as_ref(), "draft_warnings", "draft_links", self.addrs))
    }

    pub fn photo_add(&self, name: &str, jpeg: &[u8]) -> Res<Value> {
        check_jpeg(jpeg)?;
        let d = self.existing(name)?;
        let nums = self.photo_numbers(&d);
        if nums.len() >= scene::MAX_PHOTOS {
            return bad(format!("at most {} photos", scene::MAX_PHOTOS));
        }
        let n = nums.last().map_or(1, |m| m + 1);
        write_atomic(&d.join("photos").join(format!("{n}.jpg")), jpeg)?;
        Ok(json!({"photo": n}))
    }

    fn photo_dir(&self, name: &str, n: u32) -> Res<PathBuf> {
        let d = self.existing(name)?;
        if !self.photo_numbers(&d).contains(&n) {
            return Err((404, format!("photo {n} not found")));
        }
        Ok(d.join("photos"))
    }

    pub fn photo_get(&self, name: &str, n: u32, marked: bool) -> Res<Vec<u8>> {
        let p = self.photo_dir(name, n)?;
        let f = p.join(if marked { format!("{n}.marked.jpg") } else { format!("{n}.jpg") });
        std::fs::read(f).map_err(|_| (404, "no such image".into()))
    }

    pub fn marks_set(&self, name: &str, n: u32, v: &Value) -> Res<Value> {
        let m = check_marks(v)?;
        let p = self.photo_dir(name, n)?;
        write_atomic(&p.join(format!("{n}.marks.json")), m.to_string().as_bytes())?;
        Ok(json!({"ok": true}))
    }

    pub fn marked_set(&self, name: &str, n: u32, jpeg: &[u8]) -> Res<Value> {
        check_jpeg(jpeg)?;
        let p = self.photo_dir(name, n)?;
        write_atomic(&p.join(format!("{n}.marked.jpg")), jpeg)?;
        Ok(json!({"ok": true}))
    }

    /// 모델에게 줄 사진 정보: (표시 목록, 카메라 또는 null, 그려진 JPEG이면 true, JPEG 바이트). 경로는 싣지 않는다.
    pub fn photo_for_ai(&self, name: &str, n: u32) -> Res<(Value, Value, bool, Vec<u8>)> {
        let p = self.photo_dir(name, n)?;
        let marks = read_json(&p.join(format!("{n}.marks.json")))?.unwrap_or_else(|| json!([]));
        let camera = read_json(&p.join(format!("{n}.camera.json")))?.unwrap_or(Value::Null);
        let (marked, jpeg) = match std::fs::read(p.join(format!("{n}.marked.jpg"))) {
            Ok(b) => (true, b),
            Err(_) => (false, std::fs::read(p.join(format!("{n}.jpg"))).map_err(|_| (404, "no such image".to_string()))?),
        };
        Ok((marks, camera, marked, jpeg))
    }

    pub fn camera_set(&self, name: &str, n: u32, cam: &Value) -> Res<Value> {
        check_camera(cam)?;
        let p = self.photo_dir(name, n)?;
        write_atomic(&p.join(format!("{n}.camera.json")), cam.to_string().as_bytes())?;
        Ok(json!({"ok": true}))
    }
}
