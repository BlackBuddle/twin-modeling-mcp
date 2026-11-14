//! `/api/twin/*` 경로(작업 1.2, `/api/twin/live/*` 제외). 토큰과 Origin 검사는 dashboard.rs가 이미 했다.
//! 답은 (HTTP 코드, 내용 종류, 바이트)이고 경로를 싣지 않는다.

use crate::store::{shared, Res, Store, MAX_JPEG, MAX_JSON};
use serde_json::{json, Value};
use std::sync::Arc;

pub struct Hub {
    store: Option<Arc<Store>>,
}

pub type Reply = (u16, &'static str, Vec<u8>);

const JSON: &str = "application/json; charset=utf-8";

fn jr(code: u16, v: &Value) -> Reply {
    (code, JSON, v.to_string().into_bytes())
}

fn q<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    query.split('&').filter_map(|p| p.split_once('=')).find(|(k, _)| *k == key).map(|(_, v)| v)
}

fn body_json(body: &[u8]) -> Res<Value> {
    if body.len() > MAX_JSON {
        return Err((413, "body is larger than 262144 bytes".into()));
    }
    serde_json::from_slice(body).map_err(|_| (400, "body is not valid JSON".into()))
}

fn text<'a>(v: &'a Value, k: &str) -> Res<&'a str> {
    v.get(k).and_then(Value::as_str).ok_or((400, format!("{k} must be a string")))
}

impl Hub {
    pub fn new() -> Hub {
        Hub { store: None }
    }

    /// 저장소를 직접 주는 만들기(시험과 호스트의 임시 저장소).
    pub fn with_store(s: Arc<Store>) -> Hub {
        Hub { store: Some(s) }
    }

    pub fn route(&self, method: &str, path: &str, query: &str, body: &[u8]) -> Reply {
        let store = match self.store.clone().or_else(shared) {
            Some(s) => s,
            None => return jr(409, &json!({"error": "scenes need the output folder (output_dir in the server settings)"})),
        };
        match self.run(&store, method, path, query, body) {
            Ok(r) => r,
            Err((c, m)) => jr(c, &json!({"error": m})),
        }
    }

    fn run(&self, st: &Store, method: &str, path: &str, query: &str, body: &[u8]) -> Res<Reply> {
        let scene = || q(query, "scene").ok_or((400, "scene is needed".to_string()));
        let num = || q(query, "n").and_then(|n| n.parse::<u32>().ok()).ok_or((400, "n must be a photo number".to_string()));
        let rid = || q(query, "id").and_then(|n| n.parse::<u64>().ok()).ok_or((400, "id must be a render request number".to_string()));
        let ok = |v: Value| Ok(jr(200, &v));
        // 본문 한도: JPEG 1 MiB, JSON 256 KiB(dashboard.rs가 읽을 때도 같은 한도를 건다).
        if body.len() > MAX_JPEG {
            return Err((413, "body is too large".into()));
        }
        match (method, path) {
            ("GET", "/api/twin/auto") => ok(json!({"on": st.auto_apply()})),
            ("POST", "/api/twin/auto") => {
                let v = body_json(body)?;
                let on = v.get("on").and_then(Value::as_bool).ok_or((400, "on must be true or false".to_string()))?;
                st.set_auto_apply(on);
                ok(json!({"on": on}))
            }
            // 화면이 장치 크기·부품·포트를 그릴 때 쓰는 종류 목록(MCP `twin_catalog`의 catalog와 같은 값).
            ("GET", "/api/twin/catalog") => ok(twin_engine::scene::catalog().clone()),
            ("GET", "/api/twin/scenes") => ok(st.list()?),
            ("GET", "/api/twin/scene") => ok(st.get_page(scene()?)?),
            ("PUT", "/api/twin/scene") => {
                let v = body_json(body)?;
                ok(st.put(text(&v, "scene")?, v.get("data").ok_or((400, "data is needed".to_string()))?)?)
            }
            // 화면에서 고친 것(주소, 위치, 측정값)도 초안으로 간다. 자동 적용이 켜져 있으면 모델 초안과 같이 바로 장면이 된다.
            ("PUT", "/api/twin/draft") => {
                let v = body_json(body)?;
                ok(st.draft(text(&v, "scene")?, v.get("data").ok_or((400, "data is needed".to_string()))?)?)
            }
            ("DELETE", "/api/twin/draft") => ok(st.discard(scene()?)?),
            ("POST", "/api/twin/confirm") => ok(st.confirm(text(&body_json(body)?, "scene")?)?),
            // 화면의 "외형 확인": 사람이 본 장치의 외형 표시를 checked로 한다(토큰과 Origin은 dashboard.rs가 본다; 대화상자 없음)
            ("POST", "/api/twin/skin/check") => {
                let v = body_json(body)?;
                let ids: Vec<String> = v.get("devices").and_then(Value::as_array).filter(|a| a.len() <= 200).ok_or((400, "devices must be an array of at most 200 ids".to_string()))?.iter().map(|x| x.as_str().map(String::from).ok_or((400, "devices must be an array of ids".to_string()))).collect::<Result<_, _>>()?;
                let checked = v.get("checked").map_or(Ok(true), |c| c.as_bool().ok_or((400, "checked must be true or false".to_string())))?;
                ok(st.skin_check(text(&v, "scene")?, &ids, checked)?)
            }
            ("POST", "/api/twin/new") => ok(st.new_scene(text(&body_json(body)?, "scene")?)?),
            ("POST", "/api/twin/photo") => ok(st.photo_add(scene()?, body)?),
            ("GET", "/api/twin/photo") => Ok((200, "image/jpeg", st.photo_get(scene()?, num()?, q(query, "marked") == Some("1"))?)),
            ("POST", "/api/twin/marks") => ok(st.marks_set(scene()?, num()?, &body_json(body)?)?),
            ("POST", "/api/twin/marked") => ok(st.marked_set(scene()?, num()?, body)?),
            ("PUT", "/api/twin/camera") => {
                let v = body_json(body)?;
                let n = v.get("n").and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok()).ok_or((400, "n must be a photo number".to_string()))?;
                ok(st.camera_set(text(&v, "scene")?, n, v.get("camera").ok_or((400, "camera is needed".to_string()))?)?)
            }
            // AI용 렌더(계약 12절): 화면이 묻고, JPEG와 상자를 올린다
            ("GET", "/api/twin/render/next") => ok(crate::render::shared_hub().next()),
            ("POST", "/api/twin/render/image") => ok(crate::render::shared_hub().image(rid()?, body)?),
            ("POST", "/api/twin/render/answer") => ok(crate::render::shared_hub().answer(rid()?, &body_json(body)?)?),
            ("POST", "/api/twin/render/fail") => ok(crate::render::shared_hub().fail(rid()?, text(&body_json(body)?, "reason")?)?),
            (_, "/api/twin/render/next" | "/api/twin/render/image" | "/api/twin/render/answer" | "/api/twin/render/fail") => Err((405, "method not allowed".into())),
            (_, "/api/twin/skin/check" | "/api/twin/auto" |"/api/twin/catalog" | "/api/twin/scenes" | "/api/twin/scene" | "/api/twin/draft" | "/api/twin/confirm" | "/api/twin/new" | "/api/twin/photo" | "/api/twin/marks" | "/api/twin/marked" | "/api/twin/camera") => Err((405, "method not allowed".into())),
            _ => Err((404, "not found".into())),
        }
    }
}
