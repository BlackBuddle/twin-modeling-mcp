// 소개 페이지의 3D 재생기: 기록된 시뮬레이션 한 사이클(docs/replay/<name>.json)을 제품 화면과 같은 렌더러로 그린다.
// 동기화: docs/vendor/의 파일은 crates/twin-page/ui/의 twin-scene.js, twin-core.js, vendor/three/의 복사본이다. 원본이 바뀌면 다시 복사하고,
// 렌더러가 읽는 상태 모양(stateFrom)이나 모델 형식이 바뀌면 docs/replay/*.json도 서버로 다시 만든다.
// 이 파일은 "3D로 보기"를 누를 때만 index.html의 스크립트가 import한다. 서버와 말하지 않는다: 같은 출처의 JSON 하나만 읽는다.
import { createTwin, dict } from '../vendor/twin-scene.js';
import { stateFrom } from '../vendor/twin-core.js';

const $ = id => document.getElementById(id);
const ko = () => document.documentElement.lang === 'ko';
// 그림 안의 글 (twin-model.html의 r.* 글과 같다)
const TEXT = {
  'r.io': ['{label} I/O', '{label} 입출력'], 'r.on': ['on', '켜짐'], 'r.off': ['off', '꺼짐'], 'r.unknown': ['unknown', '모름'],
  'r.forced.on': ['on, forced (not written by the rule)', '켜짐, 강제로 켬 (규칙이 쓰지 않음)'], 'r.forced.off': ['off, forced (not written by the rule)', '꺼짐, 강제로 끔 (규칙이 쓰지 않음)'], 'r.forced': ['forced', '강제'],
  'r.canvas': ['Isometric drawing of the example model, playing one recorded simulation cycle.', '예제 모델의 등각 그림입니다. 기록된 시뮬레이션 한 사이클을 재생합니다.'],
  'r.stack.n': ['{label} {n}', '{label} {n}개'], 'r.stack.empty': ['{label} empty', '{label} 비었음'], 'r.count': ['{n}', '{n}개'], 'r.left': ['outside the rules', '규칙 밖'],
  'r.axis': ['Drawn at the start values', '시작 값으로 그림'], 'r.lamp.out': ['{addr} not in the model (drawn off)', '{addr} 모델에 없는 핀 (늘 꺼짐으로 그림)'], 'r.alarm': ['alarm', '알람'],
};
const T = (k, v) => { let s = TEXT[k] ? TEXT[k][ko() ? 1 : 0] : k; if (v) for (const n in v) s = s.split('{' + n + '}').join(v[n]); return s; };

const view = $('rpView'), pic = view.querySelector('picture');
const pinKind = dict();
// 렌더러는 색을 문서 뿌리의 CSS 값에서 한 번 읽는다. 그림 칸은 어두운 테마에서도 밝은 종이라서, 만들 때만 밝은 값을 쓴다
const rs = document.documentElement.style;
rs.setProperty('--coil', '#3e63dd'); rs.setProperty('--sensor', '#30a46c');
const twin = createTwin(view, { text: T, ioKind: a => pinKind[a] || 'd' });
rs.removeProperty('--coil'); rs.removeProperty('--sensor');

let D = null, full = [], name = '', t = 0, playing = false, speed = 1, shown = -1, raf = 0, last = 0, onScreen = true, gen = 0;
const step = () => D.step_ms, total = () => D.total_ms, live = () => onScreen && !document.hidden;

// 파일의 줄은 앞 줄과 다른 칸만 가진다. 핀 값은 머리의 `bits` 차례대로 "0"/"1" 글자다
function decode(d) { let prev = null; return d.states.map(s => (prev = prev ? { ...prev, ...s } : { ...s })); }
function toState(f) {
  const bits = dict(), words = dict();
  D.bits.forEach((k, i) => { bits[k] = !!f.b && f.b[i] === '1'; });
  (D.words || []).forEach((k, i) => { words[k] = f.w ? f.w[i] : 0; });
  return stateFrom({ bits, words, plant: { axes: f.axes, joints: f.joints, parts: f.parts, forced: f.forced, levels: f.levels, alarms: f.alarms } });
}
// 멈춘 시각(시험과 되감기): 두 기록 사이를 곧게 잇는다. 부품은 같은 상태일 때만 잇는다
// shortcut: 회전 관절의 360° 넘김은 잇지 않는다(재생 중에는 렌더러가 가까운 쪽으로 잇는다); 넘김이 있는 예제가 생기면 고친다
function at(ms) {
  const i = Math.min(full.length - 1, Math.floor(ms / step())), j = Math.min(full.length - 1, i + 1), k = ms / step() - i, a = full[i], b = full[j];
  if (i === j || k <= 0) return a;
  const L = (x, y) => (typeof x === 'number' && typeof y === 'number' ? x + (y - x) * k : x);
  const mix = (x, y) => (Array.isArray(x) ? x.map((v, n) => mix(v, y && y[n])) : x && typeof x === 'object' ? Object.fromEntries(Object.entries(x).map(([q, v]) => [q, mix(v, y && y[q])])) : L(x, y));
  const parts = a.parts && a.parts.map(p => { const q = (b.parts || []).find(r => r.id === p.id && r.state === p.state); return q ? { ...p, x: L(p.x, q.x), y: L(p.y, q.y), z: L(p.z, q.z) } : p; });
  return { ...a, axes: mix(a.axes, b.axes), joints: mix(a.joints, b.joints), parts };
}

