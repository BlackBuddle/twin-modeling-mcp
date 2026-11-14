//! 127.0.0.1의 HTTP 수신부(reusable-parts 3.1d-2, 설계 46의 3.4·3.6절): 3D 화면과 그 경로. 토큰, `Host`, `Origin` 검사는 `twin_page::web`이다.
//! 바깥으로 나가는 연결이 없다. 요청마다 스레드 하나(동시 연결 32개까지). 화면 문장은 머리글 `X-Page-Lang: ko`일 때만 한국어 표(`twin_page::ko`)를 지난다.

use crate::sim::Sims;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use twin_engine::bind::NO_ADDRESSES;
use twin_page::http::Hub;
use twin_page::page::{model_page, vendor};
use twin_page::store::{Store, MAX_JPEG, MAX_JSON};
use twin_page::web::{host_ok, make_token, origin_ok};

const MAX_CONNS: usize = 32;
const MAX_HEAD: usize = 16 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(10);

/// 서버 전체가 나누어 쓰는 것: 화면이 보는 저장소, 허브, 시뮬레이션들, 토큰, 포트
pub struct Ctx {
    pub port: u16,
    token: String,
    pub store: Arc<Store>,
    hub: Hub,
    pub sims: Sims,
    conns: AtomicUsize,
}

impl Ctx {
    #[cfg(test)]
    pub fn token(&self) -> &str {
        &self.token
    }

    /// 화면의 주소(모델 이름이 있으면 그 모델을 연다)
    pub fn page_url(&self, name: Option<&str>) -> String {
        match name {
            Some(n) => format!("http://127.0.0.1:{}/?scene={n}", self.port),
            None => format!("http://127.0.0.1:{}/", self.port),
        }
    }
}

/// 서버를 세운다: 모델 폴더를 만들고, `127.0.0.1:<port>`(0이면 빈 포트)를 열고, 받는 스레드를 시작한다.
pub fn start(dir: PathBuf, port: u16) -> Result<Arc<Ctx>, String> {
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot make the model folder {}: {e}", dir.display()))?;
    let l = TcpListener::bind(("127.0.0.1", port)).map_err(|_| format!("port {port} is in use"))?;
    let port = l.local_addr().map_err(|e| e.to_string())?.port();
    let store = Arc::new(Store::new(dir, &NO_ADDRESSES));
    let ctx = Arc::new(Ctx { port, token: make_token()?, hub: Hub::with_store(store.clone()), store, sims: Sims::default(), conns: AtomicUsize::new(0) });
    let c = ctx.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            if c.conns.fetch_add(1, Ordering::SeqCst) >= MAX_CONNS {
                c.conns.fetch_sub(1, Ordering::SeqCst);
                continue; // 32개를 넘는 연결은 그냥 닫는다
            }
            let c2 = c.clone();
            std::thread::spawn(move || {
                handle(&c2, s);
                c2.conns.fetch_sub(1, Ordering::SeqCst);
            });
        }
    });
    Ok(ctx)
}

struct Req {
    method: String,
    path: String,
    query: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Req {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}

/// 길이가 같으면 모든 바이트를 비교해 시간으로 새지 않게 한다.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0, |d, (x, y)| d | (x ^ y)) == 0
}

fn read_request(s: &mut TcpStream) -> Result<Req, u16> {
    let end = Instant::now() + IO_TIMEOUT;
    let (mut buf, mut chunk) = (Vec::new(), [0u8; 2048]);
    let mut fill = |s: &mut TcpStream, buf: &mut Vec<u8>| -> Result<(), u16> {
        let left = end.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(408);
        }
        s.set_read_timeout(Some(left)).map_err(|_| 400u16)?;
        match s.read(&mut chunk) {
            Ok(0) | Err(_) => Err(400),
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                Ok(())
            }
        }
    };
    let head_end = loop {
        if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break p;
        }
        if buf.len() > MAX_HEAD {
            return Err(431);
        }
        fill(s, &mut buf)?;
    };
    // 읽은 덩어리(2 KiB씩)가 끝 표시를 한 번에 넘길 수 있으므로 머리글의 길이를 끝을 찾은 뒤에도 본다(R3-7)
    if head_end > MAX_HEAD {
        return Err(431);
    }
    let head = String::from_utf8(buf[..head_end].to_vec()).map_err(|_| 400u16)?;
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or("").split(' ');
    let (method, target, version) = (first.next().unwrap_or(""), first.next().unwrap_or(""), first.next().unwrap_or(""));
    if method.is_empty() || !target.starts_with('/') || !version.starts_with("HTTP/1.") {
        return Err(400);
    }
    let headers: Vec<(String, String)> = lines.filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect();
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let mut req = Req { method: method.into(), path: path.into(), query: query.into(), headers, body: Vec::new() };
    if req.header("transfer-encoding").is_some() {
        return Err(411);
    }
    let len = req.header("content-length").map_or(Ok(0), |v| v.parse::<usize>().map_err(|_| 400u16))?;
    // 본문 한도: 렌더 JPEG 1 MiB, 그 밖의 JSON 256 KiB
    if len > if req.path == "/api/twin/render/image" { MAX_JPEG } else { MAX_JSON } {
        return Err(413);
    }
    let mut body = buf[head_end + 4..].to_vec();
    while body.len() < len {
        fill(s, &mut body)?;
    }
    body.truncate(len);
    req.body = body;
    Ok(req)
}

fn reason(code: u16) -> &'static str {
    match code {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        409 => "Conflict",
        411 => "Length Required",
        413 => "Payload Too Large",
        431 => "Request Header Fields Too Large",
        _ => "Bad Gateway",
    }
}

