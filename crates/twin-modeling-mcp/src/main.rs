//! `twin-modeling-mcp` (reusable-parts 3.1d-2, 설계 46의 3절): 트윈 모델을 검사하고 시뮬레이션하고 그리는 서버. 표준 입출력의 MCP 서버와 127.0.0.1의 3D 화면이 한 프로세스에 있다.
//! PLC 코드가 없다(시험 `twin_crates_hold_no_plc_code`). 상주 프로세스가 아니다: 클라이언트가 시작하고 표준 입력이 끝나면 끝난다.

#[cfg(feature = "print")]
mod print;
mod rpc;
mod serve;
mod sim;
mod tools;

#[cfg(test)]
mod tests;

use std::path::PathBuf;

const USAGE: &str = "usage: twin-modeling-mcp [--dir <folder>] [--port <n>]   (stdio MCP server and the page)\n       twin-modeling-mcp --page [--dir <folder>] [--port <n>]   (the page only)\n       twin-modeling-mcp --version";

struct Args {
    dir: Option<PathBuf>,
    port: u16,
    page: bool,
}

/// `None`이면 `--version`이다. 환경 변수는 `USERPROFILE`과 `HOME`만 읽는다(기본 폴더).
fn parse_args(a: &[String]) -> Result<Option<Args>, String> {
    let mut r = Args { dir: None, port: 0, page: false };
    let mut it = a.iter();
    while let Some(x) = it.next() {
        match x.as_str() {
            "--version" if a.len() == 1 => return Ok(None),
            "--page" => r.page = true,
            "--dir" => r.dir = Some(PathBuf::from(it.next().ok_or(USAGE)?)),
            "--port" => {
                let n: u32 = it.next().and_then(|v| v.parse().ok()).ok_or("--port needs a number from 1024 to 65535, or 0")?;
                if n == 8765 {
                    return Err("port 8765 is the dashboard port of plc-mcp".into());
                }
                r.port = u16::try_from(n).ok().filter(|p| *p == 0 || *p >= 1024).ok_or("--port needs a number from 1024 to 65535, or 0")?;
            }
            _ => return Err(USAGE.into()),
        }
    }
    Ok(Some(r))
}

fn default_dir() -> Result<PathBuf, String> {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).ok_or("cannot find the home folder (USERPROFILE or HOME); give --dir")?;
    Ok(PathBuf::from(home).join("Documents").join("twin-models"))
}

fn run() -> Result<(), String> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let Some(args) = parse_args(&argv)? else {
        println!("twin-modeling-mcp {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    };
    let dir = match args.dir {
        Some(d) => d,
        None => default_dir()?,
    };
    let ctx = serve::start(dir.clone(), args.port)?;
    eprintln!("twin-modeling-mcp {}: page {} ; models {}", env!("CARGO_PKG_VERSION"), ctx.page_url(None), dir.display());
    if args.page {
        loop {
            std::thread::park();
        }
    }
    rpc::run(&ctx, std::io::BufReader::new(std::io::stdin()), std::io::stdout());
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(2);
    }
}
