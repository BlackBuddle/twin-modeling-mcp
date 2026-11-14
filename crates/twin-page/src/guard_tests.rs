//! 화면 크레이트의 경계 시험(설계 46의 3.8절): `twin-page`에는 PLC, SLMP, FTP 코드가 없다. 글 검사이다. 이 시험 파일은 검사에서 뺀다.
//! 금지 글자는 그래도 조각으로 적는다.

use std::path::Path;

fn sources() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = Vec::new();
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_some_and(|x| x == "rs") && p.file_name().is_some_and(|n| n != "guard_tests.rs") {
            out.push((p.file_name().unwrap().to_string_lossy().into_owned(), std::fs::read_to_string(&p).unwrap()));
        }
    }
    out
}

/// 화면 파일: `ui/` 바로 아래의 `.html`과 `.js`(`ui/vendor/`는 남의 라이브러리라 뺀다). 새 화면 파일은 목록을 고치지 않아도 이 시험이 읽는다.
fn page_files() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("ui");
    let mut out: Vec<(String, String)> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "html" || x == "js"))
        .map(|p| (p.file_name().unwrap().to_string_lossy().into_owned(), std::fs::read_to_string(&p).unwrap()))
        .collect();
    out.sort();
    out
}

use crate::guard::markup_sinks;

#[test]
fn the_twin_pages_make_no_markup_from_data() {
    // 보호 10(마크업 부분): 화면 코드에 마크업·주소·스타일을 만드는 입구가 없다. 화면 크레이트의 시험이라 공개 저장소에도 따라간다(설계 46의 3.6절).
    // plc-mcp 전용 화면(`ui/twin.html`)은 plc-mcp 저장소의 `src/twin/store_tests.rs`가 같은 함수로 읽는다.
    let files = page_files();
    let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
    for must in ["twin-scene.js", "twin-core.js", "twin-model.html"] {
        assert!(names.contains(&must), "the guard must read {must}: {names:?}");
    }
    for (f, src) in &files {
        let found = markup_sinks(src);
        assert!(found.is_empty(), "ui/{f}: {found:?}");
    }
}

#[test]
fn the_page_crate_ui_has_no_plc_mcp_only_route_or_file() {
    // 사용자 결정 Q10: 공개 저장소에 plc-mcp 전용 화면 코드가 없다. 이 크레이트의 `ui/`에는 라이브·기록·측정·사진·쓰기·래더 경로와 차트가 없다.
    // 공개 화면이 부르는 `/api/ladder/sim/`(화면 시뮬레이션)과 `/api/twin/render/`는 두 호스트가 같이 가진 경로라 허용한다.
    let files = page_files();
    let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
    assert!(!names.contains(&"twin.html"), "the plc-mcp page is not in this crate: {names:?}");
    assert!(!Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/vendor/uplot").exists(), "the chart library is not in this crate");
    for (f, src) in &files {
        for bad in ["/api/twin/live", "/api/twin/photo", "/api/twin/marks", "/api/twin/marked", "/api/twin/camera", "/api/write", "/api/state", "uplot", "uPlot"] {
            assert!(!src.contains(bad), "ui/{f} has the plc-mcp only path {bad}");
        }
        for (i, _) in src.match_indices("/api/ladder/") {
            assert!(src[i + "/api/ladder/".len()..].starts_with("sim/"), "ui/{f} has the plc-mcp only ladder route: {}", &src[i..(i + 40).min(src.len())]);
        }
    }
}

#[test]
fn the_guard_of_the_twin_pages_catches_the_forms_that_once_passed() {
    // FR5: 시험의 시험. 장면의 값을 속성, 주소, 스타일로 넣는 모양은 따옴표나 공백이 달라도 걸린다.
    for bad in [
        "x.innerHTML = dev.label",
        "x.setAttribute('srcdoc', dev.label)",
        "location.href = dev.label",
        "x.setAttribute(\"onclick\", dev.label)",
        "x.setAttribute( `on` + k, v)",
        "x.src=dev.label",
        "x.style.backgroundImage = 'url(' + dev.label + ')'",
        "x.style.backgroundImage = `url(${dev.label})`",
        "x.style.background = \"url(\" + a",
        "x.style.cssText = a",
        "setTimeout(\"go()\", 1)",
    ] {
        assert!(!markup_sinks(bad).is_empty(), "not caught: {bad}");
    }
    // 오늘 화면의 두 `.src =`와 평범한 글 넣기는 걸리지 않는다
    for ok in ["$('pimg').src = photoUrl;", "s.src = '/vendor/uplot/uPlot.iife.min.js';", "e.textContent = dev.label", "i.setAttribute('aria-label', text)"] {
        assert!(markup_sinks(ok).is_empty(), "false alarm: {ok}");
    }
}

#[test]
fn the_tone_table_of_the_page_equals_the_catalog_tones() {
    // 화면(A5b)의 색조 이름표는 `catalog.tones`와 같은 이름을 같은 순서로 갖고, 이름마다 CSS 변수 `--sk-<이름>`이 공개 화면에 있다 (plc-mcp 화면은 plc-mcp 쪽 시험이 본다)
    let ui = Path::new(env!("CARGO_MANIFEST_DIR")).join("ui");
    let js = std::fs::read_to_string(ui.join("twin-scene.js")).unwrap();
    let line = js.lines().find(|l| l.contains("export const SKIN_TONES")).expect("SKIN_TONES");
    let list = &line[line.find('[').unwrap() + 1..line.find(']').unwrap()];
    let page: Vec<String> = list.split(',').map(|s| s.trim().trim_matches('\'').to_string()).collect();
    let cat: Vec<String> = twin_engine::scene::catalog()["tones"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    assert_eq!(page, cat);
    let html = std::fs::read_to_string(ui.join("twin-model.html")).unwrap();
    for t in &cat {
        assert!(html.contains(&format!("--sk-{t}:")), "ui/twin-model.html has no --sk-{t}");
    }
}

#[test]
fn twin_page_holds_no_plc_code() {
    let banned: Vec<String> = [["sl", "mp"], ["Sl", "mp"], ["SL", "MP"], ["ft", "p::"], ["F", "tp"], ["F", "TP"], ["qp", "g"], ["qp", "a"], ["qc", "d"], ["il", "::"], ["lin", "k::"], ["mel", "sec"], ["0x14", "01"], ["0x14", "02"], ["0x04", "01"], ["0x04", "03"], ["TcpStream::", "connect"], ["Udp", "Socket"], ["Com", "mand::new"]]
        .iter()
        .map(|p| p.concat())
        .collect();
    let files = sources();
    assert!(files.len() >= 4, "the test must see the source files: {}", files.len());
    for (name, text) in &files {
        for b in &banned {
            assert!(!text.contains(b.as_str()), "{name} contains {b}");
        }
    }
}
