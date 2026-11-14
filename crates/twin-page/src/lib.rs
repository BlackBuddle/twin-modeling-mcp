//! 3D 공정 트윈의 화면 쪽(reusable-parts 3.1b, 설계 `docs/research/photo-twin/46-standalone-model-and-render-server.md` 4.3절).
//! 장면 저장소, AI용 렌더 허브, 장면 경로(`/api/twin/*`), 화면 파일과 고정 라이브러리 목록, 웹 검사와 토큰, 한국어 화면 문장 표.
//! PLC 코드가 없다(시험 `twin_page_holds_no_plc_code`). 호스트(plc-mcp, 나중의 twin-render)가 주소 규칙(`twin_engine::bind::Addresses`)과 폴더를 준다.

pub mod guard;
pub mod http;
pub mod ko;
pub mod page;
pub mod render;
pub mod stamp;
pub mod store;
pub mod web;

#[cfg(test)]
mod guard_tests;
#[cfg(test)]
mod page_tests;
