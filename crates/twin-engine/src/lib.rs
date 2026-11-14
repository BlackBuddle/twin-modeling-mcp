//! 3D 공정 트윈의 모델 층(reusable-parts 3.1a, 설계 `docs/research/photo-twin/46-standalone-model-and-render-server.md` 4.2절).
//! 장면 형식과 검사, 기하, 기계 모델 엔진, 공작물 규칙, 설비 모델. OS 호출과 PLC 코드가 없다(시험 `twin_engine_makes_no_os_call`).
//! PLC에 닿는 자리는 `bind::Addresses` 트레이트 뒤에 있고, 호스트(plc-mcp)가 구현을 준다.

pub mod b64;
pub mod bind;
pub mod geom;
pub mod layout;
pub mod mech;
pub mod model;
pub mod plant;
pub mod run;
pub mod scene;
pub mod text;
pub mod work;

#[cfg(test)]
mod guard_tests;
