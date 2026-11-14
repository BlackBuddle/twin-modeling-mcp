//! 화면으로 가는 서버 문장을 한국어로 바꾸는 번역기와 3D 장면의 문장 표(reusable-parts 3.1b; 옛 `src/page_ko.rs`에서 옮겼다, UN27).
//! 모든 페이지 API 응답은 호스트의 `reply_json`을 지나므로 그곳에서 한 번만 바꾼다. MCP 도구 결과는 이 길을 지나지 않아 영어 그대로다(모델용 문장).
//! 바꾸는 곳: 객체의 "error", "reason", "scene_error", "draft_error" 문자열, "warnings" 문자열 배열, "unread" 객체의 문자열 값(어느 깊이든). 표에 없는 문장은 그대로 보낸다.
//! 페이지가 값으로 비교하는 짧은 코드("declined", "store off", "config off", "no such session")는 표에 넣지 않는다.
//! 이 파일의 표 `KO`는 3D 장면(형식 검사, 기하, 공작물, 기계 모델)의 문장만 둔다. 호스트가 자기 표를 앞에 두고 이 표를 뒤에 놓아 `text_in`, `value_in`을 부른다.

use serde_json::Value;

/// 문장 표: (영어 틀, 한국어 틀)의 목록.
pub type Table = &'static [(&'static str, &'static str)];

