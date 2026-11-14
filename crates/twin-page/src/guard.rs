//! 화면 코드의 마크업 가드가 쓰는 글 검사(reusable-parts 3.1e, R3-2). 이 크레이트의 시험과 plc-mcp의 화면 시험이 같은 함수를 쓰도록 여기 둔다. 호스트의 실행 코드는 이것을 부르지 않는다.

/// 화면 코드에서 마크업, 주소, 스타일, 글자 실행으로 이어지는 입구를 찾는다. 공백을 모두 지우고 따옴표(`"`, 백틱)를 `'`로 맞춘 뒤에 견주므로
/// 공백이나 따옴표 모양이 달라도 걸린다 (AR12, FR5). 남는 틈: 변수로 만든 속성 이름(`x.setAttribute(name, v)`), `Reflect`나 대괄호로 부른 메서드, 다른 파일에서 온 코드.
pub fn markup_sinks(src: &str) -> Vec<String> {
    let norm = |s: &str| s.chars().filter(|c| !c.is_whitespace()).map(|c| if c == '"' || c == '`' { '\'' } else { c }).collect::<String>();
    let mut out = Vec::new();
    for l in src.lines() {
        let n = norm(l);
        for w in [
            "innerHTML", "insertAdjacentHTML", "outerHTML", "document.write", "eval(", "newFunction", "Function(", "srcdoc", "cssText", "DOMParser", "createContextualFragment",
            ".href=", "setAttribute('on", "setAttribute('style", "setAttribute('href", "setAttribute('src", "setTimeout('",
            // `url(` 안에 변수를 이어 붙인 모양: 'url(' + x, `url(${x})`
            "url('+", "url(${", "url('${",
        ] {
            if n.contains(w) {
                out.push(format!("{w}: {}", l.trim()));
            }
        }
        // 주소를 넣는 `.src=`는 두 곳뿐이다: 서버에서 받은 사진의 blob 주소와 고정 라이브러리 주소 (장면의 값은 들어가지 않는다)
        if n.contains(".src=") && !(n.contains("photoUrl") || n.contains("'/vendor/")) {
            out.push(format!(".src=: {}", l.trim()));
        }
    }
    out
}
