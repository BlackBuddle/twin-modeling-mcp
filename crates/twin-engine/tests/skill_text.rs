//! Tests of the skill text (doc 46 section 5.4, items 3 and 4), placed in the engine crate so that the public repository has them:
//! the core `skills/twin-modeling/SKILL.md` names no tool and no PLC address, and each example JSON under `## Example` is a valid twin model
//! (it passes `model::check` and `parse_with(.., &NO_ADDRESSES)`) and gives the cycle that the text says.

use serde_json::{json, Value};
use twin_engine::bind::NO_ADDRESSES;
use twin_engine::{model, scene};

fn skill() -> String {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../skills/twin-modeling/SKILL.md");
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("cannot read the skill text: {e}")).replace('\r', "")
}

/// The body: the text after the front matter (the two `---` lines).
fn body(s: &str) -> &str {
    let rest = s.strip_prefix("---\n").expect("the skill starts with front matter");
    rest.split_once("\n---\n").expect("the front matter ends with ---").1
}

/// (heading, fenced block) for each ```json block
fn json_blocks(body: &str) -> Vec<(String, String)> {
    let (mut out, mut heading, mut cur, mut open) = (Vec::new(), String::new(), String::new(), false);
    for l in body.lines() {
        if open {
            if l.starts_with("```") {
                out.push((heading.clone(), std::mem::take(&mut cur)));
                open = false;
            } else {
                cur.push_str(l);
                cur.push('\n');
            }
        } else if l.starts_with("```json") {
            open = true;
        } else if l.starts_with("## ") {
            heading = l.to_string();
        }
    }
    out
}

/// The text outside every fenced block
fn prose(body: &str) -> String {
    let mut out = String::new();
    let mut fenced = false;
    for l in body.lines() {
        if l.starts_with("```") {
            fenced = !fenced;
        } else if !fenced {
            out.push_str(l);
            out.push('\n');
        }
    }
    out
}

/// A word that `\b([XY][0-9A-F]{1,4}|[MD][0-9]{1,5})\b` matches: the look of an X, Y, M or D address
fn looks_like_address(w: &str) -> bool {
    let (h, t) = w.split_at(w.chars().next().map_or(0, char::len_utf8));
    match h {
        "X" | "Y" => (1..=4).contains(&t.len()) && t.chars().all(|c| c.is_ascii_digit() || ('A'..='F').contains(&c)),
        "M" | "D" => (1..=5).contains(&t.len()) && t.chars().all(|c| c.is_ascii_digit()),
        _ => false,
    }
}

#[test]
fn the_core_names_no_tool_and_no_address() {
    // the check sees what it must see
    assert!(["Y3A", "X0", "M100", "D12287"].iter().all(|w| looks_like_address(w)) && !["Yellow", "Machine", "D", "X1G", "M123456"].iter().any(|w| looks_like_address(w)));
    let s = skill();
    let b = body(&s);
    for w in ["twin_", "model_catalog", "model_draft", "model_get", "model_sim", "model_render", "guide.", "facts", "not_simulated"] {
        assert!(!b.contains(w), "the core names `{w}`");
    }
    for word in prose(b).split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).filter(|w| !w.is_empty()) {
        assert!(!looks_like_address(word), "the core has a word that looks like a PLC address: {word}");
    }
}

#[test]
fn the_front_matter_version_is_the_version_line() {
    let s = skill();
    let fm = s.strip_prefix("---\n").unwrap().split_once("\n---\n").unwrap().0;
    let v = fm.lines().find_map(|l| l.strip_prefix("version: ")).expect("front matter has version");
    assert_eq!(body(&s).lines().find(|l| !l.is_empty()), Some(format!("Version: {v}").as_str()));
}

fn examples() -> Vec<Value> {
    let s = skill();
    let v: Vec<Value> = json_blocks(body(&s)).into_iter().filter(|(h, _)| h.starts_with("## Example")).map(|(h, j)| serde_json::from_str(&j).unwrap_or_else(|e| panic!("{h}: {e}"))).collect();
    assert!(v.len() >= 2, "the skill has two examples with a JSON block each");
    v
}

#[test]
fn the_skill_examples_are_valid_twin_models() {
    for m in examples() {
        model::check(&m).unwrap_or_else(|e| panic!("{}: check: {e}", m["name"]));
        scene::parse_with("ex", &m, &NO_ADDRESSES).unwrap_or_else(|e| panic!("{}: parse: {e}", m["name"]));
    }
}

fn changes(r: &Value) -> Vec<(i64, String, i64)> {
    r["changes"].as_array().unwrap().iter().map(|c| (c["t"].as_i64().unwrap(), c["device"].as_str().unwrap().to_string(), c["value"].as_i64().unwrap())).collect()
}

fn at(c: &[(i64, String, i64)], pin: &str, value: i64) -> Vec<i64> {
    c.iter().filter(|(_, p, v)| p == pin && *v == value).map(|x| x.0).collect()
}

/// The "Expected cycle" lines of the two examples are what the engine gives.
#[test]
fn the_expected_cycles_of_the_examples_hold() {
    let ex = examples();
    let s = scene::parse_with("ex", &ex[0], &NO_ADDRESSES).unwrap();
    let r = model::sim(&s, &json!({"events": [{"t_ms": 0, "pin": "screw.servo_on", "value": 1}, {"t_ms": 100, "pin": "screw.target", "value": 2000}], "watch": ["screw.home_sw", "screw.busy"], "total_ms": 2500}), &mut || false).unwrap();
    let c = changes(&r);
    assert_eq!(at(&c, "screw.busy", 1), vec![110]);
    assert_eq!(at(&c, "screw.busy", 0), vec![2100]);
    assert_eq!(at(&c, "screw.home_sw", 0), vec![120]);
    let j = r["joints"].as_array().unwrap().iter().find(|j| j["t"] == 2100).unwrap();
    assert_eq!(j["screw"]["carriage"], json!(200.0));
    assert_eq!(j["screw"]["motor"], json!(180.0), "22.5 degrees for each mm, 200 mm = 4500 degrees = 180 modulo 360");

    let s = scene::parse_with("ex", &ex[1], &NO_ADDRESSES).unwrap();
    let r = model::sim(&s, &json!({"events": [{"t_ms": 100, "pin": "cam.run", "value": 1}], "watch": ["cam.down_sw", "cam.up_sw", "cam.cam_home"], "total_ms": 4000}), &mut || false).unwrap();
    let c = changes(&r);
    assert_eq!(at(&c, "cam.down_sw", 0), vec![730]);
    assert_eq!(at(&c, "cam.up_sw", 1), vec![1580]);
    assert_eq!(at(&c, "cam.up_sw", 0), vec![2230]);
    assert_eq!(at(&c, "cam.cam_home", 1).last(), Some(&3690));
}
