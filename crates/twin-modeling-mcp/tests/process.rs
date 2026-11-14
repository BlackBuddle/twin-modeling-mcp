//! 프로세스 시험(설계 46의 3.2절, 3.1d의 시험 4): 시작 인자, 종료 코드, 표준 출력은 JSON-RPC 줄만.
//! 임시 모델 폴더와 빈 포트만 쓴다. plc-mcp의 대시보드 포트 8765는 열지 않는다(거절만 본다).

use std::io::Write;
use std::process::{Command, Stdio};

const EXE: &str = env!("CARGO_BIN_EXE_twin-modeling-mcp");

fn tmp(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("twin-mcp-proc-{tag}-{}", std::process::id()))
}

#[test]
fn port_8765_is_refused_with_exit_code_2() {
    let o = Command::new(EXE).args(["--port", "8765", "--dir"]).arg(tmp("p8765")).stdin(Stdio::null()).output().unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(String::from_utf8_lossy(&o.stderr).trim(), "port 8765 is the dashboard port of plc-mcp");
    assert!(o.stdout.is_empty());
}

#[test]
fn bad_arguments_exit_with_code_2_and_the_usage() {
    for args in [&["--nope"][..], &["--port", "80"], &["--port", "x"], &["--version", "--page"]] {
        let o = Command::new(EXE).args(args).stdin(Stdio::null()).output().unwrap();
        assert_eq!(o.status.code(), Some(2), "{args:?}");
        assert!(o.stdout.is_empty(), "{args:?}");
    }
    let o = Command::new(EXE).arg("--version").output().unwrap();
    assert!(o.status.success() && String::from_utf8_lossy(&o.stdout).starts_with("twin-modeling-mcp "));
}

#[test]
fn a_busy_port_exits_with_code_2() {
    let l = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = l.local_addr().unwrap().port().to_string();
    let o = Command::new(EXE).args(["--port", &port, "--dir"]).arg(tmp("busy")).stdin(Stdio::null()).output().unwrap();
    if port.parse::<u16>().unwrap() >= 1024 {
        assert_eq!(o.status.code(), Some(2));
        assert_eq!(String::from_utf8_lossy(&o.stderr).trim(), format!("port {port} is in use"));
    }
}

#[test]
fn stdout_holds_only_json_rpc_lines_and_stderr_names_the_page() {
    let mut c = Command::new(EXE).args(["--dir"]).arg(tmp("stdout")).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let mut i = c.stdin.take().unwrap();
    writeln!(i, "{}", r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#).unwrap();
    writeln!(i, "{}", r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#).unwrap();
    drop(i); // stdin ends: the process ends
    let o = c.wait_with_output().unwrap();
    assert!(o.status.success());
    let lines: Vec<&str> = std::str::from_utf8(&o.stdout).unwrap().lines().collect();
    assert_eq!(lines.len(), 2);
    for l in &lines {
        let v: serde_json::Value = serde_json::from_str(l).unwrap();
        assert_eq!(v["jsonrpc"], "2.0");
    }
    assert!(String::from_utf8_lossy(&o.stderr).starts_with("twin-modeling-mcp 0.1.0: page http://127.0.0.1:"));
}