/// (영어 틀, 한국어 틀). `{}`는 그 자리의 글자를 옮긴다. 옮긴 글자도 이 표로 한 번 더 바꾼다(감싼 오류 문장).
/// 앞에서부터 처음 맞는 틀을 쓴다: 긴 틀을 짧은 틀보다 앞에 둔다.
pub const KO: Table = &[
    // ── 트윈 모델 (crates/twin-engine/src/model.rs, scene.rs; reusable-parts 3.1d, 설계 46의 2.3·3.3절). 가장 앞에 둔다: 좁은 틀이 먼저 맞아야 한다 ──
    ("scene.schema: a twin model is schema 2", "scene.schema: 트윈 모델은 형식 2입니다"),
    ("scene.photos: a twin model names no photo file (photos stay with the person)", "scene.photos: 트윈 모델은 사진 파일을 이름으로 갖지 않습니다 (사진은 사람이 따로 가집니다)"),
    ("scene.modules: a module model is a PLC feature; plc-mcp adds it when it binds the model", "scene.modules: 모듈 모델은 PLC 기능입니다. plc-mcp가 모델에 주소를 묶을 때 더합니다"),
    ("device {}: io: a twin model holds no PLC address; leave io out (its slots are the pins {}.<slot>, bound by plc-mcp)", "장치 {}: io: 트윈 모델은 PLC 주소를 갖지 않습니다. io를 빼세요 (칸은 핀 {}.<칸>이고 plc-mcp가 묶습니다)"),
    ("device {}: signals[{}].device: a twin model holds no PLC address; leave device out (the pin is {}.{})", "장치 {}: signals[{}].device: 트윈 모델은 PLC 주소를 갖지 않습니다. device를 빼세요 (핀은 {}.{}입니다)"),
    ("device {}: skin.groups[{}].{}: a twin model holds no PLC address; use a pin \"<device id>.<signal>\"", "장치 {}: skin.groups[{}].{}: 트윈 모델은 PLC 주소를 갖지 않습니다. 핀 \"<장치 id>.<신호>\"를 쓰세요"),
    ("device {}: indicators[{}].from: a twin model holds no PLC address; use pins: [\"<device id>.<signal>\", ...]", "장치 {}: indicators[{}].from: 트윈 모델은 PLC 주소를 갖지 않습니다. pins: [\"<장치 id>.<신호>\", ...]를 쓰세요"),
    ("device {}: position: a twin model holds no PLC address; use mech with the preset servo_screw and a word signal", "장치 {}: position: 트윈 모델은 PLC 주소를 갖지 않습니다. 프리셋 servo_screw의 mech와 워드 신호를 쓰세요"),
    ("device {}: signals[{}].name: {} is a slot of {}; a pin name is used once", "장치 {}: signals[{}].name: {}은(는) {}의 칸 이름입니다. 핀 이름은 한 번만 씁니다"),
    ("scene: more than {} pins", "장면: 핀이 {}개를 넘습니다"),
    ("a twin model holds no PLC address", "트윈 모델은 PLC 주소를 갖지 않습니다"),
    ("a twin model holds no module model", "트윈 모델은 모듈 모델을 갖지 않습니다"),
    // 핀 형식의 show, hide, indicators
    ("device {}: {}: no device \"{}\" (devices: {})", "장치 {}: {}: \"{}\" 장치가 없습니다 (장치: {})"),
    ("device {}: {}: no pin \"{}\" (pins of {}: {})", "장치 {}: {}: \"{}\" 핀이 없습니다 ({}의 핀: {})"),
    ("device {}: {}: {} is a word pin; show and hide take a bit pin", "장치 {}: {}: {}은(는) 워드 핀입니다. show와 hide는 비트 핀을 받습니다"),
    ("device {}: {}: {} is a word pin; a lamp takes a bit pin", "장치 {}: {}: {}은(는) 워드 핀입니다. 램프는 비트 핀을 받습니다"),
    ("device {}: indicators[{}]: give pins or from and count, not both", "장치 {}: indicators[{}]: pins 또는 from과 count 중 하나만 주세요"),
    ("device {}: indicators[{}].pins: must be 1 to 64 pins", "장치 {}: indicators[{}].pins는 1개에서 64개여야 합니다"),
    ("device {}: indicators[{}].pins[{}]: must be a pin \"<device id>.<signal>\"", "장치 {}: indicators[{}].pins[{}]는 핀 \"<장치 id>.<신호>\"여야 합니다"),
    // 노트: 주소가 묶이지 않은 핀을 show나 hide가 가리킨다
    ("{}: skin: {} has no PLC address yet; its group is drawn as if the pin is OFF (bind it with io or signals[].device)", "{}: skin: {}에 아직 PLC 주소가 없습니다. 그 그룹은 핀이 꺼진 것처럼 그려집니다 (io나 signals[].device로 묶으세요)"),
    // 화면의 시뮬레이션 경로(`/api/ladder/sim/*`)
    ("no pin \"{}\" (pins: {})", "\"{}\" 핀이 없습니다 (핀: {})"),
    ("value must be true or false for a bit pin", "비트 핀의 값은 true 또는 false여야 합니다"),
    ("no model_sim run of this model yet", "이 모델은 아직 model_sim을 돌린 적이 없습니다"),
    ("source must be none or last_run", "source는 none 또는 last_run이어야 합니다"),
    ("photos are not part of a twin model", "사진은 트윈 모델에 들어 있지 않습니다"),
    // 도구 `model_sim`의 입력 글(모델이 읽는 MCP 글이지만 화면이 같은 모델로 돌릴 때도 보인다)
    ("events[{}].pin: no pin \"{}\" (pins of {}: {})", "events[{}].pin: \"{}\" 핀이 없습니다 ({}의 핀: {})"),
    ("events[{}].pin: no device \"{}\" (devices: {})", "events[{}].pin: \"{}\" 장치가 없습니다 (장치: {})"),
    ("events[{}].pin: {} is written by the plant (dir in); an event sets a pin with dir out", "events[{}].pin: {}은(는) 설비 모델이 씁니다 (dir in). 사건은 dir out 핀에만 줍니다"),
    ("events[{}].pin: must be a pin \"<device id>.<name>\"", "events[{}].pin은 핀 \"<장치 id>.<이름>\"이어야 합니다"),
    ("events[{}].pin: must be text \"<device id>.<name>\"", "events[{}].pin은 글 \"<장치 id>.<이름>\"이어야 합니다"),
    ("events[{}].value: {} is outside the 16-bit range of {} (-32768 to 32767)", "events[{}].value: {}은(는) {}의 16비트 범위 밖입니다 (-32768 ~ 32767)"),
    ("events[{}].value: {} is outside the 32-bit range of {} (-2147483648 to 2147483647)", "events[{}].value: {}은(는) {}의 32비트 범위 밖입니다 (-2147483648 ~ 2147483647)"),
    ("events[{}].value: {} is not a number 0 to 9999 for the bcd pin {}", "events[{}].value: {}은(는) bcd 핀 {}에 쓸 수 있는 0 ~ 9999가 아닙니다"),
    ("events[{}].value: must be an integer or true or false", "events[{}].value는 정수 또는 true나 false여야 합니다"),
    ("events[{}].t_ms: must be an integer from 0 to {}", "events[{}].t_ms는 0에서 {} 사이의 정수여야 합니다"),
    ("events[{}].{}: unknown field", "events[{}].{}은(는) 알 수 없는 필드입니다"),
    ("events[{}]: must be an object", "events[{}]는 객체여야 합니다"),
    ("events: must be an array of at most {}", "events는 {}개 이하의 배열이어야 합니다"),
    ("watch[{}]: must be text \"<device id>.<name>\"", "watch[{}]는 글 \"<장치 id>.<이름>\"이어야 합니다"),
    ("watch[{}]: no pin \"{}\" (pins of {}: {})", "watch[{}]: \"{}\" 핀이 없습니다 ({}의 핀: {})"),
    ("watch[{}]: no device \"{}\" (devices: {})", "watch[{}]: \"{}\" 장치가 없습니다 (장치: {})"),
    ("watch: must be an array of at most {} pins", "watch는 핀 {}개 이하의 배열이어야 합니다"),
    ("the simulation took more than 5 s of real time (stopped at t = {} ms of {} ms); make the run shorter or raise scan_ms", "시뮬레이션이 실제 시간으로 5초를 넘었습니다 (t = {} ms / {} ms에서 멈춤). 실행을 짧게 하거나 scan_ms를 올리세요"),
    // 화면의 `sim/input`(앞의 `events[{}].pin: ` 틀이 먼저 맞아야 하므로 이 넓은 틀은 그 뒤에 둔다)
    ("{} is written by the plant (dir in); an event sets a pin with dir out", "{}은(는) 설비 모델이 씁니다 (dir in). 사람이 주는 핀은 dir out입니다"),
    // ── 3D 장면 형식 2 (crates/twin-engine/src/scene.rs, geom.rs; 작업 A1). 장치 id와 필드 이름은 `{}`로 옮긴다 ──
    ("scene.schema: must be 1 or 2", "scene.schema는 1 또는 2여야 합니다"),
    ("scene: more than {} shape items", "장면: 모양 항목이 {}개를 넘습니다"),
    ("scene.photos[{}].camera: the camera of a photo is set on the page (schema 2)", "scene.photos[{}].camera: 형식 2에서는 사진의 카메라를 화면에서 정합니다"),
    ("scene.photos[{}].state: at most 64 keys", "scene.photos[{}].state: 키는 64개까지입니다"),
    ("scene.photos[{}].state: must be an object", "scene.photos[{}].state는 객체여야 합니다"),
    ("device {}: {}: needs scene.schema 2", "장치 {}: {}은(는) scene.schema 2에서만 쓸 수 있습니다"),
    ("device {}: on: no device \"{}\"", "장치 {}: on: \"{}\" 장치가 없습니다"),
    ("device {}: on: the chain of on makes a loop ({})", "장치 {}: on: on의 연결이 고리를 이룹니다 ({})"),
    ("device {}: on: more than 16 levels", "장치 {}: on: 연결이 16단을 넘습니다"),
    ("device {}: on: must be \"<id>\" or \"<id>.<frame>\"", "장치 {}: on은 \"<id>\" 또는 \"<id>.<틀>\"이어야 합니다"),
    ("device {}: on: {} has no frame \"{}\" (frames: {})", "장치 {}: on: {}에는 \"{}\" 틀이 없습니다 (틀: {})"),
    ("device {}: mount: unknown style \"{}\" (styles: {})", "장치 {}: mount: \"{}\"은(는) 없는 고정 방식입니다 (방식: {})"),
    ("device {}: mount: {} is not for the form {}", "장치 {}: mount: {}은(는) {} 형태에 쓸 수 없습니다"),
    ("device {}: mount: floor needs the world as parent (no on)", "장치 {}: mount: floor는 부모가 세계여야 합니다 (on 없음)"),
    ("device {}: shape: only kind solid", "장치 {}: shape는 solid 종류만 가질 수 있습니다"),
    ("device {}: shape: a solid needs 1 to {} items", "장치 {}: solid에는 모양 항목이 1~{}개 필요합니다"),
    ("device {}: frames: at most 16", "장치 {}: frames는 16개까지입니다"),
    ("device {}: frames.{}: a standard frame name", "장치 {}: frames.{}은(는) 표준 틀 이름이라 쓸 수 없습니다"),
    ("device {}: frames.{}: name must be 1 to 16 of a-z 0-9 _", "장치 {}: frames.{}: 이름은 a-z 0-9 _ 1~16자여야 합니다"),
    ("device {}: frames.{}.type: must be surface or axial", "장치 {}: frames.{}.type은 surface 또는 axial이어야 합니다"),
    ("device {}: io: a solid has no I/O", "장치 {}: solid에는 입출력이 없습니다"),
    ("device {}: parts: use params and valve_on (schema 2)", "장치 {}: parts: 형식 2에서는 params와 valve_on을 씁니다"),
    ("device {}: basis: at most 32 keys", "장치 {}: basis는 32개까지입니다"),
    ("device {}: basis.{}: not a param, pose or shape", "장치 {}: basis.{}은(는) 매개변수, pose, shape가 아닙니다"),
    ("device {}: basis.{}: must be user, label, measured, ref or guess", "장치 {}: basis.{}은(는) user, label, measured, ref, guess 중 하나여야 합니다"),
    ("device {}: model: must be text of at most 40 characters", "장치 {}: model은 40자 이하의 글이어야 합니다"),
    // ── 작업 A2: 크기, datum, 기준 길이 ──
    ("device {}: params.body_len_mm: must be at least stroke_mm + 20 ({})", "장치 {}: params.body_len_mm은 stroke_mm + 20 ({}) 이상이어야 합니다"),
    ("device {}: params.body_len_mm: must be at least stroke_mm + carriage_len_mm ({})", "장치 {}: params.body_len_mm은 stroke_mm + carriage_len_mm ({}) 이상이어야 합니다"),
    ("device {}: datum: {} moves with the axis of {}", "장치 {}: datum: {}은(는) {}의 축과 함께 움직이는 틀이라 쓸 수 없습니다"),
    ("device {}: datum: no frame \"{}\" (frames: {})", "장치 {}: datum: \"{}\" 틀이 없습니다 (틀: {})"),
    ("device {}: datum: must be text", "장치 {}: datum은 글이어야 합니다"),
    ("device {}: mount: the mount {} needs the frame {}, which the form {} has not got in this build", "장치 {}: mount: 고정 방식 {}에는 {} 틀이 필요한데 {} 형태에는 이 빌드에 그 틀이 없습니다"),
    ("device {}: frames: only kind solid (other kinds: a later task)", "장치 {}: frames는 solid 종류만 가질 수 있습니다 (다른 종류는 뒤 작업)"),
    ("scene.refs: at most 16 items", "scene.refs는 16개까지입니다"),
    ("scene.refs: needs scene.schema 2", "scene.refs는 scene.schema 2에서만 쓸 수 있습니다"),
    ("scene.refs[{}].mm: must be 1 to 100000", "scene.refs[{}].mm은 1~100000이어야 합니다"),
    ("scene.refs[{}].basis: must be measured or label", "scene.refs[{}].basis는 measured 또는 label이어야 합니다"),
    // ── 작업 A3: 외형, 표시등, 표시 ──
    ("scene: more than {} skin items", "장면: 외형 항목이 {}개를 넘습니다"),
    ("scene: more than {} indicator lamps", "장면: 표시등이 {}개를 넘습니다"),
    ("device {}: skin.groups: must be 1 to 16 groups", "장치 {}: skin.groups는 그룹 1~16개여야 합니다"),
    ("device {}: skin: more than 64 items", "장치 {}: skin 항목이 64개를 넘습니다"),
    ("device {}: skin: more than 16384 bytes", "장치 {}: skin이 16384바이트를 넘습니다"),
    ("device {}: skin: must be an object", "장치 {}: skin은 객체여야 합니다"),
    ("device {}: skin.groups[{}].name: must be 1 to 16 of a-z 0-9 _", "장치 {}: skin.groups[{}].name은 a-z 0-9 _ 1~16자여야 합니다"),
    ("device {}: skin.groups[{}].name: \"{}\" is used twice", "장치 {}: skin.groups[{}].name \"{}\"이(가) 두 번 쓰였습니다"),
    ("device {}: skin.groups[{}].frame: {} has no frame \"{}\" (frames: {})", "장치 {}: skin.groups[{}].frame: {}에는 \"{}\" 틀이 없습니다 (틀: {})"),
    ("device {}: skin.groups[{}]: give show or hide, not both", "장치 {}: skin.groups[{}]: show와 hide 중 하나만 쓰세요"),
    ("device {}: skin.groups[{}].{}: {} is not an address of the scene", "장치 {}: skin.groups[{}].{}: {}은(는) 장면의 주소가 아닙니다"),
    ("device {}: skin.groups[{}].{}: must be an X, Y or M bit address", "장치 {}: skin.groups[{}].{}은 X, Y, M 비트 주소여야 합니다"),
    ("device {}: skin.groups[{}].items[{}].tone: mark needs a group with show", "장치 {}: skin.groups[{}].items[{}].tone: mark는 show가 있는 그룹에서만 씁니다"),
    ("device {}: skin.groups[{}].items[{}].tone: must be one of {}", "장치 {}: skin.groups[{}].items[{}].tone은 {} 중 하나여야 합니다"),
    ("device {}: skin_source: must be model or checked", "장치 {}: skin_source는 model 또는 checked여야 합니다"),
    ("device {}: skin_source: the device has no skin", "장치 {}: 외형(skin)이 없는 장치에는 skin_source를 쓸 수 없습니다"),
    ("device {}: indicators: at most 8 rows", "장치 {}: indicators는 8줄까지입니다"),
    ("device {}: indicators: more than 128 lamps", "장치 {}: 표시등이 128개를 넘습니다"),
    ("device {}: indicators[{}].from: must be an X, Y or M bit address", "장치 {}: indicators[{}].from은 X, Y, M 비트 주소여야 합니다"),
    ("device {}: indicators[{}].from: {} + {} is past the last {} address", "장치 {}: indicators[{}].from: {} + {}은(는) 마지막 {} 주소를 넘습니다"),
    ("device {}: indicators[{}].count: must be 1 to 64", "장치 {}: indicators[{}].count는 1~64여야 합니다"),
    ("device {}: indicators[{}].step: must be at least 0.5 mm long", "장치 {}: indicators[{}].step은 길이가 0.5 mm 이상이어야 합니다"),
    ("device {}: indicators[{}].d: must be 1 to 50", "장치 {}: indicators[{}].d는 1~50이어야 합니다"),
    // ── 형식 1 장면 검사 문장 (FR1). `params.<키>`는 화면이 `/params\.(\w+)/`로 칸을 찾으므로 한국어 글 안에서도 영문 그대로 둔다 ──
    ("device {}: params.{}: must be a number from {} to {}", "장치 {}: params.{}은(는) {}~{} 사이의 숫자여야 합니다"),
    ("device {}: params.{}: must be one of {}", "장치 {}: params.{}은(는) 다음 중 하나여야 합니다: {}"),
    ("device {}: params.{}: must be true or false", "장치 {}: params.{}은(는) true 또는 false여야 합니다"),
    ("device {}: params.{}: unknown parameter for this kind", "장치 {}: params.{}은(는) 이 종류에 없는 매개변수입니다"),
    ("device {}: id: is used twice", "장치 {}: id가 두 번 쓰였습니다"),
    ("device {}: kind: is needed", "장치 {}: kind가 필요합니다"),
    ("device {}: kind: `{}` is not in the catalog", "장치 {}: kind `{}`은(는) 종류 목록에 없습니다"),
    ("device {}: parts.{}: `{}` is not a part of {}", "장치 {}: parts.{}: `{}`은(는) {}의 부품이 아닙니다"),
    ("device {}: io.{}: `{}` is not a slot of {}; slots: {}", "장치 {}: io.{}: `{}`은(는) {}의 칸이 아닙니다. 칸: {}"),
    ("device {}: io.{}: output address {} is already used by another output slot", "장치 {}: io.{}: 출력 주소 {}은(는) 다른 출력 칸이 이미 씁니다"),
    ("device {}: signals: {} has no signal list; use io slots", "장치 {}: signals: {}에는 신호 목록이 없습니다. io 칸을 쓰세요"),
    ("device {}: signals: must be an array of at most {}", "장치 {}: signals는 {}개까지의 배열이어야 합니다"),
    ("device {}: {}: must be 1 to 24 characters of A-Z a-z 0-9 _ -", "장치 {}: {}은(는) A-Z a-z 0-9 _ - 1~24자여야 합니다"),
    ("device {}: {}: must be \"in\" or \"out\"", "장치 {}: {}은(는) \"in\" 또는 \"out\"이어야 합니다"),
    ("device {}: {}: must be bit, word or bcd", "장치 {}: {}은(는) bit, word, bcd 중 하나여야 합니다"),
    ("device {}: {}: must be \"high\" or \"low\"", "장치 {}: {}은(는) \"high\" 또는 \"low\"여야 합니다"),
    ("device {}: timing: {} has no timing", "장치 {}: timing: {}에는 시간 값이 없습니다"),
    ("device {}: {}: must be 0 to 60000", "장치 {}: {}은(는) 0~60000이어야 합니다"),
    ("device {}: {}: must be an integer from 1 to 100000", "장치 {}: {}은(는) 1~100000의 정수여야 합니다"),
    ("device {}: {}: must be an ISO date string of at most 32 characters", "장치 {}: {}은(는) 32자 이하의 ISO 날짜 글이어야 합니다"),
    ("device {}: {}: must be a short word (draft, measured, manual)", "장치 {}: {}은(는) 짧은 낱말(draft, measured, manual)이어야 합니다"),
    ("device {}: checked: {} has no valve", "장치 {}: checked: {}에는 밸브가 없습니다"),
    ("device {}: {}: must be true or false", "장치 {}: {}은(는) true 또는 false여야 합니다"),
    ("device {}: {}: must be box or cyl", "장치 {}: {}은(는) box 또는 cyl이어야 합니다"),
    ("device {}: {}: must be x, y or z", "장치 {}: {}은(는) x, y, z 중 하나여야 합니다"),
    ("device {}: {}: must be 0.5 to 20000", "장치 {}: {}은(는) 0.5~20000이어야 합니다"),
    ("device {}: io.{}: {}", "장치 {}: io.{}: {}"),
    ("`{}` is not an X, Y or M address", "`{}`은(는) X, Y, M 주소가 아닙니다"),
    ("`{}` is not a D address", "`{}`은(는) D 주소가 아닙니다"),
    ("`{}` cannot be {} here ({})", "`{}`은(는) 여기에 {}으로 쓸 수 없습니다 ({})"),
    ("an output", "출력"),
    ("an input", "입력"),
    ("use Y or M", "Y나 M을 쓰세요"),
    ("use X or M", "X나 M을 쓰세요"),
    ("scene must be a JSON object", "장면은 JSON 객체여야 합니다"),
    ("scene name must be 1 to 32 characters of A-Z a-z 0-9 - _ (and not a Windows device name)", "장면 이름은 A-Z a-z 0-9 - _ 1~32자이고 Windows 장치 이름이 아니어야 합니다"),
    ("scene name must be 1 to 32 characters of A-Z a-z 0-9 - _", "장면 이름은 A-Z a-z 0-9 - _ 1~32자여야 합니다"),
    ("scene is larger than {} bytes", "장면이 {}바이트보다 큽니다"),
    ("scene.name: must be text of at most {} characters", "scene.name은 {}자 이하의 글이어야 합니다"),
    ("scene.photos: must be an array of at most {}", "scene.photos는 {}개까지의 배열이어야 합니다"),
    ("scene.photos[{}].file: must be a plain file name", "scene.photos[{}].file은 평범한 파일 이름이어야 합니다"),
    ("scene.photos[{}].camera: needs pos, target (3 numbers) and fov (1 to 179)", "scene.photos[{}].camera에는 pos, target(숫자 3개), fov(1~179)가 필요합니다"),
    ("a stored file is larger than {} bytes", "저장된 파일이 {}바이트보다 큽니다"),
    ("a stored file is not valid JSON", "저장된 파일이 올바른 JSON이 아닙니다"),
    ("the {} is larger than {} bytes when it is stored ({} bytes); make it smaller", "저장하면 {}이(가) {}바이트를 넘습니다 ({}바이트). 더 작게 만드세요"),
    ("scene {} is not a file of at most {} bytes", "장면 {}은(는) {}바이트 이하의 파일이 아닙니다"),
    ("scene.links: must be an array of at most {}", "scene.links는 {}개까지의 배열이어야 합니다"),
    ("scene.links[{}]: must be an object", "scene.links[{}]는 객체여야 합니다"),
    ("scene.links[{}].{}: unknown field", "scene.links[{}].{}은(는) 알 수 없는 필드입니다"),
    ("scene.links[{}].kind: must be \"air\" or \"wire\"", "scene.links[{}].kind는 \"air\" 또는 \"wire\"여야 합니다"),
    ("scene.links[{}].{}: must be \"<device>.<port>\"", "scene.links[{}].{}은(는) \"<장치>.<포트>\"여야 합니다"),
    ("scene.links[{}].{}: `{}` must be \"<device>.<port>\"", "scene.links[{}].{}: `{}`은(는) \"<장치>.<포트>\"여야 합니다"),
    ("scene.links[{}].{}: unknown device in `{}`", "scene.links[{}].{}: `{}`의 장치를 찾을 수 없습니다"),
    ("scene.links[{}].{}: unknown port `{}` (ports of {}: {})", "scene.links[{}].{}: `{}`은(는) 없는 포트입니다 ({}의 포트: {})"),
    ("scene.links[{}].{}: port `{}` is {}, but the link is {}", "scene.links[{}].{}: 포트 `{}`은(는) {}인데 연결은 {}입니다"),
    // ── 장면 저장 길의 나머지 문장 (AR7). 일반 틀은 맨 뒤에 둔다 ──
    ("device {}: on: \"{}\" is a port of {}, not a frame{}", "장치 {}: on: \"{}\"은(는) {}의 포트이지 틀이 아닙니다{}"),
    ("device {}: {}: must be {pos:[x,y,z], rot:[x,y,z]} with numbers in range", "장치 {}: {}은(는) {pos:[x,y,z], rot:[x,y,z]} 모양의 범위 안의 숫자여야 합니다"),
    ("device {}: {}: must be {pos, rot, type}", "장치 {}: {}은(는) {pos, rot, type} 모양이어야 합니다"),
    ("device {}: {}: must be 3 numbers from {} to {} (needed when count is above 1)", "장치 {}: {}은(는) {}~{}의 숫자 3개여야 합니다 (count가 1보다 크면 필요합니다)"),
    ("device {}: {}: must be 3 numbers from {} to {}", "장치 {}: {}은(는) {}~{}의 숫자 3개여야 합니다"),
    ("device {}: {}: must be 1 to {} items", "장치 {}: {}은(는) 항목 1~{}개여야 합니다"),
    ("device {}: {}: has control characters", "장치 {}: {}에 제어·서식·방향 문자나 공백처럼 보이는 문자가 있습니다"),
    ("scene.name: has control characters", "scene.name에 제어·서식·방향 문자나 공백처럼 보이는 문자가 있습니다"),
    ("device {}: {}: must be text of at most {} characters", "장치 {}: {}은(는) {}자 이하의 글이어야 합니다"),
    ("device {}: {}: must be an object", "장치 {}: {}은(는) 객체여야 합니다"),
    ("device {}: {}: must be an array", "장치 {}: {}은(는) 배열이어야 합니다"),
    ("device {}: {}: must be text", "장치 {}: {}은(는) 글이어야 합니다"),
    ("device {}: {}: {} has no frame \"{}\" (frames: {})", "장치 {}: {}: {}에는 \"{}\" 틀이 없습니다 (틀: {})"),
    ("scene.devices[{}].id: {} is reserved (it is a property name of a JavaScript object); use another id", "scene.devices[{}].id: {}은(는) 예약된 이름입니다 (JavaScript 객체의 속성 이름이라 쓸 수 없습니다). 다른 id를 쓰세요"),
    ("scene.devices[{}].id: must be 1 to 32 characters of A-Z a-z 0-9 - _", "scene.devices[{}].id는 A-Z a-z 0-9 - _ 1~32자여야 합니다"),
    ("scene.devices[{}]: must be an object", "scene.devices[{}]는 객체여야 합니다"),
    ("scene.devices: must be an array of at most {}", "scene.devices는 {}개까지의 배열이어야 합니다"),
    ("scene.photos[{}].state.{}: must be 6 numbers from -360 to 360", "scene.photos[{}].state.{}은(는) -360~360의 숫자 6개여야 합니다"),
    ("scene.photos[{}].state.{}: must be {} to {}", "scene.photos[{}].state.{}은(는) {}~{}이어야 합니다"),
    ("scene.photos[{}].state.{}: not a device with an axis", "scene.photos[{}].state.{}은(는) 축이 있는 장치가 아닙니다"),
    ("scene.photos[{}].{}: unknown field", "scene.photos[{}].{}은(는) 알 수 없는 필드입니다"),
    ("scene.refs[{}]: must be an object", "scene.refs[{}]는 객체여야 합니다"),
    ("scene.refs[{}].{}: unknown field", "scene.refs[{}].{}은(는) 알 수 없는 필드입니다"),
    ("scene.refs[{}].{}: must be \"<id>.<frame>\"", "scene.refs[{}].{}은(는) \"<id>.<틀>\"이어야 합니다"),
    ("scene {} not found; it has only a draft (confirm it on the 3D page, or send scene_json)", "장면 {}은(는) 아직 확정하지 않은 초안만 있습니다 (3D 화면에서 확정하거나 scene_json으로 보내세요)"),
    ("scene {} not found", "장면 {}이(가) 없습니다"),
    ("devices must be an array of at most 200 ids", "devices는 id 200개까지의 배열이어야 합니다"),
    ("devices must be an array of ids", "devices는 id의 배열이어야 합니다"),
    ("checked must be true or false", "checked는 true 또는 false여야 합니다"),
    ("scene has no devices", "장면에 장치가 없습니다"),
    ("at most 200 devices", "장치는 200개까지입니다"),
    ("device {}: {}: unknown field", "장치 {}: {}은(는) 알 수 없는 필드입니다"),
    // ── 공작물 키 (src/twin/work.rs; 작업 B1, 계약 14.12) ──
    ("scene.{}: needs scene.schema 2", "scene.{}은(는) scene.schema 2에서만 쓸 수 있습니다"),
    ("scene.points: at most 32 items", "scene.points는 32개까지입니다"),
    ("scene.points[{}]: must be an object", "scene.points[{}]는 객체여야 합니다"),
    ("scene.points[{}].{}: unknown field", "scene.points[{}].{}은(는) 알 수 없는 필드입니다"),
    ("scene.points[{}].name: must be 1 to 24 of a-z 0-9 _", "scene.points[{}].name은 a-z 0-9 _ 1~24자여야 합니다"),
    ("scene.points[{}].name: {} is used twice", "scene.points[{}].name: {}이(가) 두 번 쓰였습니다"),
    ("scene.points[{}].on: must be \"<id>\" or \"<id>.<frame>\"", "scene.points[{}].on은 \"<id>\" 또는 \"<id>.<틀>\"이어야 합니다"),
    ("scene.points[{}].on: no device \"{}\"", "scene.points[{}].on: \"{}\" 장치가 없습니다"),
    ("scene.points[{}].on: {} has no frame \"{}\" (frames: {})", "scene.points[{}].on: {}에는 \"{}\" 틀이 없습니다 (틀: {})"),
    ("scene.points[{}].on: \"{}\" is a port of {}, not a frame{}", "scene.points[{}].on: \"{}\"은(는) {}의 포트이지 틀이 아닙니다{}"),
    ("scene.points[{}].pos: must be 3 numbers from -{} to {}", "scene.points[{}].pos는 -{}~{}의 숫자 3개여야 합니다"),
    ("scene.points[{}].catch_mm: must be 0 to 200", "scene.points[{}].catch_mm은 0~200이어야 합니다"),
    ("scene.points[{}].holds: must be 1 to 50", "scene.points[{}].holds는 1~50이어야 합니다"),
    ("scene.points[{}].basis: must be user, label, measured, ref or guess", "scene.points[{}].basis는 user, label, measured, ref, guess 중 하나여야 합니다"),
    ("scene.workpieces: must be an object", "scene.workpieces는 객체여야 합니다"),
    ("scene.workpieces.types: needs 1 to 8 types", "scene.workpieces.types에는 종류가 1~8개 필요합니다"),
    ("scene.workpieces.types: at most 8", "scene.workpieces.types는 8개까지입니다"),
    ("scene.workpieces.types: the name \"{}\" must be 1 to 24 of a-z 0-9 _", "scene.workpieces.types: 이름 \"{}\"은(는) a-z 0-9 _ 1~24자여야 합니다"),
    ("scene.workpieces.types.{}: must be an object", "scene.workpieces.types.{}는 객체여야 합니다"),
    ("scene.workpieces.types.{}.{}: unknown field", "scene.workpieces.types.{}.{}은(는) 알 수 없는 필드입니다"),
    ("scene.workpieces.types.{}.shape: must be round or square", "scene.workpieces.types.{}.shape는 round 또는 square여야 합니다"),
    ("scene.workpieces.types.{}.d_mm: a round part needs d_mm (2 to 500)", "scene.workpieces.types.{}.d_mm: 둥근 부품에는 d_mm(2~500)이 필요합니다"),
    ("scene.workpieces.types.{}.side_mm: a square part needs side_mm (2 to 500)", "scene.workpieces.types.{}.side_mm: 네모 부품에는 side_mm(2~500)이 필요합니다"),
    ("scene.workpieces.types.{}.side_mm: a round part has no side_mm", "scene.workpieces.types.{}.side_mm: 둥근 부품에는 side_mm이 없습니다"),
    ("scene.workpieces.types.{}.d_mm: a square part has no d_mm", "scene.workpieces.types.{}.d_mm: 네모 부품에는 d_mm이 없습니다"),
    ("scene.workpieces.types.{}.h_mm: must be 1 to 500", "scene.workpieces.types.{}.h_mm은 1~500이어야 합니다"),
    ("scene.workpieces.types.{}.material: must be metal or plastic", "scene.workpieces.types.{}.material은 metal 또는 plastic이어야 합니다"),
    ("scene.workpieces.types.{}.tone: must be one of {}", "scene.workpieces.types.{}.tone은 {} 중 하나여야 합니다"),
    ("scene.workpieces.types.{}.basis: must be user, label, measured, ref or guess", "scene.workpieces.types.{}.basis는 user, label, measured, ref, guess 중 하나여야 합니다"),
    ("scene.workpieces.start: at most 20 items", "scene.workpieces.start는 20개까지입니다"),
    ("scene.workpieces.start[{}]: must be an object", "scene.workpieces.start[{}]는 객체여야 합니다"),
    ("scene.workpieces.start[{}].{}: unknown field", "scene.workpieces.start[{}].{}은(는) 알 수 없는 필드입니다"),
    ("scene.workpieces.start[{}].{}: must be text", "scene.workpieces.start[{}].{}은(는) 글이어야 합니다"),
    ("scene.workpieces.start[{}].type: no workpiece type \"{}\" (types: {})", "scene.workpieces.start[{}].type: \"{}\" 공작물 종류가 없습니다 (종류: {})"),
    ("scene.workpieces.start[{}].at: no point \"{}\"", "scene.workpieces.start[{}].at: \"{}\" 점이 없습니다"),
    ("scene.workpieces.{}: unknown field", "scene.workpieces.{}은(는) 알 수 없는 필드입니다"),
    ("scene.modules: {}","scene.modules 설정 오류: {}"),
    ("scene: more than {} stacks", "장면: 매거진(stack)이 {}개를 넘습니다"),
    ("scene: more than {} push rules", "장면: 밀기 규칙(push)이 {}개를 넘습니다"),
    ("scene: more than {} stops", "장면: 멈춤 규칙(stop)이 {}개를 넘습니다"),
    ("scene: more than {} sensor rules", "장면: 부품 감지 규칙(sense)이 {}개를 넘습니다"),
    ("scene: more than {} parts at the start", "장면: 시작 때의 부품이 {}개를 넘습니다"),
    ("device {}: stack.point: must be the name of a point", "장치 {}: stack.point는 점의 이름이어야 합니다"),
    ("device {}: stack.point: no point \"{}\"", "장치 {}: stack.point: \"{}\" 점이 없습니다"),
    ("device {}: stack.fed_by: must be the id of a cylinder", "장치 {}: stack.fed_by는 실린더의 id여야 합니다"),
    ("device {}: stack.fed_by: no device \"{}\"", "장치 {}: stack.fed_by: \"{}\" 장치가 없습니다"),
    ("device {}: stack.fed_by: {} has no push rule from {}", "장치 {}: stack.fed_by: {}에는 {}에서 미는 push 규칙이 없습니다"),
    ("device {}: stack.content: at most 20 items", "장치 {}: stack.content는 20개까지입니다"),
    ("device {}: stack.content[{}]: must be the name of a workpiece type", "장치 {}: stack.content[{}]는 공작물 종류의 이름이어야 합니다"),
    ("device {}: stack.content[{}]: no workpiece type \"{}\" (types: {})", "장치 {}: stack.content[{}]: \"{}\" 공작물 종류가 없습니다 (종류: {})"),
    ("device {}: stack.refill: must be the name of a workpiece type", "장치 {}: stack.refill은 공작물 종류의 이름이어야 합니다"),
    ("device {}: stack.refill: no workpiece type \"{}\" (types: {})", "장치 {}: stack.refill: \"{}\" 공작물 종류가 없습니다 (종류: {})"),
    ("device {}: push: only a cylinder with a valve", "장치 {}: push는 밸브가 있는 실린더만 가질 수 있습니다"),
    ("device {}: push: at most 4 rules", "장치 {}: push는 4개까지입니다"),
    ("device {}: push[{}].{}: must be the name of a point", "장치 {}: push[{}].{}은(는) 점의 이름이어야 합니다"),
    ("device {}: push[{}].{}: no point \"{}\"", "장치 {}: push[{}].{}: \"{}\" 점이 없습니다"),
    ("device {}: push[{}]: from and to are the same point", "장치 {}: push[{}]: from과 to가 같은 점입니다"),
    ("device {}: stop: only a conveyor or a cylinder with a valve", "장치 {}: stop은 벨트나 밸브가 있는 실린더만 가질 수 있습니다"),
    ("device {}: stop.at: must be the name of a point", "장치 {}: stop.at은 점의 이름이어야 합니다"),
    ("device {}: stop.at: no point \"{}\"", "장치 {}: stop.at: \"{}\" 점이 없습니다"),
    ("device {}: stop.belt: a conveyor stops its own belt (give only at)", "장치 {}: stop.belt: 벨트는 자기 벨트를 멈춥니다 (at만 적으세요)"),
    ("device {}: stop.belt: must be the id of a conveyor", "장치 {}: stop.belt는 벨트의 id여야 합니다"),
    ("device {}: stop.belt: {} is not a conveyor of the scene", "장치 {}: stop.belt: {}은(는) 장면의 벨트가 아닙니다"),
    ("device {}: sense: only a photo or proximity sensor", "장치 {}: sense는 광전 또는 근접 센서만 가질 수 있습니다"),
    ("device {}: sense: give exactly one of of, point, held_by, pulse_of", "장치 {}: sense: of, point, held_by, pulse_of 중 정확히 하나를 적으세요"),
    ("device {}: sense: within_mm and material belong to the point form", "장치 {}: sense: within_mm과 material은 point 형태에만 씁니다"),
    ("device {}: sense.of: the axis form is planned and not built yet", "장치 {}: sense.of: 축 형태는 계획만 있고 아직 만들지 않았습니다"),
    ("device {}: sense.within_mm: must be 1 to 200", "장치 {}: sense.within_mm은 1~200이어야 합니다"),
    ("device {}: sense.material: must be any or metal", "장치 {}: sense.material은 any 또는 metal이어야 합니다"),
    ("device {}: sense.mm_per_pulse: must be 0.5 to 1000", "장치 {}: sense.mm_per_pulse는 0.5~1000이어야 합니다"),
    ("device {}: sense.{}: must be the name of a point", "장치 {}: sense.{}은(는) 점의 이름이어야 합니다"),
    ("device {}: sense.{}: must be the id of a device that holds", "장치 {}: sense.{}은(는) 잡는 장치의 id여야 합니다"),
    ("device {}: sense.{}: must be the id of a conveyor", "장치 {}: sense.{}은(는) 벨트의 id여야 합니다"),
    ("device {}: sense.point: no point \"{}\"", "장치 {}: sense.point: \"{}\" 점이 없습니다"),
    ("device {}: sense.held_by: {} is not a device that holds a part", "장치 {}: sense.held_by: {}은(는) 부품을 잡는 장치가 아닙니다"),
    ("device {}: sense.pulse_of: {} is not a conveyor of the scene", "장치 {}: sense.pulse_of: {}은(는) 장면의 벨트가 아닙니다"),
    ("device {}: position: only a servo_axis", "장치 {}: position은 servo_axis만 가질 수 있습니다"),
    ("device {}: position.from: must be a word device (D, W, R, ZR) or a module word U<n>\\G<n>", "장치 {}: position.from은 워드 장치(D, W, R, ZR)나 모듈 워드 U<n>\\G<n>이어야 합니다"),
    ("device {}: position.width: must be 16 or 32", "장치 {}: position.width는 16 또는 32여야 합니다"),
    ("device {}: position.per_mm: must be 0.0001 to 100000", "장치 {}: position.per_mm은 0.0001~100000이어야 합니다"),
    ("device {}: position.speed_mm_s: must be 0.1 to 5000", "장치 {}: position.speed_mm_s는 0.1~5000이어야 합니다"),
    ("scene.{}: unknown field", "scene.{}은(는) 알 수 없는 필드입니다"), // 더 구체적인 scene.* 틀 뒤에 둔다
    ("confirm or discard the draft first", "먼저 초안을 확정하거나 버리세요"),
    ("device {}: no such device", "장치 {}: 그런 장치가 없습니다"),
    ("device {}: the device has no skin", "장치 {}: 외형(skin)이 없는 장치입니다"),
    // ── 래더 화면 시뮬레이션의 부품(작업 B4, `sim/start`의 `stacks`, `sim/part`, `sim/input`) ──
    ("stacks must be an object of at most 8 stacks, such as {\"magazine\":[\"white\"]}", "stacks는 매거진 8개까지의 객체여야 합니다. 예: {\"magazine\":[\"white\"]}"),
    ("stacks need scene", "stacks에는 scene이 필요합니다"),
    ("the scene has no stacks", "장면에 매거진(stack)이 없습니다"),
    ("each stack takes a list of at most 20 type names", "매거진마다 종류 이름을 20개까지 담은 목록을 줍니다"),
    ("stack {} is not a stack of the scene", "{}은(는) 장면의 매거진이 아닙니다"),
    ("the stack {} already holds 20 parts", "매거진 {}에는 이미 부품이 20개 있습니다"),
    ("type {} is not a workpiece type of the scene (types: {})", "{}은(는) 장면의 공작물 종류가 아닙니다 (종류: {})"),
    ("parts need scene", "부품을 다루려면 scene이 필요합니다"),
    ("the scene has no workpieces", "장면에 공작물이 없습니다"),
    ("no point \"{}\"", "\"{}\" 점이 없습니다"),
    ("no part {}", "{}번 부품이 없습니다"),
    ("s_mm needs a belt in at", "s_mm은 at이 벨트일 때만 씁니다"),
    ("s_mm must be 0 to {}", "s_mm은 0~{}이어야 합니다"),
    ("s_mm must be a number", "s_mm은 숫자여야 합니다"),
    ("tone must be one of {}", "tone은 {} 중 하나여야 합니다"),
    ("unknown field {}", "{}은(는) 알 수 없는 필드입니다"),
    ("body must be an object", "본문은 객체여야 합니다"),
    ("part must be a part number", "part는 부품 번호여야 합니다"),
    ("give part with one of at, tone, remove (true)", "part와 함께 at, tone, remove(true) 중 하나를 주세요"),
    ("give one of at, pos, remove, reset (or part with at, tone, remove)", "at, pos, remove, reset 중 하나를 주세요 (또는 part와 at, tone, remove 중 하나)"),
    ("pos must be 3 numbers from -100000 to 100000", "pos는 -100000~100000의 숫자 3개여야 합니다"),
    ("value null is only for a sensor bit that a part rule writes", "value null은 부품 규칙이 쓰는 센서 비트에만 줄 수 있습니다"),
    ("fault device {} is not a moving device or a sensor with sense of the scene", "고장 장치 {}은(는) 장면의 움직이는 장치도 부품 감지 센서도 아닙니다"),
    ("fault on sensor {} must be sensor_stuck_on or sensor_stuck_off", "센서 {}의 고장은 sensor_stuck_on 또는 sensor_stuck_off여야 합니다"),
    // ── `{}`로 시작하는 틀은 맨 뒤에 둔다(다른 문장을 먼저 맞춰 보게) ──
    ("{} must be text of at most 32 characters", "{}은(는) 32자 이하의 글이어야 합니다"),
    ("{} (line numbers count instructions only)", "{} (줄 번호는 명령만 셉니다)"),
    ("{} unknown step word(s) are shown as unknown boxes with their raw words", "알 수 없는 스텝 워드 {}개를 \"알 수 없음\" 상자와 원시 워드로 보입니다"),
    ("{} instruction(s) have operands that could not be decoded (their raw words are shown)", "명령 {}개의 피연산자를 해독하지 못했습니다 (원시 워드를 보입니다)"),
    ("{} step word(s) failed the parity check", "스텝 워드 {}개가 패리티 검사에 실패했습니다"),
    ("{} needs a preceding contact", "{} 앞에 접점이 필요합니다"),
    ("{} has no block to combine", "{}: 합칠 블록이 없습니다"),
    ("{} cannot be inside a rung", "{}은(는) 블록 안에 올 수 없습니다"),
    ("{} has no device", "{}에 장치가 없습니다"),
    ("{} is larger than {} bytes", "{}이(가) {}바이트보다 큽니다"),
    ("{} is not a JSON object", "{}이(가) JSON 객체가 아닙니다"),
    ("cannot read {}: {}", "{}을(를) 읽을 수 없습니다: {}"),
    // 기계 모델 엔진 `mech`(reusable-parts 2.1a): 경로(`joints[0].name`)는 `{}`로 받아 그대로 둔다. 더 좁은 틀을 앞에 둔다.
    ("device {}: mech: give preset or joints", "장치 {}: mech에는 preset이나 joints를 줘야 합니다"),
    ("device {}: mech: more than {} bytes", "장치 {}: mech가 {}바이트를 넘습니다"),
    ("device {}: mech: signal {} is written twice ({}, {})", "장치 {}: mech: 신호 {}을(를) 두 곳에서 씁니다 ({}, {})"),
    ("device {}: mech: signal {} has no device; bind it before a simulation (step 8 of guide.modeling)", "장치 {}: mech: 신호 {}에 주소가 없습니다. 시뮬레이션 전에 주소를 묶으세요 (guide.modeling의 8단계)"),
    ("device {}: mech: kind {} takes no mech (kinds: {})", "장치 {}: mech: {} 종류는 mech를 가질 수 없습니다 (가질 수 있는 종류: {})"),
    ("device {}: mech: kind machine needs mech", "장치 {}: mech: machine 종류에는 mech가 필요합니다"),
    ("device {}: mech: give position or mech, not both (position is the short form of a follow driver)", "장치 {}: mech: position과 mech를 함께 줄 수 없습니다 (position은 follow 구동기의 짧은 꼴입니다)"),
    ("device {}: kind {} needs scene.schema 2", "장치 {}: {} 종류는 scene.schema 2에서만 쓸 수 있습니다"),
    ("scene: more than {} {}", "장면: {}개를 넘는 {}이(가) 있습니다"),
    ("device {}: mech.{}: {} is also written by {} ({})", "장치 {}: mech.{}: {}은(는) {}({})도 씁니다. 엔진이 쓰는 비트는 쓰는 곳이 하나여야 합니다"),
    ("device {}: mech.{}: not a field of mech", "장치 {}: mech.{}은(는) mech의 필드가 아닙니다"),
    ("device {}: mech: give preset or joints, not both", "장치 {}: mech에는 preset과 joints를 함께 줄 수 없습니다"),
    ("device {}: mech.preset: unknown preset \"{}\" (presets for {}: {})", "장치 {}: mech.preset: 모르는 프리셋 \"{}\"입니다 ({} 종류의 프리셋: {})"),
    ("device {}: mech.preset: must be a preset name", "장치 {}: mech.preset은(는) 프리셋 이름이어야 합니다"),
    ("device {}: mech.args: is only for a preset", "장치 {}: mech.args는 preset과 함께만 줄 수 있습니다"),
    ("device {}: mech.args.{}: not an arg of preset {} (args: {})", "장치 {}: mech.args.{}은(는) 프리셋 {}의 인자가 아닙니다 (인자: {})"),
    ("device {}: mech.args.{}: needed by preset {}", "장치 {}: mech.args.{}은(는) 프리셋 {}에 꼭 필요합니다"),
    ("device {}: mech.args.{}: must be a signal name", "장치 {}: mech.args.{}은(는) 신호 이름이어야 합니다"),
    ("device {}: mech.{}: must be a signal name", "장치 {}: mech.{}은(는) 신호 이름이어야 합니다"),
    ("device {}: mech.{}: param times mul is not a finite number", "장치 {}: mech.{}: param과 mul의 곱이 유한한 수가 아닙니다"),
    ("device {}: mech.{}: must be 16 for a bcd signal", "장치 {}: mech.{}은(는) bcd 신호에서는 16이어야 합니다"),
    ("device {}: mech.{}: must be {} numbers from -100000 to 100000 (one for each of joints)", "장치 {}: mech.{}은(는) -100000~100000의 숫자 {}개(관절마다 하나)여야 합니다"),
    ("device {}: mech.args.{}: must be a list of programs", "장치 {}: mech.args.{}은(는) 프로그램 목록이어야 합니다"),
    ("device {}: mech.{}: arg is only for a preset", "장치 {}: mech.{}: arg는 프리셋의 템플릿에서만 씁니다"),
    ("device {}: mech.{}: param must be a parameter name", "장치 {}: mech.{}: param은 매개변수 이름이어야 합니다"),
    ("device {}: mech.{}: params.{} is not given", "장치 {}: mech.{}: params.{}이(가) 주어지지 않았습니다"),
    ("device {}: mech.{}: must be a joint name", "장치 {}: mech.{}은(는) 관절 이름이어야 합니다"),
    ("device {}: mech.{}: must be a number", "장치 {}: mech.{}은(는) 숫자여야 합니다"),
    ("device {}: mech.{}: is needed (must be {} to {})", "장치 {}: mech.{}이(가) 필요합니다 ({}~{})"),
    ("device {}: mech.{}: is needed (integer from 1 to 9999)", "장치 {}: mech.{}이(가) 필요합니다 (1~9999의 정수)"),
    ("device {}: mech.{}: is needed (text)", "장치 {}: mech.{}이(가) 필요합니다 (글)"),
    ("device {}: mech.{}: at most 8", "장치 {}: mech.{}은(는) 8개까지입니다"),
    ("device {}: mech.{}: at most 16", "장치 {}: mech.{}은(는) 16개까지입니다"),
    ("device {}: mech.{}: at most 32 keys", "장치 {}: mech.{}은(는) 키 32개까지입니다"),
    ("device {}: mech.{}: at most one form joint", "장치 {}: mech.{}: form 관절은 하나까지입니다"),
    ("device {}: mech.{}: at most one planar joint", "장치 {}: mech.{}: planar 관절은 하나까지입니다"),
    ("device {}: mech.{}: must be 1 to 8 joints", "장치 {}: mech.{}은(는) 관절 1~8개여야 합니다"),
    ("device {}: mech.{}: must be 1 to 8 joints of this device", "장치 {}: mech.{}은(는) 이 장치의 관절 1~8개여야 합니다"),
    ("device {}: mech.{}: must be 1 to 16 of a-z 0-9 _", "장치 {}: mech.{}은(는) a-z 0-9 _ 1~16자여야 합니다"),
    ("device {}: mech.{}: must be 1 to 16 programs", "장치 {}: mech.{}은(는) 프로그램 1~16개여야 합니다"),
    ("device {}: mech.{}: must be 1 to 32 segments", "장치 {}: mech.{}은(는) 구간 1~32개여야 합니다"),
    ("device {}: mech.{}: must be 2 to 64 points [x, y]", "장치 {}: mech.{}은(는) 점 [x, y] 2~64개여야 합니다"),
    ("device {}: mech.{}: must be [x, y] with numbers from -1000000 to 1000000", "장치 {}: mech.{}은(는) -1000000~1000000의 숫자 [x, y]여야 합니다"),
    ("device {}: mech.{}: must be {} numbers (one for each of joints)", "장치 {}: mech.{}은(는) 숫자 {}개(관절마다 하나)여야 합니다"),
    ("device {}: mech.{}: must be [x, z, heading] (3 numbers, x and z from -100000 to 100000)", "장치 {}: mech.{}은(는) [x, z, 방향] (숫자 3개, x와 z는 -100000~100000)이어야 합니다"),
    ("device {}: mech.{}: must be [xmin, xmax, zmin, zmax] from -100000 to 100000, each min below its max", "장치 {}: mech.{}은(는) -100000~100000의 [xmin, xmax, zmin, zmax]여야 하고 각 min이 max보다 작아야 합니다"),
    ("device {}: mech.{}: must be x, y, z, -x, -y or -z", "장치 {}: mech.{}은(는) x, y, z, -x, -y, -z 중 하나여야 합니다"),
    ("device {}: mech.{}: must be fixed, prismatic, revolute, planar or form", "장치 {}: mech.{}은(는) fixed, prismatic, revolute, planar, form 중 하나여야 합니다"),
    ("device {}: mech.{}: must be coils, follow, step or playback", "장치 {}: mech.{}은(는) coils, follow, step, playback 중 하나여야 합니다"),
    ("device {}: mech.{}: must be ratio, table or diff_drive", "장치 {}: mech.{}은(는) ratio, table, diff_drive 중 하나여야 합니다"),
    ("device {}: mech.{}: must be window", "장치 {}: mech.{}은(는) window여야 합니다"),
    ("device {}: mech.{}: must be 5/2-double, 5/2-single, 3/2-single, 5/3-closed, 5/3-exhaust or none", "장치 {}: mech.{}은(는) 5/2-double, 5/2-single, 3/2-single, 5/3-closed, 5/3-exhaust, none 중 하나여야 합니다"),
    ("device {}: mech.{}: must be 16 or 32", "장치 {}: mech.{}은(는) 16 또는 32여야 합니다"),
    ("device {}: mech.{}: must be user, label, measured, ref or guess", "장치 {}: mech.{}은(는) user, label, measured, ref, guess 중 하나여야 합니다"),
    ("device {}: mech.{}: must be more than 0", "장치 {}: mech.{}은(는) 0보다 커야 합니다"),
    ("device {}: mech.{}: must be below max", "장치 {}: mech.{}은(는) max보다 작아야 합니다"),
    ("device {}: mech.{}: must not be 0", "장치 {}: mech.{}은(는) 0이 아니어야 합니다"),
    ("device {}: mech.{}: must be [z_from, z_to], each 1 to 1000", "장치 {}: mech.{}은(는) [z_from, z_to] (각 1~1000)여야 합니다"),
    ("device {}: mech.{}: {} is used twice", "장치 {}: mech.{}: {}이(가) 두 번 쓰였습니다"),
    ("device {}: mech.{}: {} is a frame of {} (frames: {})", "장치 {}: mech.{}: {}은(는) {}의 틀 이름입니다 (틀: {})"),
    ("device {}: mech.{}: the form joint of servo_axis is named carriage", "장치 {}: mech.{}: servo_axis의 form 관절 이름은 carriage입니다"),
    ("device {}: mech.{}: planar must be on base", "장치 {}: mech.{}: planar 관절은 base 위에 있어야 합니다"),
    ("device {}: mech.{}: more than 8 levels of joints", "장치 {}: mech.{}: 관절이 8단계를 넘게 이어집니다"),
    ("device {}: mech.{}: no frame or earlier joint \"{}\" (frames: {}; joints: {})", "장치 {}: mech.{}: \"{}\" 틀이나 앞선 관절이 없습니다 (틀: {}; 관절: {})"),
    ("device {}: mech.{}: only prismatic and revolute joints have an axis", "장치 {}: mech.{}: axis는 prismatic과 revolute 관절만 가집니다"),
    ("device {}: mech.{}: wrap is only for a revolute joint", "장치 {}: mech.{}: wrap은 revolute 관절만 쓸 수 있습니다"),
    ("device {}: mech.{}: min and max are not for a wrap joint", "장치 {}: mech.{}: wrap 관절에는 min과 max가 없습니다"),
    ("device {}: mech.{}: min and max are only for prismatic and revolute joints", "장치 {}: mech.{}: min과 max는 prismatic과 revolute 관절만 가집니다"),
    ("device {}: mech.{}: the range of the form joint is the axis range", "장치 {}: mech.{}: form 관절의 범위는 축의 범위입니다"),
    ("device {}: mech.{}: prismatic needs min and max", "장치 {}: mech.{}: prismatic 관절에는 min과 max가 필요합니다"),
    ("device {}: mech.{}: revolute needs min and max (or wrap)", "장치 {}: mech.{}: revolute 관절에는 min과 max(또는 wrap)가 필요합니다"),
    ("device {}: mech.{}: only for a planar joint", "장치 {}: mech.{}은(는) planar 관절만 가집니다"),
    ("device {}: mech.{}: is outside the area", "장치 {}: mech.{}이(가) 영역 밖입니다"),
    ("device {}: mech.{}: the form joint starts at the first value of its range", "장치 {}: mech.{}: form 관절은 범위의 첫 값에서 시작합니다"),
    ("device {}: mech.{}: {} is outside {} ({} to {})", "장치 {}: mech.{}: {}은(는) {} 범위({}~{}) 밖입니다"),
    ("device {}: mech.{}: form needs a kind with a form axis that has no valve (servo_axis)", "장치 {}: mech.{}: form 관절은 밸브 없는 형태 축이 있는 종류(servo_axis)에만 쓸 수 있습니다"),
    ("device {}: mech.{}: no joint \"{}\" (joints: {})", "장치 {}: mech.{}: \"{}\" 관절이 없습니다 (관절: {})"),
    ("device {}: mech.{}: no signal \"{}\" (signals: {})", "장치 {}: mech.{}: \"{}\" 신호가 없습니다 (신호: {})"),
    ("device {}: mech.{}: {} is an input of the PLC (dir in); a {} reads a signal with dir out", "장치 {}: mech.{}: {}은(는) PLC의 입력(dir in)입니다. {}은(는) dir out 신호를 읽습니다"),
    ("device {}: mech.{}: {} is an output of the PLC (dir out); a {} writes a signal with dir in", "장치 {}: mech.{}: {}은(는) PLC의 출력(dir out)입니다. {}은(는) dir in 신호를 씁니다"),
    ("device {}: mech.{}: {} is a word signal; {} needs a bit signal", "장치 {}: mech.{}: {}은(는) 워드 신호입니다. {}에는 비트 신호가 필요합니다"),
    ("device {}: mech.{}: {} is a bit signal; {} needs a word or bcd signal", "장치 {}: mech.{}: {}은(는) 비트 신호입니다. {}에는 워드나 bcd 신호가 필요합니다"),
    ("device {}: mech.{}: {} is a {} joint; {}", "장치 {}: mech.{}: {}은(는) {} 관절입니다. {}"),
    ("device {}: mech.{}: {} must be a prismatic or form joint", "장치 {}: mech.{}: {}은(는) prismatic이나 form 관절이어야 합니다"),
    ("device {}: mech.{}: a valve driver needs a joint with min and max ({} wraps)", "장치 {}: mech.{}: 밸브 구동기에는 min과 max가 있는 관절이 필요합니다 ({}은(는) 감깁니다)"),
    ("device {}: mech.{}: fwd_ms, back_ms and lag_ms are not for valve none", "장치 {}: mech.{}: valve none에는 fwd_ms, back_ms, lag_ms가 없습니다"),
    ("device {}: mech.{}: needs valve none", "장치 {}: mech.{}은(는) valve none일 때만 씁니다"),
    ("device {}: mech.{}: give exactly one of count and step", "장치 {}: mech.{}: count와 step 중 하나만 주세요"),
    ("device {}: mech.{}: needs a revolute joint", "장치 {}: mech.{}은(는) revolute 관절에만 쓸 수 있습니다"),
    ("device {}: mech.{}: give exactly one of to and rate", "장치 {}: mech.{}: to와 rate 중 하나만 주세요"),
    ("device {}: mech.{}: give at most one of every and per_turn", "장치 {}: mech.{}: every와 per_turn은 하나까지만 줍니다"),
    ("device {}: mech.{}: a planar joint has no window sensor in this build", "장치 {}: mech.{}: 이 버전에서 planar 관절에는 창 센서가 없습니다"),
    ("device {}: mech.{}: a ratio coupling joins prismatic, revolute or form joints", "장치 {}: mech.{}: ratio 결합은 prismatic, revolute, form 관절을 잇습니다"),
    ("device {}: mech.{}: give exactly one of k, lead_mm, pinion_d_mm, teeth", "장치 {}: mech.{}: k, lead_mm, pinion_d_mm, teeth 중 하나만 주세요"),
    ("device {}: mech.{}: needs one prismatic and one revolute joint", "장치 {}: mech.{}: prismatic 관절 하나와 revolute 관절 하나가 필요합니다"),
    ("device {}: mech.{}: needs two revolute joints", "장치 {}: mech.{}: revolute 관절 둘이 필요합니다"),
    ("device {}: mech.{}: x must grow ({} after {})", "장치 {}: mech.{}: x는 커져야 합니다 ({} 다음에 {})"),
    ("device {}: mech.{}: the first x must be 0 and the last x {}", "장치 {}: mech.{}: 첫 x는 0, 마지막 x는 {}이어야 합니다"),
    ("device {}: mech.{}: the last y ({}) must equal the first y ({})", "장치 {}: mech.{}: 마지막 y({})는 첫 y({})와 같아야 합니다"),
    ("device {}: mech.{}: {} is not a planar joint", "장치 {}: mech.{}: {}은(는) planar 관절이 아닙니다"),
    ("device {}: mech.{}: {} is not a revolute joint", "장치 {}: mech.{}: {}은(는) revolute 관절이 아닙니다"),
    ("device {}: mech.{}: {} has no value source before this coupling (a driver or an earlier coupling must move it)", "장치 {}: mech.{}: {}에는 이 결합 앞의 값 출처가 없습니다 (구동기나 앞선 결합이 움직여야 합니다)"),
    ("device {}: mech.{}: {} is moved twice ({}, {})", "장치 {}: mech.{}: {}을(를) 두 번 움직입니다 ({}, {})"),
    ("device {}: mech.{}: {} is moved by {} and {}", "장치 {}: mech.{}: {}을(를) {}와(과) {}이(가) 함께 움직입니다"),
    // 가장 넓은 틀은 마지막에 둔다(앞의 좁은 틀이 먼저 맞아야 한다)
    ("device {}: mech.{}: must be an integer from {} to {}", "장치 {}: mech.{}은(는) {}~{}의 정수여야 합니다"),
    ("device {}: mech.{}: must be {} to {}", "장치 {}: mech.{}은(는) {}~{}이어야 합니다"),
    // 노트 글(`notes`와 시뮬레이션 상태의 `notes`)
    ("{}: signals: {} has no device yet (bind it in step 8 of guide.modeling)", "{}: signals: {}에 아직 주소가 없습니다 (guide.modeling의 8단계에서 묶으세요)"),
    ("{}: mech.{}: the wheel joints are {} mm apart, track_mm is {}", "{}: mech.{}: 바퀴 관절 사이가 {} mm인데 track_mm은 {}입니다"),
];

