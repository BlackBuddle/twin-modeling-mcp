//! 트윈 모델의 시험(reusable-parts 3.1d-1): `model::check`, 핀, 핀 형식의 `show`·`hide`·`indicators`, 핀 사건으로 도는 실행, 여섯 기계.
//! 이 파일은 PLC 주소를 하나도 쓰지 않는다(공개 저장소로 갈 수 있다).

use serde_json::{json, Value};
use twin_engine::bind::NO_ADDRESSES;
use twin_engine::model;
use twin_engine::scene::{self, Scene};

fn read(s: &str) -> Value {
    serde_json::from_str(s).unwrap()
}

fn fixtures() -> Vec<(&'static str, Value)> {
    vec![
        ("m1", read(include_str!("fixtures/m1-screw.json"))),
        ("m2", read(include_str!("fixtures/m2-index.json"))),
        ("m3", read(include_str!("fixtures/m3-robot.json"))),
        ("m4", read(include_str!("fixtures/m4-rover.json"))),
        ("m5", read(include_str!("fixtures/m5-grip.json"))),
        ("m6", read(include_str!("fixtures/m6-cam.json"))),
    ]
}

/// 서버가 하는 읽기: `check`, 그 뒤 주소 없는 호스트로 `parse_with`
fn load(v: &Value) -> Result<Scene, String> {
    model::check(v)?;
    scene::parse_with("t", v, &NO_ADDRESSES)
}

fn run(s: &Scene, args: &Value) -> Result<Value, String> {
    model::sim(s, args, &mut || false)
}

/// 장면 JSON의 한 장치에 키를 더하거나 바꾼다
fn with(mut v: Value, dev: usize, key: &str, val: Value) -> Value {
    v["devices"][dev][key] = val;
    v
}

/// 핀 형식 시험에 쓰는 작은 모델: 쌍동 실린더 하나
fn cyl() -> Value {
    json!({"schema": 2, "devices": [{"id": "cyl", "kind": "cylinder_double"}]})
}

fn lit() -> Value {
    json!({"groups": [{"name": "lit", "show": "cyl.coil_fwd", "items": [{"t": "box", "size": [5, 5, 5], "pos": [0, 0, 0], "tone": "mark"}]}]})
}

#[test]
fn model_check_refuses_each_left_out_key_with_its_text() {
    let ok = cyl();
    assert!(load(&ok).is_ok());
    let cases: Vec<(Value, &str)> = vec![
        (json!({"schema": 1, "devices": []}), "scene.schema: a twin model is schema 2"),
        (json!({"devices": []}), "scene.schema: a twin model is schema 2"),
        (json!({"schema": 2, "photos": [{"file": "a.jpg"}], "devices": []}), "scene.photos: a twin model names no photo file (photos stay with the person)"),
        (json!({"schema": 2, "modules": {}, "devices": []}), "scene.modules: a module model is a PLC feature; plc-mcp adds it when it binds the model"),
        (with(cyl(), 0, "io", json!({"coil_fwd": "Y1"})), "device cyl: io: a twin model holds no PLC address; leave io out (its slots are the pins cyl.<slot>, bound by plc-mcp)"),
        (
            with(read(include_str!("fixtures/m1-screw.json"))["model"].clone(), 0, "signals", json!([{"name": "target", "dir": "out", "type": "word", "device": "D1"}])),
            "device screw: signals[0].device: a twin model holds no PLC address; leave device out (the pin is screw.target)",
        ),
        (with(cyl(), 0, "skin", json!({"groups": [{"name": "g", "show": "Y3A", "items": [{"t": "box", "size": [5, 5, 5], "pos": [0, 0, 0]}]}]})), "device cyl: skin.groups[0].show: a twin model holds no PLC address; use a pin \"<device id>.<signal>\""),
        (with(cyl(), 0, "skin", json!({"groups": [{"name": "g", "hide": "X1", "items": [{"t": "box", "size": [5, 5, 5], "pos": [0, 0, 0]}]}]})), "device cyl: skin.groups[0].hide: a twin model holds no PLC address; use a pin \"<device id>.<signal>\""),
        (with(cyl(), 0, "indicators", json!([{"from": "X0", "count": 2, "at": [0, 0, 0], "step": [10, 0, 0]}])), "device cyl: indicators[0].from: a twin model holds no PLC address; use pins: [\"<device id>.<signal>\", ...]"),
        (with(cyl(), 0, "position", json!({"from": "D0"})), "device cyl: position: a twin model holds no PLC address; use mech with the preset servo_screw and a word signal"),
    ];
    for (v, want) in cases {
        assert_eq!(load(&v).err().as_deref(), Some(want), "{v}");
    }
}

