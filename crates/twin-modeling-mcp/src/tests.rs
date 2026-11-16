//! 시험: 도구와 JSON-RPC, HTTP의 검사, 화면 시뮬레이션 경로, 스킬 글(설계 46의 5.4절 2와 5), 경계 시험(3.8절). 이 파일은 PLC 코드 검사에서 뺀다.
//! 금지 글자는 그래도 조각으로 적는다. 프로세스를 띄우는 시험은 `tests/process.rs`에 있다.

use crate::serve::{start, Ctx};
use crate::{rpc, tools};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("twin-mcp-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn server(tag: &str) -> Arc<Ctx> {
    start(tmp(tag), 0).unwrap()
}

fn fixture(n: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../twin-engine/tests/fixtures").join(n)).unwrap()).unwrap()
}

fn call(c: &Ctx, name: &str, args: Value) -> Result<Value, String> {
    tools::call(c, name, &args, &|| false)
}

/// 기계 1(공 나사 서보 축)을 초안으로 저장한다. 사건은 설계 45의 7.1절과 같다.
fn draft_screw(c: &Ctx) -> Value {
    let f = fixture("m1-screw.json");
    call(c, "model_draft", json!({"name": "screw", "model": f["model"]})).unwrap()
}

// ---------- JSON-RPC ----------

fn rpc_lines(c: &Ctx, lines: &[Value]) -> Vec<Value> {
    let input = lines.iter().map(Value::to_string).collect::<Vec<_>>().join("\n") + "\n";
    let mut out = Vec::new();
    rpc::run(c, std::io::Cursor::new(input.into_bytes()), &mut out);
    String::from_utf8(out).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

#[test]
fn initialize_names_the_server_and_negotiates_the_version() {
    let c = server("init");
    for (asked, got) in [("2025-06-18", "2025-06-18"), ("2024-11-05", "2024-11-05"), ("2025-11-25", "2025-11-25"), ("1999-01-01", "2025-11-25")] {
        let r = rpc_lines(&c, &[json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": asked, "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}})]);
        assert_eq!((r[0]["result"]["protocolVersion"].as_str(), r[0]["result"]["serverInfo"]["name"].as_str()), (Some(got), Some("twin-modeling-mcp")));
    }
}

#[test]
fn tools_list_has_the_five_tools_in_order_and_stays_below_its_limit() {
    let c = server("list");
    let r = rpc_lines(&c, &[json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}), json!({"jsonrpc": "2.0", "id": 3, "method": "nope"}), json!({"jsonrpc": "2.0", "method": "notifications/initialized"})]);
    assert_eq!(r.len(), 2, "a notification gets no reply");
    let names: Vec<&str> = r[0]["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    // 확장 `print`로 빌드하면 여섯째 도구가 끝에 붙는다(한도는 같다)
    let extra: &[&str] = if cfg!(feature = "print") { &["model_print"] } else { &[] };
    assert_eq!(names, [&["model_catalog", "model_draft", "model_get", "model_sim", "model_render"], extra].concat());
    let bytes = r[0].to_string().len();
    println!("tools/list reply: {bytes} bytes");
    assert!(bytes <= 5000, "{bytes}");
    assert_eq!(r[1]["error"]["code"], -32601);
}

#[test]
fn a_tool_call_answers_with_text_and_structured_content_and_an_unknown_tool_is_refused() {
    let c = server("call");
    let r = rpc_lines(&c, &[json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "model_get", "arguments": {}}}), json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "twin_catalog"}}), json!({"jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": {"name": "model_sim", "arguments": {"name": "pik"}}})]);
    assert_eq!(r[0]["result"]["structuredContent"]["models"], json!([]));
    assert_eq!(r[1]["error"]["code"], -32602);
    assert_eq!(r[2]["result"]["isError"], true);
    assert!(r[0]["result"]["content"][0]["text"].as_str().unwrap().contains("\"models\""));
}

/// R3-14: UTF-8이 아닌 줄은 구문 오류 답을 받고, 그 뒤의 요청은 처리된다.
#[test]
fn a_line_that_is_not_utf8_gets_a_parse_error_and_the_loop_goes_on() {
    let c = server("utf8");
    let mut input = b"\xff\xfe not json\n".to_vec();
    input.extend_from_slice(json!({"jsonrpc": "2.0", "id": 7, "method": "ping"}).to_string().as_bytes());
    input.push(b'\n');
    let mut out = Vec::new();
    rpc::run(&c, std::io::Cursor::new(input), &mut out);
    let r: Vec<Value> = String::from_utf8(out).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!((r[0]["error"]["code"].clone(), r[1]["id"].clone()), (json!(-32700), json!(7)));
}