/// `pat`의 `{}` 자리를 뺀 나머지가 `s`와 그대로 맞으면 `{}` 자리의 글자들을 돌려준다.
pub fn captures<'a>(pat: &str, s: &'a str) -> Option<Vec<&'a str>> {
    let parts: Vec<&str> = pat.split("{}").collect();
    let mut rest = s.strip_prefix(parts[0])?;
    let mut caps = Vec::new();
    for (i, p) in parts.iter().enumerate().skip(1) {
        // 마지막 조각은 문장 끝에 붙어야 하고, 중간 조각은 처음 나오는 곳에서 자른다.
        let at = if i == parts.len() - 1 { rest.strip_suffix(p)?.len() } else { rest.find(p)? };
        caps.push(&rest[..at]);
        rest = &rest[at + p.len()..];
    }
    rest.is_empty().then_some(caps)
}

/// 옮긴 글자를 다시 바꾸는 깊이의 끝. 감싼 오류 문장은 이 안에서 끝난다(사람이 만든 긴 글이 스택을 넘지 못하게 한다).
pub const MAX_TEXT_DEPTH: usize = 3;
/// 이보다 긴 문장은 바꾸지 않고 그대로 보낸다(표의 문장은 모두 이보다 짧다).
pub const MAX_TEXT_BYTES: usize = 1024;
/// 응답 JSON을 내려가는 깊이의 끝. 이보다 깊은 곳은 바꾸지 않고 그대로 둔다.
pub const MAX_VALUE_DEPTH: usize = 32;

