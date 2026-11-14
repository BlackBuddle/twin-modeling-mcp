//! UTC 시각 글(순수한 계산, 시계를 읽지 않는다). 옛 `src/outdir.rs`에서 옮겼다(reusable-parts 3.1b): 장면 저장소의 역사 파일 이름이 쓴다.
//! `outdir`가 다시 내보내므로 plc-mcp의 호출 지점은 그대로이다.

/// 유닉스 시각(초)을 UTC (년, 월, 일, 시, 분, 초)로 만든다(Hinnant의 civil_from_days). 서버의 날짜 코드는 이것 하나이다.
pub fn civil(secs: u64) -> (i64, i64, i64, u64, u64, u64) {
    let (days, rem) = ((secs / 86400) as i64, secs % 86400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d, rem / 3600, rem % 3600 / 60, rem % 60)
}

/// UTC `yyyyMMdd-HHmmss`
pub fn utc_stamp(secs: u64) -> String {
    let (y, m, d, h, mi, s) = civil(secs);
    format!("{y:04}{m:02}{d:02}-{h:02}{mi:02}{s:02}")
}