#[test]
fn model_check_refuses_a_signal_named_like_a_slot_and_too_many_pins() {
    let v = json!({"schema": 2, "devices": [{"id": "idx", "kind": "index_table", "signals": [{"name": "run", "dir": "out", "type": "bit"}]}]});
    // `index_table` has no signal list in this catalog; `check` still gives the pin text first (the order of the table), as the design says
    let e = model::check(&v).unwrap_err();
    assert_eq!(e, "device idx: signals[0].name: run is a slot of index_table; a pin name is used once");
    // 1025 핀: 칸 4개짜리 쌍동 실린더 257개는 장치 한도(200)에 걸리므로 신호 32개짜리 `machine`으로 센다
    let sigs: Vec<Value> = (0..32).map(|i| json!({"name": format!("s{i}"), "dir": "in", "type": "bit"})).collect();
    let devs: Vec<Value> = (0..33).map(|i| json!({"id": format!("m{i}"), "kind": "machine", "signals": sigs})).collect();
    let big = json!({"schema": 2, "devices": devs});
    assert_eq!(model::check(&big).unwrap_err(), "scene: more than 1024 pins");
    let devs: Vec<Value> = (0..32).map(|i| json!({"id": format!("m{i}"), "kind": "machine", "signals": sigs})).collect();
    assert!(model::check(&json!({"schema": 2, "devices": devs})).is_ok(), "1024 pins are allowed");
}

#[test]
fn model_check_returns_the_first_error_in_the_documented_order() {
    // schema, photos, modules, then each device in list order: io before signals[].device before position
    let mut v = with(cyl(), 0, "io", json!({}));
    v["devices"][0]["io"] = json!({"coil_fwd": "Y1"});
    v["devices"][0]["position"] = json!({"from": "D0"});
    v["photos"] = json!([]);
    assert!(model::check(&v).unwrap_err().starts_with("scene.photos"));
    v.as_object_mut().unwrap().remove("photos");
    assert!(model::check(&v).unwrap_err().starts_with("device cyl: io:"));
}

#[test]
fn pins_of_the_screw_machine_are_the_signal_pins_in_order() {
    let m = &fixtures()[0].1;
    let s = load(&m["model"]).unwrap();
    let want = json!([
        {"pin": "screw.servo_on", "dir": "out", "type": "bit"},
        {"pin": "screw.target", "dir": "out", "type": "word"},
        {"pin": "screw.busy", "dir": "in", "type": "bit"},
        {"pin": "screw.home_sw", "dir": "in", "type": "bit"}
    ]);
    assert_eq!(Value::Array(model::pins(&s)), want);
}

#[test]
fn pins_of_a_kind_with_slots_come_in_the_catalog_order_then_signals() {
    let s = load(&cyl()).unwrap();
    let names: Vec<String> = model::pins(&s).iter().map(|p| p["pin"].as_str().unwrap().to_string()).collect();
    assert_eq!(names.len(), 4);
    assert!(names.iter().all(|n| n.starts_with("cyl.")));
    // 모든 칸이 핀이다(`io`를 안 적어도), 방향은 칸의 방향
    let p = model::pins(&s);
    assert_eq!(p.iter().filter(|x| x["dir"] == "out").count(), 2);
    assert_eq!(p.iter().filter(|x| x["dir"] == "in").count(), 2);
    let pa = scene::pin_addresses(&s);
    assert_eq!(pa.get("cyl.coil_fwd").map(String::as_str), Some("cyl.coil_fwd"));
}

