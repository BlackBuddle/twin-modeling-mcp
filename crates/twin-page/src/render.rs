//! AI용 렌더(`twin_render`)와 사진(`twin_photo_get`). 계약은 `docs/twin-api.md` 12절.
//! 열린 3D 화면이 `render/next`를 1초쯤마다 묻고, 요청이 있으면 그리고 JPEG와 2D 상자를 올린다. PLC와 파일에 닿지 않는다.

use crate::store::{check_jpeg, shared, Res, Store};
use serde_json::{json, Value};
use std::sync::{Condvar, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

/// 도구가 답을 기다리는 시간의 상한
pub const WAIT: Duration = Duration::from_secs(10);
/// 화면이 이 시간 안에 묻지 않았으면 열려 있지 않은 것으로 본다
const PAGE_FRESH: Duration = Duration::from_secs(5);
const MAX_BOXES: usize = 200;
const MAX_PX: u64 = 1280;
const SLICE: Duration = Duration::from_millis(50);
const NOTE: &str = "Read-only. Does not touch the PLC. The boxes are from the 3D render, not from the photo.";

/// 결과의 `_images`는 `mcp.rs`의 `tool_result`가 허용 목록(`twin_render`, `twin_photo_get`)에서만 이미지 항목으로 바꾼다.

struct Req {
    id: u64,
    scene: String,
    photo: u32,
    camera: Value,
    devices: Vec<String>,
    jpeg: Option<Vec<u8>>,
    answer: Option<Value>,
    fail: Option<String>,
}

struct St {
    next_id: u64,
    pending: Option<Req>,
    last_poll: Option<Instant>,
}

pub struct RenderHub {
    st: Mutex<St>,
    cv: Condvar,
}

fn lk(m: &Mutex<St>) -> MutexGuard<'_, St> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn shared_hub() -> &'static RenderHub {
    static H: OnceLock<RenderHub> = OnceLock::new();
    H.get_or_init(RenderHub::new)
}

fn num01(v: &Value) -> Option<f64> {
    v.as_f64().filter(|n| (0.0..=1.0).contains(n))
}

impl RenderHub {
    pub fn new() -> RenderHub {
        RenderHub { st: Mutex::new(St { next_id: 1, pending: None, last_poll: None }), cv: Condvar::new() }
    }

    /// 화면이 묻는다: 기다리는 요청이 있으면 주고, 화면이 열려 있다고 적어 둔다.
    pub fn next(&self) -> Value {
        let mut g = lk(&self.st);
        g.last_poll = Some(Instant::now());
        match g.pending.as_ref().filter(|r| r.answer.is_none() && r.fail.is_none()) {
            Some(r) => json!({"request": {"id": r.id, "scene": r.scene, "photo": r.photo, "camera": r.camera, "max_px": MAX_PX}}),
            None => json!({"request": null}),
        }
    }

    fn with_req<T>(&self, id: u64, f: impl FnOnce(&mut Req) -> Res<T>) -> Res<T> {
        let mut g = lk(&self.st);
        let r = g.pending.as_mut().filter(|r| r.id == id && r.answer.is_none() && r.fail.is_none()).ok_or((404, "no such render request".to_string()))?;
        let out = f(r)?;
        drop(g);
        self.cv.notify_all();
        Ok(out)
    }

    pub fn image(&self, id: u64, jpeg: &[u8]) -> Res<Value> {
        check_jpeg(jpeg)?;
        self.with_req(id, |r| {
            r.jpeg = Some(jpeg.to_vec());
            Ok(json!({"ok": true}))
        })
    }

    pub fn answer(&self, id: u64, body: &Value) -> Res<Value> {
        let dim = |k: &str| body.get(k).and_then(Value::as_u64).filter(|n| (1..=MAX_PX).contains(n)).ok_or((400, format!("{k} must be an integer from 1 to {MAX_PX}")));
        let (w, h) = (dim("width")?, dim("height")?);
        let boxes = body.get("boxes").and_then(Value::as_array).filter(|a| a.len() <= MAX_BOXES).ok_or((400, format!("boxes must be an array of at most {MAX_BOXES}")))?;
        self.with_req(id, |r| {
            let mut out = Vec::with_capacity(boxes.len());
            for (i, b) in boxes.iter().enumerate() {
                let d = b.get("device").and_then(Value::as_str).filter(|d| r.devices.iter().any(|x| x == d)).ok_or((400, format!("boxes[{i}].device must be a device id of the scene")))?;
                let mut o = json!({"device": d});
                for k in ["x", "y", "w", "h"] {
                    o[k] = json!(b.get(k).and_then(num01).ok_or((400, format!("boxes[{i}].{k} must be a number from 0 to 1")))?);
                }
                out.push(o);
            }
            r.answer = Some(json!({"width": w, "height": h, "boxes": out}));
            Ok(json!({"ok": true}))
        })
    }

    pub fn fail(&self, id: u64, reason: &str) -> Res<Value> {
        self.with_req(id, |r| {
            r.fail = Some(reason.chars().filter(|c| !c.is_control()).take(80).collect());
            Ok(json!({"ok": true}))
        })
    }

    /// 요청을 올리고 답을 기다린다. `stop`이 참이 되면(취소, 서버 종료) 그만둔다.
    pub fn request(&self, scene: &str, photo: u32, camera: Value, devices: Vec<String>, stop: &dyn Fn() -> bool, wait: Duration) -> Result<Value, String> {
        let deadline = Instant::now() + wait;
        let mut g = lk(&self.st);
        if g.last_poll.is_none_or(|t| t.elapsed() > PAGE_FRESH) {
            return Err("open the 3D tab on the dashboard".into());
        }
        // 앞의 요청이 끝나길 기다린다
        while g.pending.is_some() {
            if stop() || Instant::now() >= deadline {
                return Err("another render request is still waiting".into());
            }
            g = self.cv.wait_timeout(g, SLICE).unwrap_or_else(|e| e.into_inner()).0;
        }
        let id = g.next_id;
        g.next_id += 1;
        g.pending = Some(Req { id, scene: scene.into(), photo, camera: camera.clone(), devices, jpeg: None, answer: None, fail: None });
        loop {
            if g.pending.as_ref().is_some_and(|r| r.answer.is_some() || r.fail.is_some()) {
                let Some(r) = g.pending.take() else { return Err("render request ended".into()) };
                drop(g);
                self.cv.notify_all();
                return match (r.fail, r.answer) {
                    (Some(f), _) => Err(format!("the 3D page could not render: {f}")),
                    (None, Some(mut a)) => {
                        a["scene"] = json!(scene);
                        a["photo"] = json!(photo);
                        a["camera"] = camera;
                        a["has_image"] = json!(r.jpeg.is_some());
                        a["note"] = json!(NOTE);
                        if let Some(j) = r.jpeg {
                            a["_images"] = json!([{"mime": "image/jpeg", "data": twin_engine::b64::encode(&j)}]);
                        }
                        Ok(a)
                    }
                    _ => Err("render request ended".into()),
                };
            }
            if stop() || Instant::now() >= deadline {
                g.pending = None;
                drop(g);
                self.cv.notify_all();
                return Err(if stop() { "the render request was cancelled".into() } else { "the 3D page did not answer in 10 s (keep the 3D tab visible)".into() });
            }
            g = self.cv.wait_timeout(g, SLICE).unwrap_or_else(|e| e.into_inner()).0;
        }
    }
}

fn photo_arg(args: &Value) -> Result<(&str, u32), String> {
    let o = args.as_object().ok_or("arguments must be an object")?;
    if let Some(k) = o.keys().find(|k| !matches!(k.as_str(), "scene" | "photo")) {
        return Err(format!("unknown argument {}", k.chars().take(20).collect::<String>()));
    }
    let scene = o.get("scene").and_then(Value::as_str).ok_or("scene must be a string")?;
    let photo = o.get("photo").and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok()).ok_or("photo must be a photo number (integer from 1)")?;
    Ok((scene, photo))
}