#[test]
fn only_model_render_turns_images_into_image_items() {
    let v = json!({"x": 1, "_images": [{"mime": "image/jpeg", "data": "QUJD"}]});
    let r = rpc::tool_result("model_render", Ok(v.clone()));
    assert_eq!(r["content"][1], json!({"type": "image", "data": "QUJD", "mimeType": "image/jpeg"}));
    assert!(!r["content"][0]["text"].as_str().unwrap().contains("_images"));
    assert!(rpc::tool_result("model_get", Ok(v))["content"][0]["text"].as_str().unwrap().contains("_images"));
}

// ---------- 도구 ----------

#[test]
fn model_draft_gives_the_pins_of_acceptance_3_1d_item_1_and_writes_the_sheet() {
    let c = server("draft");
    let f = fixture("m1-screw.json");
    let r = call(&c, "model_draft", json!({"name": "screw", "model": f["model"], "sheet": "# Principle sheet\nball screw\n"})).unwrap();
    assert_eq!((r["saved"].as_str(), r["has_sheet"].clone()), (Some("draft"), json!(true)));
    assert_eq!(r["facts"]["pins"], json!([{"pin": "screw.servo_on", "dir": "out", "type": "bit"}, {"pin": "screw.target", "dir": "out", "type": "word"}, {"pin": "screw.busy", "dir": "in", "type": "bit"}, {"pin": "screw.home_sw", "dir": "in", "type": "bit"}]));
    assert!(r["page"].as_str().unwrap().ends_with("/?scene=screw"));
    // model_get: the model exists, the sheet exists, the draft is open
    let g = call(&c, "model_get", json!({"name": "screw"})).unwrap();
    assert_eq!((g["has_sheet"].clone(), g["draft"].is_null(), g["scene"].is_null()), (json!(true), false, true));
    assert_eq!(g["facts"]["pins"], r["facts"]["pins"]);
    let l = call(&c, "model_get", json!({})).unwrap();
    assert_eq!(l["models"][0]["name"], "screw");
    assert_eq!((l["models"][0]["has_draft"].clone(), l["models"][0]["has_sheet"].clone()), (json!(true), json!(true)));
    // the sheet route (the page reads it)
    let (code, _, body) = http(&c, "GET", "/api/twin/sheet?scene=screw", &[], &[]);
    assert_eq!((code, serde_json::from_str::<Value>(&body).unwrap()["sheet"].clone()), (200, json!("# Principle sheet\nball screw\n")));
    // a draft with no sheet leaves the stored sheet
    call(&c, "model_draft", json!({"name": "screw", "model": f["model"]})).unwrap();
    assert!(call(&c, "model_get", json!({"name": "screw"})).unwrap()["has_sheet"] == json!(true));
}

#[test]
fn model_draft_refuses_the_left_out_keys_with_the_literal_texts() {
    let c = server("refuse");
    let f = fixture("m1-screw.json");
    let mut m = f["model"].clone();
    m["devices"][0]["io"] = json!({"x": "Y1"});
    assert_eq!(call(&c, "model_draft", json!({"name": "screw", "model": m})).err().unwrap(), "device screw: io: a twin model holds no PLC address; leave io out (its slots are the pins screw.<slot>, bound by plc-mcp)");
    let mut m = f["model"].clone();
    m["devices"][0]["signals"][0]["device"] = json!("Y61");
    assert_eq!(call(&c, "model_draft", json!({"name": "screw", "model": m})).err().unwrap(), "device screw: signals[0].device: a twin model holds no PLC address; leave device out (the pin is screw.servo_on)");
    let mut m = f["model"].clone();
    m["schema"] = json!(1);
    assert_eq!(call(&c, "model_draft", json!({"name": "screw", "model": m})).err().unwrap(), "scene.schema: a twin model is schema 2");
    let mut m = f["model"].clone();
    m["photos"] = json!([{"file": "1.jpg"}]);
    assert_eq!(call(&c, "model_draft", json!({"name": "screw", "model": m})).err().unwrap(), "scene.photos: a twin model names no photo file (photos stay with the person)");
    assert!(call(&c, "model_draft", json!({"name": "screw", "model": f["model"], "sheet": "bad \u{1}"})).err().unwrap().contains("control characters"));
    // R3-5: 거절된 글은 모델도 저장하지 않는다(이 이름의 모델이 없다)
    assert!(call(&c, "model_get", json!({"name": "screw"})).is_err(), "a refused sheet saved the draft");
    // R3-4: 방향 문자(U+202E)도 거절한다
    assert!(call(&c, "model_draft", json!({"name": "screw", "model": f["model"], "sheet": "a\u{202E}b"})).err().unwrap().contains("control characters"));
    assert!(call(&c, "model_draft", json!({"name": "a b", "model": f["model"]})).is_err());
}

fn run_args() -> Value {
    let mut a = fixture("m1-screw.json")["args"].clone();
    a["name"] = json!("screw");
    a
}