#[test]
fn show_hide_and_indicators_take_pins() {
    let mut v = with(cyl(), 0, "skin", lit());
    v["devices"][0]["indicators"] = json!([{"pins": ["cyl.sensor_back", "cyl.sensor_fwd"], "at": [0, 100, 0], "step": [10, 0, 0]}]);
    let s = load(&v).unwrap();
    assert_eq!(s.skins[0].lamps, 2);
    assert!(s.skins[0].dark.is_empty());
    // hide
    let mut h = with(cyl(), 0, "skin", lit());
    h["devices"][0]["skin"]["groups"][0].as_object_mut().unwrap().remove("show");
    h["devices"][0]["skin"]["groups"][0]["hide"] = json!("cyl.sensor_back");
    h["devices"][0]["skin"]["groups"][0]["items"][0].as_object_mut().unwrap().remove("tone");
    assert!(load(&h).is_ok());
}

#[test]
fn a_bad_pin_in_show_or_an_indicator_row_gives_a_clear_text() {
    let skin = |show: Value| with(cyl(), 0, "skin", json!({"groups": [{"name": "g", "show": show, "items": [{"t": "box", "size": [5, 5, 5], "pos": [0, 0, 0]}]}]}));
    let cases = [
        (skin(json!("cyl.coil_fw")), "device cyl: skin.groups[0].show: no pin \"cyl.coil_fw\" (pins of cyl: cyl.coil_back, cyl.coil_fwd, cyl.sensor_back, cyl.sensor_fwd)"),
        (skin(json!("pump.run")), "device cyl: skin.groups[0].show: no device \"pump\" (devices: cyl)"),
        (skin(json!("noDot")), "device cyl: skin.groups[0].show: a twin model holds no PLC address; use a pin \"<device id>.<signal>\""),
        (skin(json!(5)), "device cyl: skin.groups[0].show: a twin model holds no PLC address; use a pin \"<device id>.<signal>\""),
    ];
    for (v, want) in cases {
        assert_eq!(load(&v).err().as_deref(), Some(want), "{v}");
    }
    let ind = |r: Value| with(cyl(), 0, "indicators", json!([r]));
    let at = json!([0, 0, 0]);
    let cases = [
        (ind(json!({"pins": [], "at": at})), "device cyl: indicators[0].pins: must be 1 to 64 pins"),
        (ind(json!({"pins": ["cyl.nope"], "at": at})), "device cyl: indicators[0].pins[0]: no pin \"cyl.nope\" (pins of cyl: cyl.coil_back, cyl.coil_fwd, cyl.sensor_back, cyl.sensor_fwd)"),
        (ind(json!({"pins": ["bad"], "at": at})), "device cyl: indicators[0].pins[0]: must be a pin \"<device id>.<signal>\""),
        // `from` in a twin model: the text of `check` (the "not both" text is for a host with addresses: see the plc-mcp test)
        (ind(json!({"pins": ["cyl.coil_fwd"], "from": "X1", "at": at})), "device cyl: indicators[0].from: a twin model holds no PLC address; use pins: [\"<device id>.<signal>\", ...]"),
        (ind(json!({"pins": ["cyl.coil_fwd", "cyl.coil_back"], "at": at})), "device cyl: indicators[0].step: must be 3 numbers from -1000 to 1000 (needed when count is above 1)"),
    ];
    for (v, want) in cases {
        assert_eq!(load(&v).err().as_deref(), Some(want), "{v}");
    }
}

#[test]
fn a_word_pin_is_no_lamp_and_no_show() {
    let m = read(include_str!("fixtures/m1-screw.json"))["model"].clone();
    let v = with(m.clone(), 0, "indicators", json!([{"pins": ["screw.target"], "at": [0, 0, 0]}]));
    assert_eq!(load(&v).err().as_deref(), Some("device screw: indicators[0].pins[0]: screw.target is a word pin; a lamp takes a bit pin"));
    let v = with(m, 0, "skin", json!({"groups": [{"name": "g", "show": "screw.target", "items": [{"t": "box", "size": [5, 5, 5], "pos": [0, 0, 0]}]}]}));
    assert_eq!(load(&v).err().as_deref(), Some("device screw: skin.groups[0].show: screw.target is a word pin; show and hide take a bit pin"));
}

