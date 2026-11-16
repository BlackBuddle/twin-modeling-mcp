//! 확장 `print`(cargo feature `print`): 도구 `model_print`. 외형 그룹 하나를 출력 부품 하나로 보고, 구멍(`cut`)을 뺀 OpenSCAD 글을 낸다.
//! 서버는 파일을 쓰지 않고 프로그램도 시작하지 않는다(경계 시험): 글만 돌려준다. STL은 사람이 OpenSCAD로 만든다.
//! 트윈 모델 형식은 그대로다: 부품의 덩어리는 모델의 외형 항목이고, 구멍은 이 도구의 인자에만 있다.

use crate::sim;
use serde_json::{json, Value};
use twin_engine::geom::{self, Item};
use twin_page::store::Store;

const MAX_PARTS: usize = 64;
const MAX_CUTS: usize = 32;
/// `part = "all"`일 때 부품 사이의 틈(mm)
const GAP: f64 = 10.0;

pub fn definition() -> Value {
    json!({"name": "model_print", "description": "Read-only. Extension `print`: returns OpenSCAD text to 3D print parts of a model. One skin group is one part: its items are the solid, and the `cut` items are removed from it (holes, slots). Writes no file and makes no STL: save `scad` as a .scad file and convert it with OpenSCAD. Checks no wall thickness, overhang, clearance or strength.", "inputSchema": {"type": "object", "properties": {
        "name": {"type": "string"},
        "parts": {"type": "array", "minItems": 1, "maxItems": MAX_PARTS, "items": {"type": "object"}, "description": "{group: \"<device id>.<skin group name>\", cut: [at most 32 of {t:box,size,pos,rot}|{t:cyl,d,len,d2,axis,pos,rot}]}. A cut item is in the frame of its group, as the skin items are; the origin of a joint frame is on the joint axis. Make a cut 0.2 mm or more longer than the solid that it goes through."}
    }, "required": ["name", "parts"], "additionalProperties": false}})
}

/// `model_print`: 모델(초안이 있으면 초안)을 검사한 뒤 읽는다. 검사에 걸린 모델은 내지 않는다.
pub fn model_print(st: &Store, args: &Value) -> Result<Value, String> {
    let name = args.get("name").and_then(Value::as_str).ok_or("name must be a string")?;
    sim::load(st, name).map_err(|(_, m)| m)?;
    let (model, _) = st.base_kind(name).map_err(|(_, m)| m)?;
    scad(name, &model, args.get("parts").unwrap_or(&Value::Null))
}

struct Part {
    group: String,
    module: String,
    frame: String,
    solids: Vec<Item>,
    cuts: Vec<Item>,
}

fn short(s: &str) -> String {
    s.chars().take(24).collect()
}

/// OpenSCAD 글의 수: 0.000001 단위(회전 행렬이 틀어지지 않게), -0은 0
fn t(x: f64) -> String {
    let r = (x * 1e6).round() / 1e6;
    format!("{}", if r == 0.0 { 0.0 } else { r })
}

/// 항목 하나. 원기둥은 OpenSCAD의 z축 기둥을 항목의 축으로 돌린다(`d`가 낮은 쪽, `d2`가 높은 쪽: 화면과 같다).
fn item_text(it: &Item) -> String {
    let x = it.xf();
    let row = |k: usize| format!("[{}, {}, {}, {}]", t(x.r[0][k]), t(x.r[1][k]), t(x.r[2][k]), t(x.p[k]));
    let body = if it.cyl {
        let turn = match it.axis {
            'x' => "rotate([0, 90, 0]) ",
            'y' => "rotate([-90, 0, 0]) ",
            _ => "",
        };
        format!("{turn}cylinder(h = {}, d1 = {}, d2 = {}, center = true);", t(it.len), t(it.d), t(it.d2))
    } else {
        format!("cube([{}, {}, {}], center = true);", t(it.size[0]), t(it.size[1]), t(it.size[2]))
    };
    format!("multmatrix([{}, {}, {}, [0, 0, 0, 1]]) {body}", row(0), row(1), row(2))
}

/// 덩어리 항목들을 감싸는 상자(그룹의 틀 좌표): (낮은 점, 높은 점). 돌린 항목은 그 바깥 상자의 꼭짓점으로 잰다.
fn bounds(items: &[Item]) -> ([f64; 3], [f64; 3]) {
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for it in items {
        let x = it.xf();
        for k in 0..8 {
            let p = x.pt([0, 1, 2].map(|i| if k >> i & 1 == 1 { it.size[i] / 2.0 } else { -it.size[i] / 2.0 }));
            for i in 0..3 {
                (lo[i], hi[i]) = (lo[i].min(p[i]), hi[i].max(p[i]));
            }
        }
    }
    (lo, hi)
}