// 모델 사본: 핀이 곧 주소다 (twin-model.html의 withPins와 같다)
function withPins(sc, cat) {
  const c = structuredClone(sc);
  for (const k of Object.keys(pinKind)) delete pinKind[k];
  for (const d of c.devices || []) {
    const io = (cat.kinds[d.kind] || {}).io || {};
    d.io = {}; for (const [n, s] of Object.entries(io)) { d.io[n] = d.id + '.' + n; pinKind[d.id + '.' + n] = s.type === 'word' ? 'd' : s.dir === 'out' ? 'y' : 'x'; }
    for (const s of d.signals || []) { s.device = d.id + '.' + s.name; pinKind[s.device] = s.type && s.type !== 'bit' ? 'd' : s.dir === 'out' ? 'y' : 'x'; }
  }
  return c;
}

// ---- 조작 ----
const f1 = ms => (ms / 1000).toFixed(1);
function paint() {
  $('rpTimeEn').textContent = `${f1(t)} of ${f1(total())} s`;
  $('rpTimeKo').textContent = `${f1(total())}초 중 ${f1(t)}초`;
  const on = playing;
  $('rpPlay').setAttribute('aria-pressed', String(on));
  $('rpPlayEn').textContent = on ? 'Pause' : 'Play'; $('rpPlayKo').textContent = on ? '일시 정지' : '재생';
  $('rpIcPlay').toggleAttribute('hidden', on); $('rpIcPause').toggleAttribute('hidden', !on); // SVG 요소에는 hidden 속성 손잡이가 없다
}
function frame(now) {
  raf = 0; if (!playing || !live()) return;
  t = Math.min(total(), t + Math.min(100, now - last) * speed); last = now; // 프레임이 멈췄다 오면(느린 기계, 막힌 탭) 건너뛰지 않고 이어 간다
  const i = Math.floor(t / step());
  // 다음 기록을 향해, 그 기록의 시각에 닿도록 렌더러가 잇는다
  if (i !== shown) { shown = i; twin.setState(toState(full[Math.min(full.length - 1, i + 1)]), Math.max(16, (step() * (i + 1) - t) / speed)); }
  if (t >= total()) playing = false;
  paint(); if (playing) raf = requestAnimationFrame(frame);
}
function run() { if (!raf && playing && live()) { last = performance.now(); raf = requestAnimationFrame(frame); } }
function seek(ms) { t = Math.max(0, Math.min(total(), ms)); shown = Math.floor(t / step()); twin.setState(toState(at(t)), 0); paint(); }
function play(on) { if (on && t >= total()) seek(0); playing = on; paint(); run(); }

$('rpPlay').onclick = () => play(!playing);
$('rpRestart').onclick = () => { seek(0); play(true); };
$('rpSpeed').onchange = () => { speed = Number($('rpSpeed').value) || 1; shown = -1; };

// 그림 칸이 화면 밖이거나 탭이 숨으면 그리지 않는다. 시계도 멈춘다(돌아오면 이어서)
new IntersectionObserver(es => { onScreen = es[es.length - 1].isIntersecting; twin.active(live()); if (live()) run(); }).observe(view);
document.addEventListener('visibilitychange', () => { twin.active(live()); if (live()) run(); });

export async function open(n) {
  const my = ++gen;
  const r = await fetch(`replay/${encodeURIComponent(n)}.json`);
  if (!r.ok) throw new Error('HTTP ' + r.status);
  const d = await r.json();
  if (my !== gen) return;
  playing = false; D = d; name = n; full = decode(d); t = 0; shown = -1;
  twin.load(withPins(d.scene, d.catalog), d.catalog, d.links, true, d.resolved);
  twin.setState(toState(full[0]), 0);
  pic.hidden = true; play(true);
}
// 시험 손잡이: 멈추고 그 시각으로 간다
window.__replay = { seek: ms => { playing = false; seek(ms); }, play: () => play(true), state: () => ({ name, t, playing, speed, total: D && total() }), twin };
