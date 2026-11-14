//! 공개 화면(`twin-model.html`)의 글 검사 (reusable-parts 3.1e, 설계 47의 10절 9·10번과 6.2절의 토큰 검사). 화면 파일을 글로 읽어 견준다.

use std::collections::BTreeMap;
use std::path::Path;

fn ui(f: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("ui").join(f)).unwrap()
}

/// 따옴표 `'`로 묶인 글을 차례로 뽑는다(`\'` 같은 역슬래시 이스케이프를 푼다). 줄 주석(`//`로 시작하는 줄)은 뺀다.
fn quoted(block: &str) -> Vec<String> {
    let text: String = block.lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n");
    let (mut out, mut cur, mut inq, mut esc) = (Vec::new(), String::new(), false, false);
    for c in text.chars() {
        if inq {
            if esc {
                cur.push(c);
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '\'' {
                out.push(std::mem::take(&mut cur));
                inq = false;
            } else {
                cur.push(c);
            }
        } else if c == '\'' {
            inq = true;
        }
    }
    out
}

/// 글 호출 `t('키'`와 `T('키'`의 키들. 앞 글자가 이름의 글자이면(`set('`, `get('`) 호출이 아니다.
fn call_keys(src: &str) -> Vec<String> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    for name in ["t('", "T('"] {
        for (i, _) in src.match_indices(name) {
            let prev = if i == 0 { b' ' } else { b[i - 1] };
            if prev.is_ascii_alphanumeric() || matches!(prev, b'_' | b'$' | b'.') {
                continue;
            }
            let rest = &src[i + name.len()..];
            if let Some(j) = rest.find('\'') {
                out.push(rest[..j].to_string());
            }
        }
    }
    out
}

/// `const <name> = {` 부터 첫 `\n};`까지.
fn block<'a>(src: &'a str, start: &str) -> &'a str {
    let i = src.find(start).unwrap_or_else(|| panic!("no {start}")) + start.len();
    let j = src[i..].find("\n};").expect("block end") + i;
    &src[i..j]
}

/// 화면의 글 표: 키 → [영어, 한국어]
fn text_table(src: &str) -> BTreeMap<String, [String; 2]> {
    let q = quoted(block(src, "const TEXT = {"));
    assert_eq!(q.len() % 3, 0, "the table is key, English, Korean in sequence");
    q.chunks(3).map(|c| (c[0].clone(), [c[1].clone(), c[2].clone()])).collect()
}

#[test]
fn the_public_page_has_none_of_the_left_out_code() {
    let src = ui("twin-model.html");
    for bad in ["/api/write", "/api/state", "/api/twin/live", "/api/twin/photo", "/api/twin/marks", "uplot", "uPlot", "TransformControls", "innerHTML", "insertAdjacentHTML", "outerHTML"] {
        assert!(!src.contains(bad), "twin-model.html contains {bad}");
    }
    // 두 화면이 같이 쓰는 모듈에도 같은 규칙이다
    let core = ui("twin-core.js");
    for bad in ["/api/write", "/api/state", "innerHTML", "insertAdjacentHTML", "outerHTML"] {
        assert!(!core.contains(bad), "twin-core.js contains {bad}");
    }
}