/// `twin_render`
pub fn call(args: &Value, stop: &dyn Fn() -> bool) -> Result<Value, String> {
    let st = shared().ok_or(NO_DIR)?;
    call_on(shared_hub(), &st, args, stop, WAIT)
}

const NO_DIR: &str = "scenes need the output folder (output_dir in the server settings)";

pub fn call_on(hub: &RenderHub, st: &Store, args: &Value, stop: &dyn Fn() -> bool, wait: Duration) -> Result<Value, String> {
    let (scene, photo) = photo_arg(args)?;
    let info = st.get(scene).map_err(|(_, m)| m)?;
    let ph = info["photos"].as_array().and_then(|a| a.iter().find(|p| p["photo"] == photo)).ok_or(format!("photo {photo} not found"))?;
    let camera = Some(ph["camera"].clone()).filter(|c| !c.is_null()).ok_or(format!("photo {photo} has no camera"))?;
    let devices = info["scene"]["devices"].as_array().map(|a| a.iter().filter_map(|d| d["id"].as_str().map(String::from)).collect()).unwrap_or_default();
    hub.request(scene, photo, camera, devices, stop, wait)
}

/// `twin_photo_get`: 표시와 카메라는 글로, 그려진 JPEG(없으면 원본)는 이미지 내용으로.
pub fn photo_get(args: &Value) -> Result<Value, String> {
    let st = shared().ok_or(NO_DIR.to_string())?;
    photo_value(&st, args)
}

/// `_images`을 아직 지우지 않은 값(시험이 본다).
pub fn photo_value(st: &Store, args: &Value) -> Result<Value, String> {
    let (scene, photo) = photo_arg(args)?;
    let (marks, camera, marked, jpeg) = st.photo_for_ai(scene, photo).map_err(|(_, m)| m)?;
    Ok(json!({
        "scene": scene, "photo": photo, "camera": camera, "marks": marks, "image": if marked { "marked" } else { "photo" },
        "_images": [{"mime": "image/jpeg", "data": twin_engine::b64::encode(&jpeg)}]
    }))
}
