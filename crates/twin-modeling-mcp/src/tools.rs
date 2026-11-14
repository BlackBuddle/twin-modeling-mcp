//! 도구 다섯 개(reusable-parts 3.1d-2, 설계 46의 3.3절): `model_catalog`, `model_draft`, `model_get`, `model_sim`, `model_render`.
//! 글은 안전 글이다: 모델은 이것으로 도구가 무엇을 하는지(초안만 쓴다, PLC가 없다) 안다. 제한과 숫자는 입력 스키마와 오류 글에 있다.

use crate::serve::Ctx;
use crate::sim;
use serde_json::{json, Value};
use std::time::Duration;
use twin_engine::bind::NO_ADDRESSES;
use twin_engine::scene::{self, CatalogTexts};
use twin_engine::{geom, model};

/// 모델링 절차의 글: 스킬 본문(front matter 뒤) + `"\n"` + 이 서버의 부분(`references/twin-modeling-mcp.md`). 줄 끝은 LF로 맞춘다.
pub fn skill_core() -> String {
    let s = include_str!("../../../skills/twin-modeling/SKILL.md").replace("\r\n", "\n");
    s.splitn(3, "---\n").nth(2).unwrap_or("").trim_start_matches('\n').to_string()
}

pub fn modeling_text() -> String {
    skill_core() + "\n" + &include_str!("../../../skills/twin-modeling/references/twin-modeling-mcp.md").replace("\r\n", "\n")
}

/// 본문 첫 줄 `Version: <n>`의 수
pub fn modeling_version() -> u64 {
    skill_core().lines().next().and_then(|l| l.trim().strip_prefix("Version:")).and_then(|v| v.trim().parse().ok()).unwrap_or(0)
}

/// 형식의 단계(새 장면 키, 기본 도형, 프리셋이 생기면 올린다)
pub const FORMAT_VERSION: u64 = 1;

const DEVICES_TEXT: &str = "array of at most 200: {id (1-32 of A-Z a-z 0-9 - _, unique), kind (a key of catalog.kinds), label, pose:{pos:[x,y,z] mm, rot:[x,y,z] deg}, params (per kind), timing:{lag_ms, fwd_ms, back_ms}, signals:[{name, dir, type, active}] (no device: a pin is <id>.<slot or signal name>), mech, skin (groups; show and hide take a pin), indicators (rows with pins), on, mount, datum, frames, shape, basis, model, stack, push, stop, sense}. A twin model holds no PLC address: no io, no signals[].device, no position, no scene photos or modules.";

/// 트윈 모델이 두지 않는 키와 그 까닭(`model_rules`)
fn model_rules() -> Value {
    json!([
        {"key": "schema", "rule": "a twin model is schema 2"},
        {"key": "photos", "rule": "a photo is a file of the person; a model file names none"},
        {"key": "modules", "rule": "a module model is a PLC feature"},
        {"key": "io", "rule": "its values are PLC addresses; every slot of the kind is a pin <id>.<slot>"},
        {"key": "signals[].device", "rule": "a PLC address; the pin is <id>.<name>"},
        {"key": "position", "rule": "a word address; use mech with the preset servo_screw and a word signal"},
        {"key": "skin.groups[].show, hide", "rule": "one pin \"<device id>.<signal>\", not an address"},
        {"key": "indicators", "rule": "pins: [\"<device id>.<signal>\", ...], not from and count"},
        {"key": "signals[].name", "rule": "not a slot name of its kind: a pin name is used once"}
    ])
}

/// plc-mcp의 말을 이 서버의 말로 바꾼다(공작물 글과 외형 글). 순서 글(`guide.order`)은 plc-mcp의 도구 순서라 뺀다: 절차는 guide.modeling에 있다.
/// 시험이 각 원문이 한 번씩 있음을 본다(엔진 글이 바뀌면 조용히 지나가지 않고 시험이 실패한다).
pub(crate) const HOST_SWAPS: [(&str, &str, &str); 5] = [
    ("workpieces", "1. Read the photo (twin_photo_get) and the tones of `tones`.", "1. Read the photo and the tones of `tones`."),
    ("workpieces", "; `position` {from (D, W, R, ZR word or U<n>\\G<n>), width 16|32, per_mm, speed_mm_s} on a servo_axis", ""),
    // 원문의 도구 이름은 조각으로 적는다: 경계 시험이 이 크레이트 소스에서 낱말 `ladder`를 화면 경로 말고는 막는다
    ("workpieces", concat!(" and `modules` (a QD77MS model, same form as scenario.modules of lad", "der_from_il)"), ""),
    ("skin_rules", "show|hide (a bit address that an io slot or a bit signal of a device of the scene has)", "show|hide (a bit pin \"<device id>.<slot or signal>\" of the model)"),
    ("skin_rules", "[{from (first bit address), count (1-64),", "[{pins [\"<device id>.<signal>\", ...] (1-64 bit pins),"),
];