#[test]
fn every_text_key_of_the_public_page_is_in_its_table_with_two_strings() {
    let src = ui("twin-model.html");
    let table = text_table(&src);
    assert!(table.len() > 100, "{} rows", table.len());
    for (k, [en, ko]) in &table {
        assert!(!en.trim().is_empty() && !ko.trim().is_empty(), "{k}: an empty string");
    }
    // `data-t` 와 `data-t-label` 이름표
    for tag in ["data-t=\"", "data-t-label=\""] {
        for part in src.split(tag).skip(1) {
            let key = part.split('"').next().unwrap();
            assert!(table.contains_key(key), "{tag}{key}");
        }
    }
    // 화면 코드 세 곳(화면의 스크립트, 공유 모듈, 렌더러)에서 글을 부르는 `t('키'`, `T('키'`의 키와, 키 모양(`그룹.이름`)의 모든 글은 표에 있다 (그룹 이름을 따지지 않으므로
    // `'sheett.tag'` 같은 오타도 걸린다). 표 자체는 뺀다. 끝이 `.`인 앞머리(`'kind.' + ...`)는 이어 붙이는 키라서 아래에서 따로 본다.
    let script = &src[src.find("<script type=\"module\">").unwrap()..];
    let after_table = &script[script.find("\n};").unwrap()..];
    let core = ui("twin-core.js");
    let scene = ui("twin-scene.js");
    for (file, code) in [("twin-model.html", after_table), ("twin-core.js", core.as_str()), ("twin-scene.js", scene.as_str())] {
        for k in call_keys(code) {
            if !k.ends_with('.') {
                assert!(table.contains_key(&k), "{file} calls the text {k}");
            }
        }
        for s in quoted(code) {
            let key_like = s.contains('.') && s.chars().all(|c| c.is_ascii_lowercase() || c == '.' || c == '_') && !s.ends_with('.') && !s.starts_with('.');
            if key_like {
                assert!(table.contains_key(&s), "{file} has the key-like text {s}");
            }
        }
    }
    // 이어 붙이는 키: 종류 이름과 고장 이름
    let kinds = twin_engine::scene::catalog()["kinds"].as_object().unwrap().keys().cloned().collect::<Vec<_>>();
    assert_eq!(kinds.len(), 27);
    for k in &kinds {
        assert!(table.contains_key(&format!("kind.{k}")), "kind.{k}");
    }
    for f in ["sensor_stuck_on", "sensor_stuck_off", "coil_dead", "air_off", "slow"] {
        assert!(table.contains_key(&format!("faults.{f}")), "faults.{f}");
    }
    assert!(table.keys().filter(|k| k.starts_with("kind.")).count() == 27, "no kind name is left over");
}

#[test]
fn korean_strings_hold_no_latin_letters_except_product_names_and_units() {
    let table = text_table(&ui("twin-model.html"));
    let allowed = ["model_sim", "model_draft", "twin-modeling-mcp", "ms", "mm", "s", "t", "AI"];
    for (k, [_, ko]) in &table {
        // `{값}` 자리는 뺀다
        let (mut plain, mut depth) = (String::new(), 0);
        for c in ko.chars() {
            match c {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ if depth == 0 => plain.push(c),
                _ => {}
            }
        }
        for run in plain.split(|c: char| !(c.is_ascii_alphabetic() || c == '_' || c == '-')).filter(|r| r.chars().any(|c| c.is_ascii_alphabetic())) {
            assert!(allowed.contains(&run), "{k}: Korean text has the Latin word {run}: {ko}");
        }
    }
}

#[test]
fn the_renderer_korean_texts_equal_the_korean_column_of_the_page_table() {
    // 렌더러의 기본값(plc-mcp 화면이 쓰는 한국어)과 공개 화면 표의 한국어 열은 같다: 한 곳이 바뀌면 이 시험이 알려 준다
    let table = text_table(&ui("twin-model.html"));
    let js = ui("twin-scene.js");
    let q = quoted(block(&js, "const KO_TEXT = {"));
    assert_eq!(q.len() % 2, 0);
    let mut n = 0;
    for c in q.chunks(2) {
        n += 1;
        if matches!(c[0].as_str(), "r.canvas" | "r.lamp.out" | "r.axis") {
            continue; // 공개 화면은 "공정" 대신 "모델", "주소" 대신 "핀"이라고 쓰고, PLC를 읽지 않으므로 축 글도 다르다 (설계 47의 4.1절, 리뷰 R3-22, R3-26)
        }
        let row = table.get(&c[0]).unwrap_or_else(|| panic!("the page table has no {}", c[0]));
        assert_eq!(row[1], c[1], "{}", c[0]);
    }
    assert_eq!(n, 15);
}

// 두 화면의 색 토큰이 같은지 보는 시험은 plc-mcp 쪽에 있다(`src/twin/store_tests.rs`): 이 크레이트에는 plc-mcp 화면 파일이 없다.
