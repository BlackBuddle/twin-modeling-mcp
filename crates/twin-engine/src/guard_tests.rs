//! 엔진의 경계 시험(설계 46의 3.8절): 엔진은 OS를 부르지 않는다(파일, 네트워크, 프로세스, 스레드, 시계, 난수).
//! 한계: 글 검사이다. 이 시험 파일은 파일을 읽으므로 검사에서 뺀다(`cfg(test)`라서 엔진의 실행 코드가 아니다). 금지 글자는 그래도 조각으로 적는다.
//! `HashMap`과 `HashSet`은 `mech.rs`에서만 막는다(옛 시험 `twin_mech_knows_no_plc_and_no_clock`): `scene.rs`와 `layout.rs`는 포함 검사에 `HashSet`을 쓴다.

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

#[test]
fn twin_engine_makes_no_os_call() {
    let banned: Vec<String> = [["std::", "fs"], ["std::", "net"], ["std::", "process"], ["std::", "thread"], ["Inst", "ant"], ["System", "Time"]].iter().map(|p| p.concat()).collect();
    let files = sources();
    assert!(files.len() >= 9, // lib, b64, bind, geom, layout, mech, plant, scene, text, work
"the test must see the source files: {}", files.len());
    for (name, text) in &files {
        for b in &banned {
            assert!(!text.contains(b.as_str()), "{name} contains {b}");
        }
        // 난수는 낱말 하나로 센다(`operand`, `random_access` 같은 글자는 걸리지 않는다)
        let word = ["ra", "nd"].concat();
        assert!(!text.split(|c: char| !(c.is_alphanumeric() || c == '_')).any(|w| w == word), "{name} contains the word {word}");
    }
}

#[test]
fn twin_engine_holds_no_plc_code() {
    // 엔진은 PLC의 주소 문법과 프로토콜을 모른다: 호스트가 `Addresses`로 준다
    let banned: Vec<String> = [["sl", "mp"], ["il", "::"], ["Tcp", "Stream"], ["Udp", "Socket"], ["Com", "mand::new"]].iter().map(|p| p.concat()).collect();
    for (name, text) in &sources() {
        for b in &banned {
            assert!(!text.contains(b.as_str()), "{name} contains {b}");
        }
    }
}