/// 모든 답에 같은 보호 머리글을 붙인다(캐시 금지, 틀 삽입 금지, 형식 추측 금지). `lang`은 API 답에만 붙는다.
fn send(s: &mut TcpStream, code: u16, ctype: &str, body: &[u8], lang: Option<&str>) {
    let cl = lang.map_or(String::new(), |l| format!("Content-Language: {l}\r\n"));
    // 화면 글에는 두 번째 방어선을 더한다(R3-25): 화면은 토큰을 갖고 있으므로 같은 출처의 글과 연결만 허용한다. 화면은 인라인 스크립트(importmap)와 스타일을 쓴다.
    let csp = if ctype.starts_with("text/html") {
        "frame-ancestors 'none'; default-src 'none'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; connect-src 'self'; img-src 'self' data: blob:"
    } else {
        "frame-ancestors 'none'"
    };
    let h = format!(
        "HTTP/1.1 {code} {}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\n{cl}Cache-Control: no-store\r\nX-Frame-Options: DENY\r\nContent-Security-Policy: {csp}\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
        reason(code),
        body.len()
    );
    let _ = s.write_all(h.as_bytes());
    let _ = s.write_all(body);
}

const JSON: &str = "application/json; charset=utf-8";

/// 화면으로 가는 JSON: 머리글이 `ko`일 때만 한국어 표를 지난다.
fn send_json(s: &mut TcpStream, code: u16, v: &Value, ko: bool) {
    let v = if ko { twin_page::ko::value_in(v, &[twin_page::ko::KO]) } else { v.clone() };
    send(s, code, JSON, v.to_string().as_bytes(), Some(if ko { "ko" } else { "en" }));
}

fn handle(ctx: &Ctx, mut s: TcpStream) {
    let _ = s.set_write_timeout(Some(IO_TIMEOUT));
    let req = match read_request(&mut s) {
        Ok(r) => r,
        Err(code) => {
            send_json(&mut s, code, &json!({"error": reason(code)}), false);
            // 다 읽지 않은 요청이 남은 채 닫으면 상대가 답 대신 연결 재설정을 받을 수 있다. 조금 비우고 닫는다.
            let _ = s.shutdown(std::net::Shutdown::Write);
            let _ = s.set_read_timeout(Some(Duration::from_millis(100)));
            let mut sink = [0u8; 4096];
            for _ in 0..16 {
                if s.read(&mut sink).is_ok_and(|n| n > 0) {
                    continue;
                }
                break;
            }
            return;
        }
    };
    let ko = req.header("x-page-lang") == Some("ko");
    // 어떤 경로든 일을 하기 전에 Host부터 본다(DNS rebinding)
    if !host_ok(req.header("host"), ctx.port) {
        return send_json(&mut s, 403, &json!({"error": "forbidden"}), false);
    }
    let (m, p) = (req.method.as_str(), req.path.as_str());
    if m == "GET" && (p == "/" || p == "/twin") {
        return send(&mut s, 200, "text/html; charset=utf-8", model_page(&ctx.token).as_bytes(), None);
    }
    if m == "GET" && p.starts_with("/vendor/") {
        // 고정 목록 6개와 글자 그대로 같은 이름만 준다(TW201: 차트 파일은 이 서버에 없다). 경로를 풀거나 파일을 읽지 않으므로 `..`은 그냥 없는 이름이다.
        return match vendor(&p["/vendor/".len()..]) {
            Some(b) => send(&mut s, 200, "text/javascript; charset=utf-8", b.as_bytes(), None),
            None => send_json(&mut s, 404, &json!({"error": "not found"}), false),
        };
    }
    if !p.starts_with("/api/") {
        return send_json(&mut s, 404, &json!({"error": "not found"}), false);
    }
    // 모든 API: 토큰, 그리고 쓰는 방법은 Origin도
    if !req.header("x-dashboard-token").is_some_and(|t| same(t, &ctx.token)) || (m != "GET" && !origin_ok(req.header("origin"), ctx.port)) {
        return send_json(&mut s, 403, &json!({"error": "forbidden"}), false);
    }
    let (code, v) = api(ctx, &req);
    send_json(&mut s, code, &v, ko);
}

fn api(ctx: &Ctx, r: &Req) -> (u16, Value) {
    let (m, p, q) = (r.method.as_str(), r.path.as_str(), r.query.as_str());
    if let Some(x) = ctx.sims.route(&ctx.store, m, p, q, &r.body) {
        return x;
    }
    if p.starts_with("/api/twin/live/") {
        return (404, json!({"error": "not found"}));
    }
    // 사진은 사람의 파일이다: 트윈 모델은 사진을 두지 않는다
    if matches!(p, "/api/twin/photo" | "/api/twin/marks" | "/api/twin/marked" | "/api/twin/camera") {
        return (404, json!({"error": "photos are not part of a twin model"}));
    }
    if p == "/api/twin/sheet" {
        let Some(name) = q.split('&').find_map(|x| x.strip_prefix("scene=")) else { return (400, json!({"error": "scene is needed"})) };
        if m != "GET" {
            return (405, json!({"error": "method not allowed"}));
        }
        return match ctx.store.sheet_get(name) {
            Ok(t) => (200, json!({"sheet": t})),
            Err((c, e)) => (c, json!({"error": e})),
        };
    }
    if p.starts_with("/api/twin/") {
        let (code, ctype, bytes) = ctx.hub.route(m, p, q, &r.body);
        if ctype.starts_with("application/json") {
            if let Ok(v) = serde_json::from_slice(&bytes) {
                return (code, v);
            }
        }
        // JPEG와 그 밖의 본문은 이 서버가 내지 않는다(사진 경로가 막혀 있다)
        return (code, json!({"error": "unexpected reply"}));
    }
    (404, json!({"error": "not found"}))
}