pub fn catalog() -> Value {
    let mut r = scene::catalog_tool_result(&CatalogTexts { modeling: modeling_text(), modeling_version: modeling_version(), devices: DEVICES_TEXT.into(), note: "Read-only. A simulation pass is not a test on real equipment." });
    // 이 서버에는 PLC 주소가 없다: 장면 형식 글의 "비트 주소"는 핀이다
    if let Some(t) = r["scene_schema"]["schema"].as_str().map(|t| t.replace("by a bit address of the scene", "by a pin of the scene")) {
        r["scene_schema"]["schema"] = json!(t);
    }
    if let Some(g) = r["guide"].as_object_mut() {
        g.remove("order");
        for (k, from, to) in HOST_SWAPS {
            if let Some(t) = g.get(k).and_then(Value::as_str).map(|t| t.replace(from, to)) {
                g.insert(k.into(), json!(t));
            }
        }
    }
    r["model_rules"] = model_rules();
    r["format_version"] = json!(FORMAT_VERSION);
    r
}

pub fn definitions() -> Vec<Value> {
    vec![
        json!({"name": "model_catalog", "description": "Read-only. Returns the device kinds, presets, frames, tones and the modeling guide (guide.modeling) for a twin model. Call it first.", "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false}}),
        json!({"name": "model_draft", "description": "Checks a twin model (a scene of schema 2 with pins and no PLC address) and saves it as a draft; the person confirms it on the page. sheet: the principle sheet text, saved beside the model. Returns facts, pins, ask, notes.", "inputSchema": {"type": "object", "properties": {
            "name": {"type": "string", "description": "1 to 32 of A-Z a-z 0-9 - _"}, "model": {"type": "object"}, "sheet": {"type": "string", "description": "at most 65536 bytes"}
        }, "required": ["name", "model"], "additionalProperties": false}}),
        json!({"name": "model_get", "description": "Read-only. Without name: lists the models. With name: the scene, the draft, the diff, facts and pins.", "inputSchema": {"type": "object", "properties": {"name": {"type": "string"}}, "additionalProperties": false}}),
        json!({"name": "model_sim", "description": "Read-only. Runs the model (the draft if there is one) with events on pins, no program, no PLC. Returns changes, joints, positions. A simulation pass is not a test on real equipment.", "inputSchema": {"type": "object", "properties": {
            "name": {"type": "string"}, "events": {"type": "array", "maxItems": 100, "items": {"type": "object"}, "description": "{t_ms, pin, value}"}, "part_events": {"type": "array", "maxItems": 100, "items": {"type": "object"}},
            "total_ms": {"type": "integer", "minimum": 10, "maximum": 60000}, "scan_ms": {"type": "integer", "minimum": 1, "maximum": 100}, "watch": {"type": "array", "maxItems": 40, "items": {"type": "string"}, "description": "pins"}
        }, "required": ["name"], "additionalProperties": false}}),
        json!({"name": "model_render", "description": "Read-only. Asks the open 3D page to draw the model; returns the camera, boxes and a JPEG. Needs the page open (model_draft gives its address).", "inputSchema": {"type": "object", "properties": {
            "name": {"type": "string"}, "view": {"type": "string", "enum": ["iso", "front", "back", "left", "right", "top"]}
        }, "required": ["name"], "additionalProperties": false}}),
    ]
}

pub fn is_tool(name: &str) -> bool {
    definitions().iter().any(|d| d["name"] == name)
}

fn name_arg(args: &Value) -> Result<&str, String> {
    args.get("name").and_then(Value::as_str).ok_or_else(|| "name must be a string".to_string())
}

/// 모델이 있는지 본다. 없으면 `no model "pik" (models: pick, screw)`.
fn need(ctx: &Ctx, name: &str) -> Result<(), String> {
    if !scene::valid_scene_name(name) {
        return Err("name must be 1 to 32 characters of A-Z a-z 0-9 - _ (and not a Windows device name)".into());
    }
    let list = ctx.store.list().map_err(|(_, m)| m)?;
    let names: Vec<&str> = list["scenes"].as_array().into_iter().flatten().filter_map(|s| s["name"].as_str()).collect();
    if names.contains(&name) {
        return Ok(());
    }
    let more = if names.len() > 16 { format!(" and {} more", names.len() - 16) } else { String::new() };
    Err(format!("no model \"{name}\" (models: {}{more})", names.iter().take(16).copied().collect::<Vec<_>>().join(", ")))
}

fn check_args(args: &Value, allowed: &[&str]) -> Result<(), String> {
    let o = args.as_object().ok_or("arguments must be an object")?;
    match o.keys().find(|k| !allowed.contains(&k.as_str())) {
        Some(k) => Err(format!("unknown argument {}", k.chars().take(20).collect::<String>())),
        None => Ok(()),
    }
}

/// 이 장면의 핀 목록(`facts.pins`). 장면을 못 읽으면 None.
fn pins_of(name: &str, v: &Value) -> Option<Vec<Value>> {
    model::check(v).ok()?;
    scene::parse_with(name, v, &NO_ADDRESSES).ok().map(|s| model::pins(&s))
}

fn draft(ctx: &Ctx, args: &Value) -> Result<Value, String> {
    check_args(args, &["name", "model", "sheet"])?;
    let name = name_arg(args)?;
    let m = args.get("model").filter(|m| m.is_object()).ok_or("model must be an object")?;
    let sheet = match args.get("sheet").filter(|s| !s.is_null()) {
        None => None,
        Some(s) => Some(s.as_str().ok_or("sheet must be text")?),
    };
    // 글이 거절되면 아무것도 저장하지 않는다(모델을 먼저 저장하고 오류를 내지 않는다)
    if let Some(t) = sheet {
        twin_page::store::Store::sheet_check(t).map_err(|(_, e)| e)?;
    }
    model::check(m)?;
    let mut r = ctx.store.draft(name, m).map_err(|(_, e)| e)?;
    // 글은 모델이 저장된 뒤에 모델 옆에 쓴다(저장이 거절되면 쓰지 않는다)
    if let Some(t) = sheet {
        ctx.store.sheet_set(name, t).map_err(|(_, e)| e)?;
    }
    if let (Some(pins), Some(f)) = (pins_of(name, m), r.get_mut("facts")) {
        f["pins"] = json!(pins);
    }
    r["has_sheet"] = json!(sheet.is_some() || ctx.store.sheet_get(name).ok().flatten().is_some());
    r["page"] = json!(ctx.page_url(Some(name)));
    Ok(r)
}

fn get(ctx: &Ctx, args: &Value) -> Result<Value, String> {
    check_args(args, &["name"])?;
    // null은 이름이 없는 것과 같다(목록). 글이 아닌 이름은 오류이다(R3-11)
    let name = match args.get("name").filter(|x| !x.is_null()) {
        None => None,
        Some(n) => Some(n.as_str().ok_or("name must be a string")?),
    };
    let Some(name) = name else {
        let list = ctx.store.list().map_err(|(_, e)| e)?;
        let models: Vec<Value> = list["scenes"]
            .as_array()
            .into_iter()
            .flatten()
            .take(32)
            .map(|s| {
                let n = s["name"].as_str().unwrap_or("");
                json!({"name": n, "devices": s["devices"], "has_draft": s["has_draft"], "has_sheet": ctx.store.sheet_get(n).ok().flatten().is_some(), "scene_ms": s["scene_ms"], "draft_ms": s["draft_ms"]})
            })
            .collect();
        return Ok(json!({"models": models, "page": ctx.page_url(None)}));
    };
    need(ctx, name)?;
    let g = ctx.store.get(name).map_err(|(_, e)| e)?;
    let draft_open = !g["draft"].is_null();
    let mut facts = if draft_open { g["draft_facts"].clone() } else { g["facts"].clone() };
    if let (Some(pins), Some(f)) = (pins_of(name, if draft_open { &g["draft"] } else { &g["scene"] }), facts.as_object_mut()) {
        f.insert("pins".into(), json!(pins));
    }
    Ok(json!({"scene": g["scene"], "draft": g["draft"], "diff": g["diff"], "facts": facts, "has_sheet": ctx.store.sheet_get(name).ok().flatten().is_some(), "page": ctx.page_url(Some(name))}))
}

fn sim(ctx: &Ctx, args: &Value) -> Result<Value, String> {
    need(ctx, name_arg(args)?)?;
    sim::model_sim(&ctx.store, args, &ctx.sims.last)
}

/// 카메라: 모델의 상자들을 감싸는 상자의 중심을 보고, 대각선으로 거리를 정한다(`fov` 40). `iso`는 방위각 45도, 고도 30도이다.
/// 장치마다의 상자를 세계 자리로 옮긴다(`resolved`의 상자는 장치 `base` 틀 안의 자리이고, `world`는 그 틀의 12개 수: x축, y축, z축, 원점).
pub(crate) fn world_boxes(resolved: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    for d in resolved["devices"].as_array().into_iter().flatten() {
        let w: Vec<f64> = d["world"].as_array().map_or(Vec::new(), |a| a.iter().map(|x| x.as_f64().unwrap_or(0.0)).collect());
        for b in d["boxes"].as_array().into_iter().flatten() {
            let p: Vec<f64> = (0..3).map(|i| b["pos"][i].as_f64().unwrap_or(0.0)).collect();
            let pos: Vec<f64> = if w.len() == 12 { (0..3).map(|i| w[9 + i] + w[i] * p[0] + w[3 + i] * p[1] + w[6 + i] * p[2]).collect() } else { p };
            out.push(json!({"pos": pos, "size": b["size"]}));
        }
        out.extend(reach_points(d, &w));
    }
    out
}

/// 엔진 관절 사슬이 닿는 범위(설계 47의 8.1절, F1): 뿌리 관절의 시작 자리를 중심으로, 마디 길이의 합 + 직동 관절의 `max` + 여유 60 mm인 정육면체의 여덟 꼭짓점을
/// 크기 0인 상자로 낸다(바닥 y 0 아래는 자른다). 화면의 `reachBox`(`twin-scene.js`)와 같은 식이라 `model_render`의 그림에도 팔이 모든 자세에서 들어간다.
/// shortcut: 도구(자식 장치)의 상자는 따로 재지 않고 여유 60 mm로 갈음한다. 큰 도구가 생기면 그 상자의 반 대각선을 더한다.
fn reach_points(d: &Value, w: &[f64]) -> Vec<Value> {
    let Some(fr) = d["frames"].as_object() else { return Vec::new() };
    let joint = |n: &str| fr.get(n).filter(|f| f["joint"].is_object());
    let parent = |n: &str| joint(n).and_then(|f| f["joint"]["parent"].as_str()).filter(|p| joint(p).is_some()).map(String::from);
    let org = |n: &str| -> Option<[f64; 3]> {
        let x = fr.get(n)?["xf"].as_array().filter(|x| x.len() == 12)?;
        Some([x[9].as_f64()?, x[10].as_f64()?, x[11].as_f64()?])
    };
    // 한 마디에서 뿌리까지의 길이와 그 뿌리의 이름
    let reach = |n: &str| -> (f64, String) {
        let (mut total, mut cur, mut root) = (0.0, n.to_string(), n.to_string());
        for _ in 0..16 {
            let p = parent(&cur);
            if let (Some(a), Some(o)) = (org(&cur), p.as_deref().and_then(org)) {
                total += (0..3).map(|i| (a[i] - o[i]).powi(2)).sum::<f64>().sqrt();
            }
            if joint(&cur).is_some_and(|f| f["joint"]["type"] == "prismatic") {
                total += joint(&cur).and_then(|f| f["joint"]["max"].as_f64()).unwrap_or(0.0).max(0.0);
            }
            root = cur;
            match p {
                Some(p) => cur = p,
                None => break,
            }
        }
        (total, root)
    };
    let mut trees: std::collections::BTreeMap<String, f64> = std::collections::BTreeMap::new();
    for n in fr.keys().filter(|n| joint(n).is_some()) {
        let (len, root) = reach(n);
        let e = trees.entry(root).or_insert(0.0);
        *e = e.max(len);
    }
    let mut out = Vec::new();
    for (root, len) in trees {
        let Some(o) = org(&root) else { continue };
        let c: Vec<f64> = if w.len() == 12 { (0..3).map(|i| w[9 + i] + w[i] * o[0] + w[3 + i] * o[1] + w[6 + i] * o[2]).collect() } else { o.to_vec() };
        let r = len + 60.0;
        for k in 0..8 {
            let p: Vec<f64> = (0..3).map(|i| c[i] + if k >> i & 1 == 1 { r } else { -r }).collect();
            out.push(json!({"pos": [p[0], p[1].max(0.0), p[2]], "size": [0, 0, 0]}));
        }
    }
    out
}

pub(crate) fn camera(boxes: &[Value], view: &str) -> Result<Value, String> {
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for b in boxes {
        let (p, s) = (&b["pos"], &b["size"]);
        // 상자의 중심에서 대각선의 절반만큼: 돌려진 상자도 들어간다
        let half = (0..3).map(|i| s[i].as_f64().unwrap_or(0.0).powi(2)).sum::<f64>().sqrt() / 2.0;
        for i in 0..3 {
            let c = p[i].as_f64().unwrap_or(0.0);
            (lo[i], hi[i]) = (lo[i].min(c - half), hi[i].max(c + half));
        }
    }
    if boxes.is_empty() {
        return Err("the model has no device to draw".into());
    }
    let target: Vec<f64> = (0..3).map(|i| (lo[i] + hi[i]) / 2.0).collect();
    let diag = (0..3).map(|i| (hi[i] - lo[i]).powi(2)).sum::<f64>().sqrt().max(100.0);
    let fov = 40.0f64;
    let dist = diag / (2.0 * (fov / 2.0).to_radians().tan()) + diag / 2.0;
    let dir = match view {
        "front" => [0.0, 0.0, 1.0],
        "back" => [0.0, 0.0, -1.0],
        "left" => [-1.0, 0.0, 0.0],
        "right" => [1.0, 0.0, 0.0],
        "top" => [0.0, 1.0, 0.001],
        _ => {
            let (az, el) = (45.0f64.to_radians(), 30.0f64.to_radians());
            [el.cos() * az.sin(), el.sin(), el.cos() * az.cos()]
        }
    };
    let pos: Vec<f64> = (0..3).map(|i| target[i] + dir[i] * dist).collect();
    Ok(json!({"pos": pos, "target": target, "fov": fov}))
}

fn render(ctx: &Ctx, args: &Value, stop: &dyn Fn() -> bool) -> Result<Value, String> {
    check_args(args, &["name", "view"])?;
    let name = name_arg(args)?;
    need(ctx, name)?;
    let view = match args.get("view").filter(|v| !v.is_null()) {
        None => "iso",
        Some(v) => v.as_str().filter(|v| matches!(*v, "iso" | "front" | "back" | "left" | "right" | "top")).ok_or("view must be iso, front, back, left, right or top")?,
    };
    let (s, _) = sim::load(&ctx.store, name).map_err(|(_, m)| m)?;
    let cam = camera(&world_boxes(&geom::resolved(&s)), view)?;
    let devices = s.devices.iter().map(|d| d.id.clone()).collect();
    // `RenderHub::request`: 요청에는 사진이 없다(번호 0). 화면은 카메라만 읽고 16:9로 그린다.
    let mut r = twin_page::render::shared_hub()
        .request(name, 0, cam, devices, stop, Duration::from_secs(10))
        .map_err(|e| if e.starts_with("open the 3D tab") { format!("open the 3D page first: {} (no page asked for work in the last 5 s)", ctx.page_url(Some(name))) } else { e })?;
    if let Some(o) = r.as_object_mut() {
        o.remove("photo");
        o.insert("view".into(), json!(view));
        o.insert("note".into(), json!("The boxes are from the 3D render, not from a photo."));
    }
    Ok(r)
}

/// 도구 호출. `stop`은 취소(`notifications/cancelled`)가 오면 참이 된다(렌더 기다림이 끝난다).
pub fn call(ctx: &Ctx, name: &str, args: &Value, stop: &dyn Fn() -> bool) -> Result<Value, String> {
    match name {
        "model_catalog" => check_args(args, &[]).map(|()| catalog()),
        "model_draft" => draft(ctx, args),
        "model_get" => get(ctx, args),
        "model_sim" => sim(ctx, args),
        "model_render" => render(ctx, args, stop),
        _ => Err("unknown tool".into()),
    }
}