/// 문장 하나를 바꾼다. 표들을 앞에서부터 차례로 찾고, 어디에도 없으면 그대로 돌려준다.
pub fn text_in(msg: &str, tables: &[Table]) -> String {
    text_at(msg, 0, tables)
}

fn text_at(msg: &str, depth: usize, tables: &[Table]) -> String {
    if msg.len() > MAX_TEXT_BYTES {
        return msg.to_string();
    }
    for (en, ko) in tables.iter().flat_map(|t| t.iter()) {
        if let Some(caps) = captures(en, msg) {
            let mut out = String::new();
            let mut parts = ko.split("{}");
            out.push_str(parts.next().unwrap_or(""));
            let lits: Vec<&str> = en.split("{}").collect();
            for (i, (c, p)) in caps.iter().zip(parts).enumerate() {
                // 깊이 끝에서는 옮긴 글자를 다시 바꾸지 않는다. 따옴표 사이의 글자(사람이 쓴 이름)도 바꾸지 않는다 (FR4):
                // 틀에서 `{}` 앞 조각이 따옴표로 끝나고 뒤 조각이 같은 따옴표로 시작하면 그 자리는 이름이다.
                let quoted = lits[i].chars().last().is_some_and(|q| matches!(q, '"' | '\'' | '`') && lits[i + 1].starts_with(q));
                if depth < MAX_TEXT_DEPTH && !quoted {
                    out.push_str(&text_at(c, depth + 1, tables));
                } else {
                    out.push_str(c);
                }
                out.push_str(p);
            }
            return out;
        }
    }
    msg.to_string()
}