/// 모델 글(검사를 지난 것)과 `parts`로 OpenSCAD 글을 만든다. 트윈의 +y(위)가 OpenSCAD의 +z이다: 회전이라 좌우가 뒤집히지 않는다.
pub fn scad(name: &str, model: &Value, parts: &Value) -> Result<Value, String> {
    let list = parts.as_array().filter(|a| (1..=MAX_PARTS).contains(&a.len())).ok_or(format!("parts must be 1 to {MAX_PARTS} parts"))?;
    let mut out: Vec<Part> = Vec::new();
    for (i, p) in list.iter().enumerate() {
        let f = format!("parts[{i}]");
        let o = p.as_object().ok_or(format!("{f} must be an object"))?;
        if let Some(k) = o.keys().find(|k| !matches!(k.as_str(), "group" | "cut")) {
            return Err(format!("{f}.{}: unknown field", short(k)));
        }
        let (id, gname) = o.get("group").and_then(Value::as_str).and_then(|g| g.split_once('.')).ok_or(format!("{f}.group must be \"<device id>.<skin group name>\""))?;
        let dev = model["devices"].as_array().into_iter().flatten().find(|d| d["id"] == id).ok_or(format!("{f}.group: no device \"{}\"", short(id)))?;
        let groups: Vec<&Value> = dev["skin"]["groups"].as_array().into_iter().flatten().collect();
        let Some(g) = groups.iter().find(|g| g["name"] == gname) else {
            let names: Vec<&str> = groups.iter().filter_map(|g| g["name"].as_str()).collect();
            return Err(format!("{f}.group: device {id} has no skin group \"{}\" (groups: {})", short(gname), names.join(", ")));
        };
        // 여기부터 이름은 모델의 것이다(검사를 지난 글자만 있다): OpenSCAD 글에 넣어도 된다
        let group = format!("{id}.{gname}");
        let module = format!("p_{id}_{gname}").replace('-', "_");
        if out.iter().any(|q| q.group == group) {
            return Err(format!("{f}.group: {group} is used twice"));
        }
        if out.iter().any(|q| q.module == module) {
            return Err(format!("{f}.group: {group} and another part make the same module name {module}"));
        }
        let item = |v: &Value, look: bool, at: String| geom::parse_item(v, look).map_err(|(fld, m)| if fld.is_empty() { format!("{at}: {m}") } else { format!("{at}.{}: {m}", short(&fld)) });
        let solids = g["items"].as_array().into_iter().flatten().enumerate().map(|(j, v)| item(v, true, format!("{group}.items[{j}]"))).collect::<Result<Vec<_>, _>>()?;
        if solids.is_empty() {
            return Err(format!("{f}.group: {group} has no items"));
        }
        let cuts = match o.get("cut").filter(|c| !c.is_null()) {
            None => Vec::new(),
            Some(c) => {
                let a = c.as_array().filter(|a| a.len() <= MAX_CUTS).ok_or(format!("{f}.cut must be at most {MAX_CUTS} items"))?;
                a.iter().enumerate().map(|(j, v)| item(v, false, format!("{f}.cut[{j}]"))).collect::<Result<Vec<_>, _>>()?
            }
        };
        out.push(Part { group, module, frame: g["frame"].as_str().unwrap_or("base").to_string(), solids, cuts });
    }

    let names: Vec<String> = out.iter().map(|p| format!("\"{}\"", p.group)).collect();
    let mut text = format!("// twin-modeling-mcp model_print: {name}. Units mm. Twin +y (up) is OpenSCAD +z.\n// part: \"all\" (the parts in a row) or one of {}\npart = \"all\";\n$fn = 48;\n", names.join(", "));
    let (mut all, mut one, mut facts, mut cursor) = (String::new(), String::new(), Vec::new(), 0.0);
    for p in &out {
        let lines = |items: &[Item], pad: &str| items.iter().map(|it| format!("{pad}{}\n", item_text(it))).collect::<String>();
        text += &format!("\nmodule {}() {{\n  rotate([90, 0, 0]) difference() {{\n    union() {{\n{}    }}\n{}  }}\n}}\n", p.module, lines(&p.solids, "      "), lines(&p.cuts, "    "));
        let (lo, hi) = bounds(&p.solids);
        all += &format!("  translate([{}, 0, 0]) {}();\n", t(cursor - lo[0]), p.module);
        one += &format!("if (part == \"{}\") {}();\n", p.group, p.module);
        cursor += hi[0] - lo[0] + GAP;
        // OpenSCAD 축의 크기: x, 트윈 z, 트윈 y
        facts.push(json!({"part": p.group, "module": p.module, "frame": p.frame, "solids": p.solids.len(), "cuts": p.cuts.len(), "box_mm": [geom::n(hi[0] - lo[0]), geom::n(hi[2] - lo[2]), geom::n(hi[1] - lo[1])]}));
    }
    text += &format!("\nif (part == \"all\") {{\n{all}}}\n{one}");
    Ok(json!({
        "scad": text,
        "parts": facts,
        "save_as": format!("{name}.scad"),
        "stl": format!("One part: openscad -o <part>.stl -D \"part=\\\"<part>\\\"\" {name}.scad (quote the -D value for your shell). All parts in a row: openscad -o {name}.stl {name}.scad"),
        "note": "box_mm is the outer box of the solid in OpenSCAD axes (twin x, z, y). Not checked: wall thickness, overhang, clearance, strength. A print from this text is not a tested part."
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> Value {
        json!({"schema": 2, "name": "m", "devices": [{"id": "cam-1", "kind": "machine", "skin": {"groups": [
            {"name": "plate", "frame": "lift", "items": [{"t": "box", "size": [120, 10, 60], "pos": [0, 5, 0], "tone": "orange"}]},
            {"name": "disc", "items": [{"t": "cyl", "d": 100, "len": 20, "axis": "z", "tone": "dark"}, {"t": "box", "size": [10, 20, 30], "rot": [90, 0, 0], "pos": [0, 0, 40]}]}
        ]}}]})
    }

    #[test]
    fn a_part_is_the_skin_group_minus_its_cuts() {
        let r = scad("m", &model(), &json!([{"group": "cam-1.plate", "cut": [{"t": "cyl", "d": 4, "len": 10.4, "axis": "y", "pos": [30, 5, 0]}]}, {"group": "cam-1.disc"}])).unwrap();
        let text = r["scad"].as_str().unwrap();
        // 덩어리는 union 안에, 구멍은 difference의 둘째 자식으로 있다
        let want = "module p_cam_1_plate() {\n  rotate([90, 0, 0]) difference() {\n    union() {\n      multmatrix([[1, 0, 0, 0], [0, 1, 0, 5], [0, 0, 1, 0], [0, 0, 0, 1]]) cube([120, 10, 60], center = true);\n    }\n    multmatrix([[1, 0, 0, 30], [0, 1, 0, 5], [0, 0, 1, 0], [0, 0, 0, 1]]) rotate([-90, 0, 0]) cylinder(h = 10.4, d1 = 4, d2 = 4, center = true);\n  }\n}\n";
        assert!(text.contains(want), "{text}");
        assert!(text.contains("if (part == \"cam-1.plate\") p_cam_1_plate();"));
        // 부품마다 자기 낮은 x만큼 밀려 놓인다: 첫 부품 0 + 60, 둘째는 폭 120 + 틈 10 뒤에 + 50
        assert!(text.contains("  translate([60, 0, 0]) p_cam_1_plate();\n  translate([180, 0, 0]) p_cam_1_disc();"), "{text}");
        assert_eq!(text.matches('{').count(), text.matches('}').count());
        assert_eq!(r["parts"][0], json!({"part": "cam-1.plate", "module": "p_cam_1_plate", "frame": "lift", "solids": 1, "cuts": 1, "box_mm": [120, 60, 10]}));
        // 돌린 상자: rot [90,0,0]은 +y를 +z로 보낸다 → 틀에서 [10, 30, 20], z 30~50. 원판(z ±10)과 합쳐 z 60
        assert_eq!(r["parts"][1]["box_mm"], json!([100, 60, 100]));
        assert_eq!(r["parts"][1]["frame"], "base");
    }

    #[test]
    fn a_wrong_part_names_its_field() {
        let e = |p: Value| scad("m", &model(), &p).unwrap_err();
        assert_eq!(e(json!([])), "parts must be 1 to 64 parts");
        assert_eq!(e(json!([{"group": "cam-1.plate", "hole": []}])), "parts[0].hole: unknown field");
        assert_eq!(e(json!([{"group": "plate"}])), "parts[0].group must be \"<device id>.<skin group name>\"");
        assert_eq!(e(json!([{"group": "x.plate"}])), "parts[0].group: no device \"x\"");
        assert_eq!(e(json!([{"group": "cam-1.lid"}])), "parts[0].group: device cam-1 has no skin group \"lid\" (groups: plate, disc)");
        assert_eq!(e(json!([{"group": "cam-1.disc"}, {"group": "cam-1.disc"}])), "parts[1].group: cam-1.disc is used twice");
        assert_eq!(e(json!([{"group": "cam-1.disc", "cut": [{"t": "cyl", "d": 4}]}])), "parts[0].cut[0].len: must be 0.5 to 20000");
        assert_eq!(e(json!([{"group": "cam-1.disc", "cut": [{"t": "box", "size": [1, 1, 1], "tone": "red"}]}])), "parts[0].cut[0].tone: unknown field");
    }
}
