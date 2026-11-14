//! 화면 서버(호스트의 HTTP 수신부)가 쓰는 검사와 토큰. 옛 `src/dashboard.rs`에서 옮겼다(reusable-parts 3.1b, 설계 46의 4.3절). 규칙은 바뀌지 않았다.
//! 호스트(대시보드, 나중의 twin-render)는 포트를 주고 이 함수들로 `Host`와 `Origin` 머리글을 검사하며 토큰을 만든다.

#[cfg(windows)]
#[link(name = "bcrypt")]
extern "system" {
    fn BCryptGenRandom(alg: *mut core::ffi::c_void, buf: *mut u8, len: u32, flags: u32) -> i32;
}

/// 운영체제 난수로 채운다. Windows는 BCryptGenRandom(시스템 기본 RNG, 플래그 2), 그 밖은 /dev/urandom.
pub fn fill_random(buf: &mut [u8]) -> Result<(), String> {
    #[cfg(windows)]
    {
        // SAFETY: buf는 유효한 쓰기 가능 메모리이고 길이가 맞다. 알고리즘 핸들은 null(플래그 2가 시스템 RNG를 고른다).
        let st = unsafe { BCryptGenRandom(std::ptr::null_mut(), buf.as_mut_ptr(), buf.len() as u32, 2) };
        if st == 0 { Ok(()) } else { Err(format!("BCryptGenRandom failed: {st:#x}")) }
    }
    #[cfg(not(windows))]
    {
        use std::io::Read;
        std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(buf)).map_err(|e| e.to_string())
    }
}

/// 토큰: OS 난수 32바이트(256비트)를 16진 64글자로 만든다.
pub fn make_token() -> Result<String, String> {
    let mut b = [0u8; 32];
    fill_random(&mut b).map_err(|e| format!("dashboard: cannot make a token: {e}"))?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

/// Host는 127.0.0.1:<port> 또는 localhost:<port>만 받는다(DNS rebinding 방지).
pub fn host_ok(h: Option<&str>, port: u16) -> bool {
    h.is_some_and(|h| h.eq_ignore_ascii_case(&format!("127.0.0.1:{port}")) || h.eq_ignore_ascii_case(&format!("localhost:{port}")))
}

/// Origin은 이 서버 자신의 주소(`http://127.0.0.1:<port>` 또는 `http://localhost:<port>`)만 받는다.
pub fn origin_ok(o: Option<&str>, port: u16) -> bool {
    o.is_some_and(|o| o.eq_ignore_ascii_case(&format!("http://127.0.0.1:{port}")) || o.eq_ignore_ascii_case(&format!("http://localhost:{port}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_is_64_hex_characters_and_differs_each_time() {
        let (a, b) = (make_token().unwrap(), make_token().unwrap());
        assert_eq!(a.len(), 64);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn host_and_origin_accept_only_the_own_address_with_the_own_port() {
        assert!(host_ok(Some("127.0.0.1:8123"), 8123) && host_ok(Some("LocalHost:8123"), 8123));
        assert!(!host_ok(Some("127.0.0.1:8124"), 8123) && !host_ok(Some("evil.example"), 8123) && !host_ok(Some("127.0.0.1"), 8123) && !host_ok(None, 8123));
        assert!(origin_ok(Some("http://127.0.0.1:8123"), 8123) && origin_ok(Some("http://localhost:8123"), 8123));
        assert!(!origin_ok(Some("http://evil.example"), 8123) && !origin_ok(Some("https://127.0.0.1:8123"), 8123) && !origin_ok(None, 8123));
    }
}