/// 응답 JSON 안의 "error", "reason", "warnings"를 바꾼 사본.
pub fn value_in(v: &Value, tables: &[Table]) -> Value {
    value_at(v, 0, tables)
}

fn value_at(v: &Value, depth: usize, tables: &[Table]) -> Value {
    // shortcut: 깊이 끝 아래는 바꾸지 않고 복사한다. 요청 JSON은 파서가 128단계까지만 받으므로 복사는 안전하다.
    if depth >= MAX_VALUE_DEPTH {
        return v.clone();
    }
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .map(|(k, x)| {
                    let y = match (k.as_str(), x) {
                        // 모델이 적은 글(`summary`, `reasons`, 가닥의 `model_reason`)은 키가 이 목록에 없어 바뀌지 않는다
                        ("error" | "reason" | "scene_error" | "draft_error", Value::String(s)) => Value::String(text_in(s, tables)),
                        // `notes`: 서버가 만든 노트 글(주소 없는 신호, 바퀴 거리, 시뮬레이션에서 빠진 `mech` 장치). 표에 없는 노트는 그대로 영어로 남는다.
                        ("warnings" | "io_problems" | "notes", Value::Array(a)) => Value::Array(a.iter().map(|w| w.as_str().map_or_else(|| value_at(w, depth + 1, tables), |s| Value::String(text_in(s, tables)))).collect()),
                        // 래더 모니터의 "unread": 장치 이름 → 읽지 않는 이유. 이름(키)은 그대로 둔다.
                        // 장면 저장소의 "unreadable": 읽지 못한 파일(scene, draft) → 까닭
                        ("unreadable", Value::Object(o)) => Value::Object(o.iter().map(|(n, w)| (n.clone(), w.as_str().map_or_else(|| value_at(w, depth + 1, tables), |s| Value::String(text_in(s, tables))))).collect()),
                        ("unread", Value::Object(o)) => Value::Object(o.iter().map(|(n, w)| (n.clone(), w.as_str().map_or_else(|| value_at(w, depth + 1, tables), |s| Value::String(text_in(s, tables))))).collect()),
                        _ => value_at(x, depth + 1, tables),
                    };
                    (k.clone(), y)
                })
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(|x| value_at(x, depth + 1, tables)).collect()),
        _ => v.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_fill_the_places_in_order() {
        assert_eq!(captures("a {} b {}", "a X b Y"), Some(vec!["X", "Y"]));
        assert_eq!(captures("a {} b", "a X c"), None);
        assert_eq!(captures("exact", "exact"), Some(vec![]));
        assert_eq!(captures("exact", "exact more"), None);
    }

    // 3D 쪽 표만으로 하는 시험: 세 장면 문장 하나와 깊이 한도. PLC 쪽 표와 함께 하는 시험은 호스트(`src/page_ko.rs`)에 있다.
    #[test]
    fn the_scene_table_translates_and_a_text_over_the_limit_stays() {
        let t: &[Table] = &[KO];
        assert_eq!(text_in("scene.schema: must be 1 or 2", t), "scene.schema는 1 또는 2여야 합니다");
        assert_eq!(text_in("not in any table", t), "not in any table");
        let big = "cannot read a: ".repeat(4400);
        assert_eq!(text_in(&big, t), big);
        // 객체 안쪽의 "reason"도 바뀐다
        let deep = serde_json::json!({"error": "scene.schema: must be 1 or 2", "x": {"reason": "scene.schema: must be 1 or 2"}});
        assert_eq!(value_in(&deep, t)["x"]["reason"], "scene.schema는 1 또는 2여야 합니다");
    }
}
