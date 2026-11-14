//! 공개 3D 화면과 고정 라이브러리 목록(reusable-parts 3.1b, 3.1e). 이 크레이트의 화면 파일은 두 호스트가 같이 쓰는 코드와 공개 화면뿐이다:
//! plc-mcp 전용 화면(`twin.html`, 차트 uPlot)은 plc-mcp 쪽 `ui/`에 있다(사용자 결정 Q10: 공개 저장소에 plc-mcp 전용 화면 코드가 없다).
//! 화면 글과 라이브러리는 exe에 들어간다. `/vendor/<이름>`으로 내보내는 것은 이 목록의 파일뿐이고(F35), 목록에 없는 이름은 404다(호스트가 확인한다).

/// 화면 글 안의 토큰 자리 표시. 호스트가 요청마다 진짜 토큰으로 바꾼다.
pub const TOKEN_MARK: &str = "__DASHBOARD_TOKEN__";

/// `/vendor/<이름>`으로 내보내는 고정 목록(F35). exe에 넣은 이 파일들만 나간다. 목록에 없는 이름은 404다.
/// `TransformControls.js`는 공유 렌더러 `twin-scene.js`가 맨 위에서 가져오므로 나간다(렌더러가 이 파일 없이는 올라오지 않는다). 공개 화면에는 손잡이를 쓰는 코드가 없다.
pub const VENDOR: &[(&str, &str)] = &[
    ("three/three.module.js", include_str!("../ui/vendor/three/three.module.js")),
    ("three/three.core.js", include_str!("../ui/vendor/three/three.core.js")),
    ("three/controls/OrbitControls.js", include_str!("../ui/vendor/three/controls/OrbitControls.js")),
    ("three/controls/TransformControls.js", include_str!("../ui/vendor/three/controls/TransformControls.js")),
    ("twin-scene.js", include_str!("../ui/twin-scene.js")),
    // 두 화면이 같이 쓰는 함수 (3.1e)
    ("twin-core.js", include_str!("../ui/twin-core.js")),
];

/// 공개 화면(twin-modeling-mcp의 `GET /`, 3.1e): 영어와 한국어, 라이브·기록·측정·사진 코드 없음.
pub const MODEL_PAGE: &str = include_str!("../ui/twin-model.html");

/// 고정 목록의 파일을 이름으로 찾는다. 목록에 없는 이름은 `None`(호스트가 404를 준다).
pub fn vendor(name: &str) -> Option<&'static str> {
    VENDOR.iter().find(|(n, _)| *n == name).map(|(_, b)| *b)
}

/// 토큰을 넣은 공개 화면 글.
pub fn model_page(token: &str) -> String {
    MODEL_PAGE.replace(TOKEN_MARK, token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_public_page_takes_the_token_and_the_vendor_list_is_the_six_fixed_files() {
        assert!(MODEL_PAGE.contains(TOKEN_MARK) && MODEL_PAGE.contains("\"three\": \"/vendor/three/three.module.js\""));
        assert!(model_page("T0KEN").contains("T0KEN") && !model_page("T0KEN").contains(TOKEN_MARK));
        assert_eq!(VENDOR.len(), 6);
        assert!(VENDOR.iter().all(|(n, b)| !n.contains("..") && !n.starts_with('/') && !b.is_empty() && vendor(n).is_some()));
        // 차트와 plc-mcp 전용 화면은 이 크레이트에 없다
        for n in ["uplot/uPlot.iife.min.js", "uplot/uPlot.min.css", "../ui/twin.html", "three/LICENSE", ""] {
            assert!(vendor(n).is_none(), "{n}");
        }
    }
}
