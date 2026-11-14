//! 모델이 보낸 글의 글자 검사. 제어 문자, 서식·방향 문자, 공백처럼 보이는 글자를 거부하는 규칙이 한 곳에 있다(module-layers M2).
//! 초안 저장소(`ladder_draft`)와 장면 검사(`twin::scene`)가 같은 규칙을 쓴다.

/// 유니코드 범주 Cf(서식·방향 문자: 영폭 공백, 양방향 제어 `U+202E` 등). 화면에서 글자처럼 보이지 않고 줄 모양을 바꿀 수 있다.
/// shortcut: 표준 라이브러리에 범주 조회가 없어 범위 표로 둔다. 유니코드가 Cf를 늘리면 표를 고친다.
fn is_format(c: char) -> bool {
    matches!(c as u32,
        0xAD | 0x600..=0x605 | 0x61C | 0x6DD | 0x70F | 0x890..=0x891 | 0x8E2 | 0x180E | 0x200B..=0x200F | 0x202A..=0x202E | 0x2060..=0x2064
        | 0x2066..=0x206F | 0xFEFF | 0xFFF9..=0xFFFB | 0x110BD | 0x110CD | 0x13430..=0x1343F | 0x1BCA0..=0x1BCA3 | 0x1D173..=0x1D17A
        | 0xE0001 | 0xE0020..=0xE007F)
}

/// 공백이 아닌데 공백처럼 보이는 글자(한글 채움 문자 `U+115F` `U+1160` `U+3164` `U+FFA0`, 점자 빈 칸 `U+2800`). 유니코드 공백이 아니라
/// 공백 줄이기에 걸리지 않는다(LR26). 같은 이유로 거부하는 글자는 여기에 더한다.
fn looks_blank(c: char) -> bool {
    matches!(c as u32, 0x115F | 0x1160 | 0x2800 | 0x3164 | 0xFFA0)
}

/// 모델 글에서 거부하는 글자: 제어 문자, 서식·방향 문자(Cf), 공백처럼 보이는 글자.
pub fn refused_char(c: char) -> bool {
    c.is_control() || is_format(c) || looks_blank(c)
}