#[test]
fn the_screw_run_gives_the_hand_values_of_report_45() {
    let m = &fixtures()[0].1;
    let s = load(&m["model"]).unwrap();
    let r = run(&s, &m["args"]).unwrap();
    let want = json!([
        {"t": 0, "device": "screw.home_sw", "value": 1}, {"t": 0, "device": "screw.servo_on", "value": 1},
        {"t": 110, "device": "screw.busy", "value": 1}, {"t": 120, "device": "screw.home_sw", "value": 0},
        {"t": 2100, "device": "screw.busy", "value": 0}, {"t": 2510, "device": "screw.busy", "value": 1},
        {"t": 4490, "device": "screw.home_sw", "value": 1}, {"t": 4500, "device": "screw.busy", "value": 0}
    ]);
    assert_eq!(r["changes"], want);
    assert_eq!(r["watch"], json!(["screw.home_sw", "screw.busy", "screw.servo_on"]));
    for (t, c, mo) in [(600, 50.0, 45.0), (1100, 100.0, 90.0), (2100, 200.0, 180.0), (3500, 100.0, 90.0), (4500, 0.0, 0.0)] {
        let j = r["joints"].as_array().unwrap().iter().find(|j| j["t"] == t).unwrap_or_else(|| panic!("no joints at {t}"));
        assert_eq!(j["screw"], json!({"carriage": c, "motor": mo}), "t {t}");
        let p = r["positions"].as_array().unwrap().iter().find(|p| p["t"] == t).unwrap();
        assert_eq!(p["screw"], json!(c), "positions at {t}");
    }
    assert!(r.get("plc_stopped").is_none() && r.get("modules").is_none());
    assert_eq!(r["note"], model::SIM_NOTE);
    assert_eq!(r["simulated"], true);
}

#[test]
fn the_other_five_machines_run_and_the_values_are_stable() {
    for (n, f) in &fixtures()[1..] {
        let s = load(&f["model"]).unwrap_or_else(|e| panic!("{n}: {e}"));
        let r = run(&s, &f["args"]).unwrap_or_else(|e| panic!("{n}: {e}"));
        assert!(!r["changes"].as_array().unwrap().is_empty(), "{n}: nothing changed");
        assert!(r["joints"].as_array().is_some_and(|j| !j.is_empty()), "{n}: no joints");
        // 같은 입력은 같은 JSON이다
        assert_eq!(run(&s, &f["args"]).unwrap(), r, "{n}");
    }
}

#[test]
fn default_watch_is_the_first_bit_in_pins_by_name() {
    let s = load(&fixtures()[0].1["model"]).unwrap();
    let r = run(&s, &json!({"total_ms": 20})).unwrap();
    assert_eq!(r["watch"], json!(["screw.busy", "screw.home_sw"]));
}

#[test]
fn event_and_pin_texts_are_literal() {
    let s = load(&fixtures()[0].1["model"]).unwrap();
    let ev = |e: Value| run(&s, &json!({"events": [e]})).unwrap_err();
    assert_eq!(ev(json!({"t_ms": 0, "pin": "screw.targt", "value": 1})), "events[0].pin: no pin \"screw.targt\" (pins of screw: screw.busy, screw.home_sw, screw.servo_on, screw.target)");
    assert_eq!(ev(json!({"t_ms": 0, "pin": "screw.busy", "value": 1})), "events[0].pin: screw.busy is written by the plant (dir in); an event sets a pin with dir out");
    assert_eq!(ev(json!({"t_ms": 0, "pin": "screw.target", "value": 70000})), "events[0].value: 70000 is outside the 16-bit range of screw.target (-32768 to 32767)");
    assert_eq!(ev(json!({"t_ms": 0, "pin": "other.target", "value": 1})), "events[0].pin: no device \"other\" (devices: screw)");
    assert_eq!(ev(json!({"t_ms": 0, "pin": "target", "value": 1})), "events[0].pin: must be a pin \"<device id>.<name>\"");
    assert_eq!(ev(json!({"t_ms": 70000, "pin": "screw.target", "value": 1})), "events[0].t_ms: must be an integer from 0 to 60000");
    assert_eq!(ev(json!({"t_ms": 0, "pin": "screw.target", "value": "x"})), "events[0].value: must be an integer or true or false");
    assert_eq!(ev(json!({"t_ms": 0, "pin": "screw.target", "value": 1, "x": 1})), "events[0].x: unknown field");
    assert_eq!(run(&s, &json!({"watch": ["screw.nope"]})).unwrap_err(), "watch[0]: no pin \"screw.nope\" (pins of screw: screw.busy, screw.home_sw, screw.servo_on, screw.target)");
    assert_eq!(run(&s, &json!({"total_ms": 5})).unwrap_err(), "total_ms must be an integer from 10 to 60000");
    assert_eq!(run(&s, &json!({"scan_ms": 0})).unwrap_err(), "scan_ms must be an integer from 1 to 100");
    assert_eq!(run(&s, &json!({"bad": 1})).unwrap_err(), "unknown argument bad");
    let many: Vec<Value> = (0..101).map(|_| json!({"t_ms": 0, "pin": "screw.servo_on", "value": 1})).collect();
    assert_eq!(run(&s, &json!({"events": many})).unwrap_err(), "events: must be an array of at most 100");
}