#[test]
fn model_sim_gives_the_changes_of_report_45_and_names_what_it_ran() {
    let c = server("sim");
    draft_screw(&c);
    let r = call(&c, "model_sim", run_args()).unwrap();
    assert_eq!(r["ran"], "draft");
    let ch: Vec<(i64, &str, i64)> = r["changes"].as_array().unwrap().iter().map(|x| (x["t"].as_i64().unwrap(), x["device"].as_str().unwrap(), x["value"].as_i64().unwrap())).collect();
    assert_eq!(ch, [(0, "screw.home_sw", 1), (0, "screw.servo_on", 1), (110, "screw.busy", 1), (120, "screw.home_sw", 0), (2100, "screw.busy", 0), (2510, "screw.busy", 1), (4490, "screw.home_sw", 1), (4500, "screw.busy", 0)]);
    assert_eq!(r["note"], "A simulation pass is not a test on real equipment.");
    assert!(!r["joints"].as_array().unwrap().is_empty() && r.get("plc_stopped").is_none() && r.get("modules").is_none());
    // the texts of doc 46 3.3
    let mut a = run_args();
    a["events"] = json!([{"t_ms": 0, "pin": "screw.targt", "value": 1}]);
    assert_eq!(call(&c, "model_sim", a).err().unwrap(), "events[0].pin: no pin \"screw.targt\" (pins of screw: screw.busy, screw.home_sw, screw.servo_on, screw.target)");
    let mut a = run_args();
    a["events"] = json!([{"t_ms": 0, "pin": "screw.busy", "value": 1}]);
    assert_eq!(call(&c, "model_sim", a).err().unwrap(), "events[0].pin: screw.busy is written by the plant (dir in); an event sets a pin with dir out");
    let mut a = run_args();
    a["events"] = json!([{"t_ms": 0, "pin": "screw.target", "value": 70000}]);
    assert_eq!(call(&c, "model_sim", a).err().unwrap(), "events[0].value: 70000 is outside the 16-bit range of screw.target (-32768 to 32767)");
    assert_eq!(call(&c, "model_sim", json!({"name": "pik"})).err().unwrap(), "no model \"pik\" (models: screw)");
}

#[test]
fn model_render_needs_the_open_page_and_says_where_to_open_it() {
    let c = server("render");
    draft_screw(&c);
    let e = call(&c, "model_render", json!({"name": "screw"})).err().unwrap();
    assert!(e.starts_with("open the 3D page first: http://127.0.0.1:") && e.ends_with("/?scene=screw (no page asked for work in the last 5 s)"), "{e}");
    assert!(call(&c, "model_render", json!({"name": "screw", "view": "up"})).err().unwrap().contains("view must be"));
}

#[test]
fn the_camera_looks_at_the_middle_of_the_boxes_from_the_view_side() {
    let b = [json!({"pos": [100.0, 50.0, 0.0], "size": [200.0, 100.0, 100.0]}), json!({"pos": [300.0, 50.0, 0.0], "size": [200.0, 100.0, 100.0]})];
    let c = tools::camera(&b, "front").unwrap();
    assert_eq!(c["fov"], 40.0);
    assert!((c["target"][0].as_f64().unwrap() - 200.0).abs() < 1e-9 && c["pos"][2].as_f64().unwrap() > 0.0 && c["pos"][0] == c["target"][0]);
    assert!(tools::camera(&b, "iso").unwrap()["pos"][1].as_f64().unwrap() > c["target"][1].as_f64().unwrap());
    assert_eq!(tools::camera(&[], "iso").err().unwrap(), "the model has no device to draw");
}

#[test]
fn the_camera_box_holds_the_reach_of_an_arm() {
    // F1 (설계 47의 8.1절): 기계 3은 뿌리 관절 j1이 원점에 있고 마디 길이의 합이 1200 mm라서, 상자 여덟 꼭짓점이 ±1260 mm, 바닥 아래는 잘린다
    let c = server("reach");
    let f = fixture("m3-robot.json");
    call(&c, "model_draft", json!({"name": "robot", "model": f["model"]})).unwrap();
    let (s, _) = crate::sim::load(&c.store, "robot").unwrap();
    let pts = tools::world_boxes(&twin_engine::geom::resolved(&s));
    let reach: Vec<&Value> = pts.iter().filter(|p| p["size"] == json!([0, 0, 0])).collect();
    assert_eq!(reach.len(), 8, "one tree of joints gives one cube");
    let max = |i: usize| reach.iter().map(|p| p["pos"][i].as_f64().unwrap()).fold(f64::MIN, f64::max);
    let min = |i: usize| reach.iter().map(|p| p["pos"][i].as_f64().unwrap()).fold(f64::MAX, f64::min);
    assert!((max(0) - 1260.0).abs() < 1.0 && (min(0) + 1260.0).abs() < 1.0 && (max(2) - 1260.0).abs() < 1.0, "x {} {} z {}", min(0), max(0), max(2));
    assert!(min(1) >= 0.0 && (max(1) - 1260.0).abs() < 1.0, "y {} {}", min(1), max(1));
    // 카메라는 이 상자를 담아서 팔이 없는 모델보다 멀다
    let with = tools::camera(&pts, "iso").unwrap()["pos"].clone();
    let bare: Vec<Value> = pts.iter().filter(|p| p["size"] != json!([0, 0, 0])).cloned().collect();
    let without = tools::camera(&bare, "iso").unwrap()["pos"].clone();
    assert!(with[1].as_f64().unwrap() > without[1].as_f64().unwrap() * 2.0, "{with} {without}");
}

