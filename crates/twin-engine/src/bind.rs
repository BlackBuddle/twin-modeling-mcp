//! 엔진과 호스트의 경계(설계 46의 2.6절). 엔진은 PLC 주소의 문법을 모른다: 주소를 읽고 검사하고 이어 붙이는 일은
//! `Addresses`를 구현한 호스트(plc-mcp: `src/twin/bind.rs`)가 한다. 엔진은 정규화한 이름(`X1`, `Y3A`, `D100`)만 글로 다룬다.

use serde_json::Value;
use std::collections::BTreeMap;

/// 장치 이름(정규화한 `X1`, `Y3A`, `M300`, `D100`)으로 비트와 워드를 읽고 쓰는 작은 입출력.
pub trait PlantIo {
    fn bit(&self, name: &str) -> bool;
    fn set_bit(&mut self, name: &str, v: bool);
    fn set_word(&mut self, name: &str, v: i64);
    /// 워드 읽기(대리 축의 지령 단어). 읽지 못하는 입출력은 0이다.
    fn word(&self, _name: &str) -> i64 {
        0
    }
}

/// 호스트가 주는 주소 규칙. 오류 글은 호스트의 글 그대로 장면 검사 글에 들어간다.
pub trait Addresses: Sync + std::fmt::Debug {
    /// `io` 칸 값이나 `signals[].device`: 정규화한 이름, 또는 `device <id>: <field>: ` 뒤에 붙을 오류 글.
    /// 비트는 `out`이면 Y나 M, 아니면 X나 M이고 워드는 D 장치이다. 너무 긴 글도 여기서 거절한다.
    fn check(&self, text: &str, out: bool, word: bool) -> Result<String, String>;
    /// 외형의 `show`·`hide`: 비트 주소 하나의 정규화한 이름. 비트 주소가 아니면 None.
    fn bit(&self, text: &str) -> Option<String>;
    /// 표시등 줄 `indicators[].from`부터 `count`개의 정규화한 이름. `from`이 비트 주소가 아니면 None,
    /// 줄이 그 장치의 마지막 주소를 넘으면 Some(Err(`device <id>: indicators[i].from: ` 뒤의 글)).
    fn bit_run(&self, text: &str, count: u32) -> Option<Result<Vec<String>, String>>;
    /// `position.from`: 워드 장치(또는 모듈 워드)이면 정규화한 이름, 아니면 None.
    fn word_device(&self, text: &str) -> Option<String>;
    /// 이 워드의 32비트 읽기가 가능한가(다음 단어의 이름이 있는가). 없으면 `position`은 16비트로 읽는다.
    fn can_read32(&self, addr: &str) -> bool;
    /// 부호 있는 32비트 읽기: 아래 단어가 `addr`, 위 단어가 그 다음 단어. 다음 단어가 없으면 None.
    fn read32(&self, io: &dyn PlantIo, addr: &str) -> Option<i64>;
    /// 장면 키 `modules`: 모델이 놓이는 모듈 번호들, 또는 오류 글(`modules: ...`로 시작할 수 있다).
    fn modules(&self, v: &Value) -> Result<Vec<u8>, String>;
    /// 주소가 없는 호스트(트윈 모델 서버)인가. 참이면 장면을 읽을 때 종류의 모든 `io` 칸과 모든 신호가 핀 이름(`<장치 id>.<이름>`)을
    /// 주소로 갖고(`bound`), `show`·`hide`·`indicators`는 핀 형식만 받는다(설계 46의 2.4절과 tasks.md의 핀 형식 결정).
    fn virtual_pins(&self) -> bool {
        false
    }
}

/// 주소가 없는 호스트: 트윈 모델(설계 46의 2절)을 읽는다. 모든 주소 규칙은 거절하고, 핀은 이름 그대로 주소가 된다(`virtual_pins`).
#[derive(Debug)]
pub struct NoAddresses;

pub static NO_ADDRESSES: NoAddresses = NoAddresses;

impl Addresses for NoAddresses {
    fn check(&self, _text: &str, _out: bool, _word: bool) -> Result<String, String> {
        Err("a twin model holds no PLC address".into())
    }
    fn bit(&self, _text: &str) -> Option<String> {
        None
    }
    fn bit_run(&self, _text: &str, _count: u32) -> Option<Result<Vec<String>, String>> {
        None
    }
    fn word_device(&self, _text: &str) -> Option<String> {
        None
    }
    /// 핀 하나가 32비트 값 전체를 가진다(다음 단어가 없다).
    fn can_read32(&self, _addr: &str) -> bool {
        true
    }
    fn read32(&self, io: &dyn PlantIo, addr: &str) -> Option<i64> {
        Some(io.word(addr))
    }
    fn modules(&self, _v: &Value) -> Result<Vec<u8>, String> {
        Err("a twin model holds no module model".into())
    }
    fn virtual_pins(&self) -> bool {
        true
    }
}

/// 핀 이름으로 읽고 쓰는 입출력(`PlantIo`): 트윈 모델의 실행이 쓴다. 없는 핀은 꺼짐과 0이다.
#[derive(Debug, Default)]
pub struct NameIo {
    bits: BTreeMap<String, bool>,
    words: BTreeMap<String, i64>,
}

impl NameIo {
    pub fn new() -> NameIo {
        NameIo::default()
    }
}

impl PlantIo for NameIo {
    fn bit(&self, name: &str) -> bool {
        self.bits.get(name).copied().unwrap_or(false)
    }
    // 키가 이미 있으면 새 String을 만들지 않는다.
    fn set_bit(&mut self, name: &str, v: bool) {
        match self.bits.get_mut(name) {
            Some(b) => *b = v,
            None => {
                self.bits.insert(name.to_string(), v);
            }
        }
    }
    fn set_word(&mut self, name: &str, v: i64) {
        match self.words.get_mut(name) {
            Some(w) => *w = v,
            None => {
                self.words.insert(name.to_string(), v);
            }
        }
    }
    fn word(&self, name: &str) -> i64 {
        self.words.get(name).copied().unwrap_or(0)
    }
}