#[test]
fn a_32_bit_follow_pin_takes_a_32_bit_value_and_a_16_bit_pin_does_not() {
    let mut m = fixtures()[0].1["model"].clone();
    m["devices"][0]["mech"]["drivers"][0]["width"] = json!(32);
    let s = load(&m).unwrap();
    let ev = |v: i64| run(&s, &json!({"events": [{"t_ms": 0, "pin": "screw.target", "value": v}], "total_ms": 20}));
    assert!(ev(70000).is_ok());
    assert_eq!(ev(3_000_000_000).unwrap_err(), "events[0].value: 3000000000 is outside the 32-bit range of screw.target (-2147483648 to 2147483647)");
}

#[test]
fn a_bcd_pin_takes_0_to_9999() {
    let m = read(include_str!("fixtures/m3-robot.json"))["model"].clone();
    let mut m = m;
    m["devices"][0]["signals"][1]["type"] = json!("bcd");
    let s = load(&m).unwrap();
    let ev = |v: i64| run(&s, &json!({"events": [{"t_ms": 0, "pin": "robot.prog", "value": v}], "total_ms": 20}));
    assert!(ev(2).is_ok());
    assert_eq!(ev(12345).unwrap_err(), "events[0].value: 12345 is not a number 0 to 9999 for the bcd pin robot.prog");
    assert_eq!(model::pins(&s).iter().find(|p| p["pin"] == "robot.prog").unwrap()["type"], "bcd");
}

#[test]
fn the_wall_clock_callback_stops_a_run() {
    let s = load(&fixtures()[0].1["model"]).unwrap();
    let mut n = 0;
    let e = model::sim(&s, &json!({"total_ms": 1000}), &mut || {
        n += 1;
        n > 3
    })
    .unwrap_err();
    assert_eq!(e, "the simulation took more than 5 s of real time (stopped at t = 40 ms of 1000 ms); make the run shorter or raise scan_ms");
}

#[test]
fn a_part_event_without_workpieces_is_refused() {
    let s = load(&fixtures()[0].1["model"]).unwrap();
    assert_eq!(run(&s, &json!({"part_events": [{"t_ms": 0, "remove": "x"}]})).unwrap_err(), "part_events[0]: the scene has no workpieces");
}

#[test]
fn the_address_form_of_show_is_a_pin_text_error_even_without_check() {
    // `parse_with` alone (a host that skips `check`) also refuses an address form
    let v = with(cyl(), 0, "skin", json!({"groups": [{"name": "g", "show": "Y3A", "items": [{"t": "box", "size": [5, 5, 5], "pos": [0, 0, 0]}]}]}));
    assert_eq!(scene::parse_with("t", &v, &NO_ADDRESSES).err().as_deref(), Some("device cyl: skin.groups[0].show: a twin model holds no PLC address; use a pin \"<device id>.<signal>\""));
    let v = with(cyl(), 0, "indicators", json!([{"from": "X0", "count": 1, "at": [0, 0, 0]}]));
    assert_eq!(scene::parse_with("t", &v, &NO_ADDRESSES).err().as_deref(), Some("device cyl: indicators[0].from: a twin model holds no PLC address; use pins: [\"<device id>.<signal>\", ...]"));
}