// ---------- HTTP ----------

fn http(c: &Ctx, method: &str, path: &str, headers: &[(&str, &str)], body: &[u8]) -> (u16, String, String) {
    let mut s = TcpStream::connect(("127.0.0.1", c.port)).unwrap();
    let mut h = format!("{method} {path} HTTP/1.1\r\n");
    if !headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("host")) {
        h += &format!("Host: 127.0.0.1:{}\r\n", c.port);
    }
    if method != "GET" && !headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("origin")) {
        h += &format!("Origin: http://127.0.0.1:{}\r\n", c.port);
    }
    if !headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("x-dashboard-token")) && !headers.iter().any(|(k, _)| *k == "no-token") {
        h += &format!("X-Dashboard-Token: {}\r\n", c.token());
    }
    for (k, v) in headers.iter().filter(|(k, _)| *k != "no-token") {
        h += &format!("{k}: {v}\r\n");
    }
    h += &format!("Content-Length: {}\r\n\r\n", body.len());
    s.write_all(h.as_bytes()).unwrap();
    s.write_all(body).unwrap();
    let mut out = Vec::new();
    let _ = s.read_to_end(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
    let code = head.split(' ').nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
    (code, head.to_ascii_lowercase(), body.to_string())
}

#[test]
fn hostile_http_requests_are_refused() {
    let c = server("hostile");
    // Host
    assert_eq!(http(&c, "GET", "/", &[("Host", "evil.example")], &[]).0, 403);
    assert_eq!(http(&c, "GET", "/api/twin/scenes", &[("Host", "evil.example")], &[]).0, 403);
    // the page needs no token; every /api/ route does
    assert_eq!(http(&c, "GET", "/", &[], &[]).0, 200);
    assert_eq!(http(&c, "GET", "/api/twin/scenes", &[("no-token", "")], &[]).0, 403);
    assert_eq!(http(&c, "GET", "/api/twin/scenes", &[("X-Dashboard-Token", "00")], &[]).0, 403);
    // Origin on a writing method
    assert_eq!(http(&c, "POST", "/api/twin/confirm", &[("Origin", "http://evil.example")], b"{\"scene\":\"x\"}").0, 403);
    assert_eq!(http(&c, "POST", "/api/ladder/sim/start", &[("Origin", "http://evil.example")], b"{}").0, 403);
    // photos and live are not part of a twin model
    let (code, _, b) = http(&c, "GET", "/api/twin/photo?scene=x&n=1", &[], &[]);
    assert_eq!((code, b.as_str()), (404, "{\"error\":\"photos are not part of a twin model\"}"));
    assert_eq!(http(&c, "GET", "/api/twin/live/state?scene=x", &[], &[]).0, 404);
    // vendor: only the fixed list
    assert_eq!(http(&c, "GET", "/vendor/twin-scene.js", &[], &[]).0, 200);
    // 공개 화면(3.1e): `GET /`은 토큰이 들어간 공개 화면이고, 내보내는 파일은 6개이며 차트(uPlot)는 404다
    let (code, h, page) = http(&c, "GET", "/", &[("no-token", "")], &[]);
    assert!(code == 200 && h.to_ascii_lowercase().contains("x-frame-options: deny") && page.contains("twin-modeling-mcp") && page.contains(&format!("const TOKEN = \"{}\"", c.token())) && !page.contains("__DASHBOARD_TOKEN__"));
    // R3-25: 화면 글은 같은 출처의 글과 연결만 허용하는 정책을 받고, JSON 답은 틀 삽입 금지만 받는다
    let h = h.to_ascii_lowercase();
    assert!(h.contains("content-security-policy: frame-ancestors 'none'; default-src 'none'; script-src 'self' 'unsafe-inline'") && h.contains("connect-src 'self'"), "{h}");
    for ok in ["three/three.module.js", "three/three.core.js", "three/controls/OrbitControls.js", "three/controls/TransformControls.js", "twin-scene.js", "twin-core.js"] {
        assert_eq!(http(&c, "GET", &format!("/vendor/{ok}"), &[], &[]).0, 200, "{ok}");
    }
    for no in ["uplot/uPlot.iife.min.js", "uplot/uPlot.min.css"] {
        assert_eq!(http(&c, "GET", &format!("/vendor/{no}"), &[], &[]).0, 404, "{no}");
    }
    for bad in ["/vendor/../Cargo.toml", "/vendor/three/../three/three.core.js", "/vendor/", "/vendor/nope.js", "/vendor/%2e%2e/x"] {
        assert_eq!(http(&c, "GET", bad, &[], &[]).0, 404, "{bad}");
    }
    // a body over the JSON limit
    assert_eq!(http(&c, "PUT", "/api/twin/draft", &[], &vec![b' '; 262_145]).0, 413);
    // R3-7: 머리글 한 줄 17,000바이트는 431
    assert_eq!(http(&c, "GET", "/api/twin/scenes", &[("X-Pad", &"a".repeat(17_000))], &[]).0, 431);
    // every reply carries the protection headers
    let (_, h, _) = http(&c, "GET", "/api/twin/scenes", &[], &[]);
    for want in ["x-frame-options: deny", "content-security-policy: frame-ancestors 'none'", "x-content-type-options: nosniff", "content-language: en"] {
        assert!(h.contains(want), "{want}: {h}");
    }
}

