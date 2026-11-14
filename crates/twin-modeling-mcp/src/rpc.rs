//! stdio JSON-RPC 고리(reusable-parts 3.1d-2, 설계 46의 3.3절): `initialize`, `ping`, `tools/list`, `tools/call`과 알림.
//! 표준 출력에는 JSON-RPC 줄만 나간다. plc-mcp의 릴레이와 달리 세션도 쓰기 속도 제한도 없다(쓸 PLC가 없다).

use crate::serve::Ctx;
use crate::tools;
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};

/// plc-mcp가 답하는 프로토콜 버전. 요청한 버전이 이 목록에 있으면 그것, 아니면 가장 새 버전을 답한다.
pub const SUPPORTED_VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];
/// 이미지 내용을 붙일 수 있는 도구(허용 목록)
const IMAGE_TOOLS: &[&str] = &["model_render"];
const MAX_IMAGES: usize = 4;
const MAX_IMAGE_B64: usize = 2 * 1024 * 1024;

fn instructions(port: u16) -> String {
    format!("Twin model render server. It has no PLC. Call model_catalog first and follow guide.modeling. The 3D page for the person: http://127.0.0.1:{port}/")
}

fn initialize_result(params: &Value, port: u16) -> Value {
    let asked = params.get("protocolVersion").and_then(Value::as_str);
    let v = SUPPORTED_VERSIONS.iter().copied().find(|v| Some(*v) == asked).unwrap_or(SUPPORTED_VERSIONS[0]);
    json!({"protocolVersion": v, "capabilities": {"tools": {}}, "serverInfo": {"name": "twin-modeling-mcp", "version": env!("CARGO_PKG_VERSION")}, "instructions": instructions(port)})
}

/// 도구 호출 결과: 글 항목 하나(JSON). 허용 목록의 도구는 `_images`(`{mime, data}` 배열)를 글에서 빼고 이미지 항목으로 붙인다(4장, base64 2 MiB까지).
pub fn tool_result(name: &str, r: Result<Value, String>) -> Value {
    match r {
        Ok(mut v) => {
            let mut images = Vec::new();
            if IMAGE_TOOLS.contains(&name) {
                if let Some(list) = v.as_object_mut().and_then(|o| o.remove("_images")) {
                    let list = list.as_array().cloned().unwrap_or_default();
                    if list.len() > MAX_IMAGES {
                        return tool_result("", Err(format!("too many images in the result ({} > {MAX_IMAGES})", list.len())));
                    }
                    if list.iter().map(|i| i["data"].as_str().map_or(0, str::len)).sum::<usize>() > MAX_IMAGE_B64 {
                        return tool_result("", Err(format!("images are larger than {MAX_IMAGE_B64} bytes")));
                    }
                    images = list.iter().filter_map(|i| Some(json!({"type": "image", "data": i["data"].as_str()?, "mimeType": i["mime"].as_str()?}))).collect();
                }
            }
            let mut res = json!({"content": [{"type": "text", "text": v.to_string()}]});
            if let Some(c) = res["content"].as_array_mut() {
                c.extend(images);
            }
            if v.is_object() {
                res["structuredContent"] = v;
            }
            res
        }
        Err(e) => json!({"content": [{"type": "text", "text": e}], "isError": true}),
    }
}

fn ok(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn err(id: Value, code: i64, m: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": m}})
}

/// 요청 하나를 처리한다. 알림과 취소된 요청에는 답이 없다(None).
fn handle(ctx: &Ctx, v: &Value, cancel: &AtomicBool) -> Option<Value> {
    let id = v.get("id")?.clone();
    let params = v.get("params").cloned().unwrap_or(Value::Null);
    Some(match v.get("method").and_then(Value::as_str) {
        Some("initialize") => ok(id, initialize_result(&params, ctx.port)),
        Some("ping") => ok(id, json!({})),
        Some("tools/list") => ok(id, json!({"tools": tools::definitions()})),
        Some("tools/call") => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            if !tools::is_tool(name) {
                return Some(err(id, -32602, "unknown tool"));
            }
            let r = tools::call(ctx, name, params.get("arguments").unwrap_or(&json!({})), &|| cancel.load(Ordering::SeqCst));
            // 취소된 요청에는 답을 보내지 않는다
            if cancel.load(Ordering::SeqCst) {
                return None;
            }
            ok(id, tool_result(name, r))
        }
        Some(_) => err(id, -32601, "method not found"),
        None => err(id, -32600, "invalid request"),
    })
}

/// 입력 줄마다 답 한 줄을 쓴다. 읽는 스레드가 따로 있어, 요청이 도는 동안 `notifications/cancelled`가 와도 본다.
pub fn run<R: BufRead + Send + 'static, W: Write>(ctx: &Ctx, input: R, mut out: W) {
    let current: Arc<Mutex<Option<Value>>> = Arc::new(Mutex::new(None));
    let cancel = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel::<Value>();
    let (cur2, cancel2) = (current.clone(), cancel.clone());
    std::thread::spawn(move || {
        // 바이트로 읽는다: UTF-8이 아닌 줄 하나가 읽기를 끝내지 않고 구문 오류 답을 받는다(R3-14)
        for line in input.split(b'\n').map_while(Result::ok).map(|l| String::from_utf8_lossy(&l).into_owned()).filter(|l| !l.trim().is_empty()) {
            let v: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
            if v["method"] == "notifications/cancelled" {
                if cur2.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|c| *c == v["params"]["requestId"]) {
                    cancel2.store(true, Ordering::SeqCst);
                }
                continue;
            }
            if tx.send(v).is_err() {
                break;
            }
        }
    });
    for v in rx {
        if v.is_null() {
            let _ = writeln!(out, "{}", err(Value::Null, -32700, "parse error"));
            let _ = out.flush();
            continue;
        }
        // 취소 표시를 먼저 지운다: 새 요청의 id를 올린 뒤에 지우면 그 사이에 온 취소가 사라진다(R3-8)
        cancel.store(false, Ordering::SeqCst);
        *current.lock().unwrap_or_else(|e| e.into_inner()) = v.get("id").cloned();
        if let Some(r) = handle(ctx, &v, &cancel) {
            let _ = writeln!(out, "{r}");
            let _ = out.flush();
        }
    }
}