#[test]
fn the_listener_is_on_loopback_and_a_busy_port_is_refused() {
    let c = server("loop");
    let l = std::net::TcpListener::bind(("127.0.0.1", c.port));
    assert!(l.is_err(), "the port is taken");
    assert_eq!(start(tmp("busy"), c.port).err().unwrap(), format!("port {} is in use", c.port));
    assert!(std::net::TcpStream::connect(("127.0.0.1", c.port)).is_ok());
}

#[test]
fn a_page_text_is_korean_only_when_the_page_asks_for_it() {
    let c = server("lang");
    let bad = br#"{"scene":"x","data":{"schema":1,"devices":[]}}"#;
    let (code, h, en) = http(&c, "PUT", "/api/twin/draft", &[], bad);
    assert_eq!(code, 400);
    assert!(h.contains("content-language: en") && en.contains("scene.schema: a twin model is schema 2"), "{en}");
    let (_, h, ko) = http(&c, "PUT", "/api/twin/draft", &[("X-Page-Lang", "ko")], br#"{"scene":"x","data":{"schema":2,"devices":[{"id":"c","kind":"cylinder_double","io":{"coil_fwd":"Y1"}}]}}"#);
    assert!(h.contains("content-language: ko"), "{h}");
    assert!(ko.contains("PLC 주소"), "{ko}");
    let (_, _, other) = http(&c, "PUT", "/api/twin/draft", &[("X-Page-Lang", "fr")], bad);
    assert_eq!(other, en);
}

/// 화면의 `/api/` 경로가 낼 수 있는 새 글마다 한국어 틀이 있다(설계 47의 4.3절). 목록은 이 시험이 가진다: 글이 늘면 여기와 표에 같이 더한다.
#[test]
fn every_new_text_of_the_page_routes_has_a_korean_entry() {
    let texts = [
        "scene.schema: a twin model is schema 2",
        "scene.photos: a twin model names no photo file (photos stay with the person)",
        "scene.modules: a module model is a PLC feature; plc-mcp adds it when it binds the model",
        "device cyl: io: a twin model holds no PLC address; leave io out (its slots are the pins cyl.<slot>, bound by plc-mcp)",
        "device screw: signals[0].device: a twin model holds no PLC address; leave device out (the pin is screw.target)",
        "device vacuum: skin.groups[1].show: a twin model holds no PLC address; use a pin \"<device id>.<signal>\"",
        "device tb: indicators[0].from: a twin model holds no PLC address; use pins: [\"<device id>.<signal>\", ...]",
        "device servo: position: a twin model holds no PLC address; use mech with the preset servo_screw and a word signal",
        "device idx: signals[0].name: run is a slot of index_table; a pin name is used once",
        "scene: more than 1024 pins",
        "device cyl: skin.groups[0].show: no device \"pump\" (devices: cyl)",
        "device cyl: skin.groups[0].show: no pin \"cyl.nope\" (pins of cyl: cyl.coil_back, cyl.coil_fwd)",
        "device cyl: skin.groups[0].show: cyl.coil_fwd is a word pin; show and hide take a bit pin",
        "device cyl: indicators[0].pins[0]: cyl.x is a word pin; a lamp takes a bit pin",
        "device cyl: indicators[0]: give pins or from and count, not both",
        "device cyl: indicators[0].pins: must be 1 to 64 pins",
        "device cyl: indicators[0].pins[2]: must be a pin \"<device id>.<signal>\"",
        "cyl: skin: cyl.coil_back has no PLC address yet; its group is drawn as if the pin is OFF (bind it with io or signals[].device)",
        "screw.busy is written by the plant (dir in); an event sets a pin with dir out",
        "no pin \"screw.x\" (pins: screw.busy)",
        "value must be true or false for a bit pin",
        "no model_sim run of this model yet",
        "source must be none or last_run",
        "events[0].pin: no pin \"screw.targt\" (pins of screw: screw.busy, screw.target)",
        "events[1].pin: screw.busy is written by the plant (dir in); an event sets a pin with dir out",
        "events[2].value: 70000 is outside the 16-bit range of screw.target (-32768 to 32767)",
        "events[3].value: 12345 is not a number 0 to 9999 for the bcd pin robot.prog",
        "events[0].t_ms: must be an integer from 0 to 60000",
        "watch: must be an array of at most 40 pins",
        "the simulation took more than 5 s of real time (stopped at t = 5000 ms of 60000 ms); make the run shorter or raise scan_ms",
    ];
    for t in texts {
        let ko = twin_page::ko::text_in(t, &[twin_page::ko::KO]);
        assert_ne!(ko, t, "no Korean entry for: {t}");
        assert!(ko.chars().any(|c| ('\u{AC00}'..='\u{D7A3}').contains(&c)), "{ko}");
    }
}

// ---------- 화면의 시뮬레이션 ----------

fn post(c: &Ctx, path: &str, v: Value) -> (u16, Value) {
    let (code, _, b) = http(c, "POST", path, &[], v.to_string().as_bytes());
    (code, serde_json::from_str(&b).unwrap())
}

#[test]
fn the_page_simulation_with_no_program_shows_every_pin_and_takes_a_press() {
    let c = server("pagesim");
    draft_screw(&c);
    let (code, s) = post(&c, "/api/ladder/sim/start", json!({"scene": "screw", "source": "none", "realtime": false}));
    assert_eq!(code, 200, "{s}");
    let id = s["sim"].as_str().unwrap().to_string();
    // F2: every pin is in bits or words, never missing
    assert_eq!(s["bits"], json!({"screw.busy": false, "screw.home_sw": true, "screw.servo_on": false}), "{s}");
    assert_eq!(s["words"], json!({"screw.target": 0}));
    assert!(s.get("total_ms").is_none());
    let (_, s) = post(&c, "/api/ladder/sim/input", json!({"sim": id, "device": "screw.servo_on", "value": true}));
    assert_eq!(s["bits"]["screw.servo_on"], true);
    let (code, e) = post(&c, "/api/ladder/sim/input", json!({"sim": id, "device": "screw.busy", "value": true}));
    assert_eq!((code, e["error"].as_str()), (400, Some("screw.busy is written by the plant (dir in); an event sets a pin with dir out")));
    assert_eq!(post(&c, "/api/ladder/sim/input", json!({"sim": id, "device": "screw.nope", "value": true})).0, 400);
    // R3-6: 값은 `model_sim`의 사건과 같은 규칙으로 읽는다(범위 글이 같다)
    let (code, e) = post(&c, "/api/ladder/sim/input", json!({"sim": id, "device": "screw.target", "value": 70000}));
    assert_eq!((code, e["error"].as_str()), (400, Some("value: 70000 is outside the 16-bit range of screw.target (-32768 to 32767)")));
    let (_, s) = post(&c, "/api/ladder/sim/input", json!({"sim": id, "device": "screw.target", "value": 2000}));
    let (_, s2) = post(&c, "/api/ladder/sim/step", json!({"sim": id, "scans": 50}));
    assert_eq!((s2["ran"].as_u64(), s2["bits"]["screw.busy"].clone()), (Some(50), json!(true)), "{s} {s2}");
    assert_eq!(s2["words"]["screw.target"], 2000);
    let (code, _, g) = http(&c, "GET", &format!("/api/ladder/sim/state?sim={id}"), &[], &[]);
    assert_eq!((code, serde_json::from_str::<Value>(&g).unwrap()["sim"].as_str()), (200, Some(id.as_str())));
    assert_eq!(post(&c, "/api/ladder/sim/stop", json!({"sim": id})).0, 200);
    assert_eq!(post(&c, "/api/ladder/sim/stop", json!({"sim": id})).0, 404);
}

#[test]
fn the_source_last_run_replays_the_last_model_sim_input_to_its_total_time() {
    let c = server("lastrun");
    draft_screw(&c);
    assert_eq!(post(&c, "/api/ladder/sim/start", json!({"scene": "screw", "source": "last_run"})).0, 409);
    // R3-12: 없는 모델은 404
    assert_eq!(post(&c, "/api/ladder/sim/start", json!({"scene": "nope"})).0, 404);
    // R3-11: null은 없는 것과 같고, 글이 아닌 source는 오류
    assert_eq!(post(&c, "/api/ladder/sim/start", json!({"scene": "screw", "source": null, "realtime": null})).0, 200);
    assert_eq!(post(&c, "/api/ladder/sim/start", json!({"scene": "screw", "source": 5})).0, 400);
    assert_eq!(call(&c, "model_get", json!({"name": 5})).err().unwrap(), "name must be a string");
    call(&c, "model_sim", run_args()).unwrap();
    let (code, s) = post(&c, "/api/ladder/sim/start", json!({"scene": "screw", "source": "last_run", "realtime": false}));
    assert_eq!((code, s["total_ms"].as_i64(), s["source"].as_str()), (200, Some(5000), Some("last_run")), "{s}");
    let (_, s) = post(&c, "/api/ladder/sim/step", json!({"sim": s["sim"], "scans": 300}));
    assert_eq!((s["time_ms"].as_i64(), s["bits"]["screw.busy"].clone()), (Some(3000), json!(true)));
    let (_, s) = post(&c, "/api/ladder/sim/step", json!({"sim": s["sim"], "scans": 300}));
    assert_eq!((s["time_ms"].as_i64(), s["done"].clone(), s["ran"].as_u64()), (Some(5000), json!(true), Some(200)));
}

/// R3-3: `last_run`은 `model_sim`의 실행과 같다: part_events는 시각 순서로 돈다(늦은 항목 뒤의 이른 항목이 기다리지 않는다).
#[test]
fn last_run_orders_part_events_by_time_like_model_sim() {
    let c = server("partorder");
    let model = json!({"schema": 2, "devices": [{"id": "tbl", "kind": "solid", "mount": "floor", "shape": [{"t": "box", "size": [200, 20, 200], "pos": [0, 10, 0]}]}],
        "points": [{"name": "p1", "on": "tbl", "pos": [0, 20, 0]}],
        "workpieces": {"types": {"white": {"shape": "round", "d_mm": 20, "h_mm": 10, "material": "plastic", "tone": "white"}}}});
    call(&c, "model_draft", json!({"name": "wp", "model": model})).unwrap();
    // 100의 reset이 목록에서 500의 부품 뒤에 있다: 시각 순서로는 reset이 먼저이고 부품이 남는다
    let args = json!({"name": "wp", "total_ms": 1000, "part_events": [{"t_ms": 500, "at": "p1", "type": "white"}, {"t_ms": 100, "reset": true}]});
    let r = call(&c, "model_sim", args).unwrap();
    assert_eq!(r["parts_final"].as_array().unwrap().len(), 1, "{r}");
    let (code, s) = post(&c, "/api/ladder/sim/start", json!({"scene": "wp", "source": "last_run", "realtime": false}));
    assert_eq!(code, 200, "{s}");
    let (_, s) = post(&c, "/api/ladder/sim/step", json!({"sim": s["sim"], "scans": 100}));
    assert_eq!((s["time_ms"].as_i64(), s["plant"]["parts"].as_array().map(Vec::len)), (Some(1000), Some(1)), "{s}");
}

#[test]
fn at_most_four_page_simulations_exist() {
    let c = server("four");
    draft_screw(&c);
    let ids: Vec<String> = (0..5).map(|_| post(&c, "/api/ladder/sim/start", json!({"scene": "screw", "realtime": false})).1["sim"].as_str().unwrap().to_string()).collect();
    assert_eq!(http(&c, "GET", &format!("/api/ladder/sim/state?sim={}", ids[0]), &[], &[]).0, 404);
    assert_eq!(http(&c, "GET", &format!("/api/ladder/sim/state?sim={}", ids[4]), &[], &[]).0, 200);
}

// ---------- 스킬 글 (설계 46의 5.4절 2와 5) ----------

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn lf(s: &str) -> String {
    s.replace("\r\n", "\n")
}

/// 항목 2: `model_catalog`의 `guide.modeling`은 스킬 본문 + "\n" + 서버 부분이다.
#[test]
fn model_guide_is_the_skill_text() {
    let skill = lf(&std::fs::read_to_string(root().join("skills/twin-modeling/SKILL.md")).unwrap());
    let part = lf(&std::fs::read_to_string(root().join("skills/twin-modeling/references/twin-modeling-mcp.md")).unwrap());
    let body = skill.splitn(3, "---\n").nth(2).unwrap().trim_start_matches('\n');
    let c = tools::catalog();
    assert_eq!(c["guide"]["modeling"].as_str().unwrap(), format!("{body}\n{part}"));
    let v = body.lines().next().unwrap().trim().strip_prefix("Version:").unwrap().trim().parse::<u64>().unwrap();
    assert_eq!(c["guide"]["modeling_version"], v);
    assert!(skill.lines().any(|l| l.trim() == format!("version: {v}")), "the front matter version equals the Version: line");
    assert_eq!(c["format_version"], 1);
    assert_eq!(c["model_rules"].as_array().unwrap().len(), 9);
}

/// R3-1: 이 서버가 보내는 안내문(절차 글 `guide.modeling`을 뺀 것)에는 plc-mcp의 도구 이름과 주소 말이 없다.
/// 바꾼 글의 원문이 엔진 글에 한 번씩 있음도 본다(엔진 글이 바뀌면 조용히 지나가지 않는다).
#[test]
fn the_server_catalog_holds_no_plc_mcp_words() {
    let engine: Value = serde_json::from_str(include_str!("../../twin-engine/src/catalog.json")).unwrap();
    for (k, from, _) in tools::HOST_SWAPS {
        assert_eq!(engine["guide"][k].as_str().unwrap().matches(from).count(), 1, "{k}: {from}");
    }
    let mut c = tools::catalog();
    assert!(c["guide"].get("order").is_none());
    c["guide"].as_object_mut().unwrap().remove("modeling");
    let text = c.to_string();
    for bad in ["twin_", "ladder_", "server_info", "plc_", "bit address", "ZR word", "scenario.modules"] {
        assert!(!text.contains(bad), "the catalog still holds {bad}");
    }
}

/// 항목 5: `references/catalog.json`은 `model_catalog` 결과의 복사본이다(안내문의 절차 글만 뺀다: 그것은 스킬 글 자체이다).
/// 변수 `UPDATE_CATALOG=1`이면 시험이 그 파일을 다시 쓴다.
#[test]
fn the_catalog_copy_is_the_model_catalog_result() {
    let mut c = tools::catalog();
    if let Some(g) = c["guide"].as_object_mut() {
        g.remove("modeling");
        g.remove("modeling_version");
    }
    let want = serde_json::to_string_pretty(&c).unwrap() + "\n";
    let path = root().join("skills/twin-modeling/references/catalog.json");
    if std::env::var("UPDATE_CATALOG").as_deref() == Ok("1") {
        std::fs::write(&path, &want).unwrap();
    }
    assert_eq!(lf(&std::fs::read_to_string(&path).unwrap()), want, "run the test once with UPDATE_CATALOG=1 and commit the file");
}

// ---------- 경계 시험 (설계 46의 3.8절) ----------

fn crate_dirs() -> Vec<PathBuf> {
    ["twin-engine", "twin-page", "twin-modeling-mcp"].iter().map(|n| root().join("crates").join(n)).collect()
}

/// `[dependencies]`에는 `serde_json`과 서로 말고 아무것도 없고, 개발용과 빌드용 의존은 없다. Cargo는 그래프에 없는 크레이트를 연결하지 못한다.
#[test]
fn twin_crates_depend_only_on_serde_json_and_each_other() {
    for d in crate_dirs() {
        let t = std::fs::read_to_string(d.join("Cargo.toml")).unwrap();
        assert!(!t.contains("[dev-dependencies]") && !t.contains("[build-dependencies]") && !t.contains("[target."), "{}", d.display());
        let deps = t.split("[dependencies]").nth(1).unwrap().lines().skip(1).take_while(|l| !l.starts_with('[')).filter_map(|l| l.split_once('=')).map(|(k, _)| k.trim().to_string()).collect::<Vec<_>>();
        for n in &deps {
            assert!(matches!(n.as_str(), "serde_json" | "twin-engine" | "twin-page"), "{}: {n}", d.display());
        }
        assert!(deps.contains(&"serde_json".to_string()));
    }
}

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            rs_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// 세 크레이트의 `src/`에는 PLC, SLMP, FTP, 프로그램 코드, 밖으로 나가는 연결, 프로세스 시작이 없다. 시험 파일 자신(이 파일과 두 가드 시험 파일)은 뺀다.
#[test]
fn twin_crates_hold_no_plc_code() {
    let banned: Vec<String> = [["sl", "mp"], ["Sl", "mp"], ["SL", "MP"], ["ft", "p::"], ["F", "tp"], ["F", "TP"], ["qp", "g"], ["qp", "a"], ["qc", "d"], ["il", "::"], ["lin", "k::"], ["mel", "sec"], ["0x14", "01"], ["0x14", "02"], ["0x04", "01"], ["0x04", "03"], ["TcpStream::", "connect"], ["Udp", "Socket"], ["Com", "mand::new"], ["process::", "Command"]]
        .iter()
        .map(|p| p.concat())
        .collect();
    let mut files = Vec::new();
    for d in crate_dirs() {
        rs_files(&d.join("src"), &mut files);
    }
    let files: Vec<&PathBuf> = files.iter().filter(|p| !matches!(p.file_name().and_then(|n| n.to_str()), Some("guard_tests.rs" | "tests.rs"))).collect();
    assert!(files.len() >= 20, "the test must see the source files: {}", files.len());
    for p in files {
        let text = std::fs::read_to_string(p).unwrap();
        for b in &banned {
            assert!(!text.contains(b.as_str()), "{} contains {b}", p.display());
        }
        // 낱말 `ladder`는 이 서버의 화면 경로 `/api/ladder/sim/`의 글에만 둔다(주석은 제외)
        for l in text.lines().filter(|l| !l.trim_start().starts_with("//")) {
            assert!(!l.contains("ladder") || l.contains("/api/ladder/sim/"), "{}: {l}", p.display());
        }
    }
}
