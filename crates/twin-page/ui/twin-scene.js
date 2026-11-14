// 3D 공정 화면의 장면 코드 (Anatomy 모양, U29). `/vendor/twin-scene.js`로 나간다.
// 정사영 등각 카메라, 4단 toon 음영, 잉크 윤곽선, 단면, 공기 흐름 점. 그림자 맵·환경 맵·PBR 재질 없음. 요청할 때만 그린다.
// 장면은 API의 장치·종류 목록(크기, 부품, 포트)·링크로 만든다. PLC에는 닿지 않는다.
import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { TransformControls } from 'three/addons/controls/TransformControls.js';

const rad = THREE.MathUtils.degToRad, clamp = THREE.MathUtils.clamp;
const num = (v, d) => (typeof v === 'number' && isFinite(v) ? v : d);
const TRI_CAP = 60000; // 모델 하나의 삼각형 상한 (F38). 넘으면 윤곽선을 더 만들지 않고 콘솔에 알린다
const EDGE_DEG = 30; // 이보다 날카로운 모서리만 윤곽선 (F38)
const TONES = { hi: '--a-hi', mid: '--a-mid', lo: '--a-lo', faint: '--a-faint' };
const VARS = ['--a-top', '--a-s3', '--a-stop', '--a-hi', '--a-mid', '--a-lo', '--a-faint', '--a-lit', '--a-halo', '--coil', '--sensor', '--alarm', '--exhaust', '--motion', '--lamp-g', '--lamp-a'];
const LAMP = { red: '--alarm', green: '--lamp-g', yellow: '--lamp-a', blue: '--coil', white: '--a-mid' };
const NONE = [];
for (const t of [TONES, LAMP]) Object.setPrototypeOf(t, null); // 장면의 값으로 찾는 표 (AR11)
// 장치 id, 틀 이름, 주소로 찾는 표는 프로토타입이 없는 객체다. `__proto__`나 `constructor` 같은 이름이 와도 엉뚱한 값이 나오지 않는다 (AR2).
export const dict = o => Object.assign(Object.create(null), o);
// 외형 색조 이름표 (A5b). `catalog.tones`와 같아야 한다 (시험이 둘을 견준다). 장면의 값은 이 목록에서 이름으로만 찾는다
export const SKIN_TONES = ['metal', 'black', 'dark', 'white', 'orange', 'green', 'red', 'blue', 'yellow', 'clear', 'mark'];
const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
const el = (tag, cls, text) => { const e = document.createElement(tag); if (cls) e.className = cls; if (text != null) e.textContent = text; return e; };

export const ioKind = a => (a[0] === 'X' ? 'x' : a[0] === 'Y' ? 'y' : a[0] === 'M' ? 'm' : 'd');
// 그림 안의 글 (TW215): 쪽이 `opts.text(키, 값)`으로 바꾼다. 기본값은 이 표의 한국어이고, 기본 값일 때 plc-mcp 화면의 글자는 그대로다.
const KO_TEXT = {
  'r.io': '{label} 입출력', 'r.on': '켜짐', 'r.off': '꺼짐', 'r.unknown': '모름',
  'r.forced.on': '켜짐, 강제로 켬 (규칙이 쓰지 않음)', 'r.forced.off': '꺼짐, 강제로 끔 (규칙이 쓰지 않음)', 'r.forced': '강제',
  'r.canvas': '공정 등각 그림. 장치 목록은 메뉴에 있습니다.', 'r.stack.n': '{label} {n}개', 'r.stack.empty': '{label} 비었음', 'r.count': '{n}개',
  'r.left': '규칙 밖', 'r.axis': '축 위치: 읽지 않음 (원점 위치로 그림)', 'r.lamp.out': '{addr} 장면에 없는 주소 (늘 꺼짐으로 그림)', 'r.alarm': '알람',
};
export const koText = (k, v) => { let s = KO_TEXT[k] ?? k; if (v) for (const n in v) s = s.split('{' + n + '}').join(v[n]); return s; };
// 장치의 입출력 목록: 종류의 칸 순서, 그다음 신호. {slot, addr, out, word}
export function ioList(dev, K) {
  const out = [];
  for (const [slot, s] of Object.entries((K && K.io) || {})) { const a = dev.io && dev.io[slot]; if (a) out.push({ slot, addr: a, out: s.dir === 'out', word: s.type === 'word' }); }
  for (const s of dev.signals || []) out.push({ slot: s.name, addr: s.device, out: s.dir === 'out', word: !!s.type && s.type !== 'bit' });
  // 줄의 순서: X, Y, M, D, 그 안에서는 번호순 (실린더 A: X0 X1 Y10 Y11)
  const rank = a => 'XYMD'.indexOf(a[0]) * 1e6 + (parseInt(a.slice(1), a[0] === 'X' || a[0] === 'Y' ? 16 : 10) || 0);
  return out.sort((p, q) => rank(p.addr) - rank(q.addr));
}
// 입출력 램프 줄 (F42): 주소 + 램프. 켜짐 = 채운 원, 꺼짐 = 빈 원, 아직 읽지 못함 = 점선 원. 글자도 함께 바뀐다
export function lampRow(ios, cls, label, o = {}) {
  const T = o.text || koText, IK = o.ioKind || ioKind;
  const box = el('span', cls); box.setAttribute('role', 'group'); box.setAttribute('aria-label', T('r.io', { label }));
  const items = ios.map(io => { const it = el('span', 'sl ' + IK(io.addr)); it.setAttribute('role', 'img'); it.append(el('i', 'lp'), el('span', '', io.addr)); const val = io.word ? it.appendChild(el('b')) : null; box.append(it); return { io, it, val }; });
  return { box, items };
}
export function lampPaint(items, st, o = {}) {
  const T = o.text || koText;
  for (const { io, it, val } of items) {
    const known = io.word ? io.addr in st.words : io.addr in st.bits, on = !io.word && !!st.bits[io.addr];
    const fr = !io.word && Array.isArray(st.forced) && st.forced.includes(io.addr); // 규칙이 쓰지 않는 입력 (TW172): 겹선 테두리와 "강제"
    const text = fr ? `${io.addr} ${T(on ? 'r.forced.on' : 'r.forced.off')}` : `${io.addr} ${!known ? T('r.unknown') : io.word ? st.words[io.addr] : on ? T('r.on') : T('r.off')}`;
    if (it.title === text) continue;
    it.classList.toggle('fr', fr); const fm = it.querySelector('.fm'); if (fr && !fm) it.children[1].after(el('span', 'fm', T('r.forced'))); else if (!fr && fm) fm.remove();
    it.classList.toggle('on', on); it.classList.toggle('na', !known); it.title = text; it.setAttribute('aria-label', text);
    if (val) val.textContent = known ? ' ' + st.words[io.addr] : '';
  }
}

export function createTwin(host, opts = {}) {
  const T = opts.text || koText, LO = { text: T, ioKind: opts.ioKind || ioKind }; // 그림 안의 글과 램프 색 (TW215)
  const cssv = n => getComputedStyle(document.documentElement).getPropertyValue(n).trim();
  const C = {}; for (const k of VARS) C[k] = new THREE.Color(cssv(k));
  const black = new THREE.Color(0);
  const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
  renderer.setClearColor(0, 0); renderer.setPixelRatio(Math.min(devicePixelRatio, 1.5)); renderer.localClippingEnabled = true;
  const cv = renderer.domElement; host.prepend(cv);
  cv.setAttribute('role', 'img'); cv.setAttribute('aria-label', T('r.canvas'));
  const scene = new THREE.Scene(), world = new THREE.Group(); scene.add(world);
  // 정사영 카메라: 방위 45°, 고도 30° (Anatomy craft.md 6절). 사진 맞춤 때만 원근 카메라
  const ortho = new THREE.OrthographicCamera(-1, 1, 1, -1, 1, 400000), persp = new THREE.PerspectiveCamera(50, 1, 10, 400000);
  let camera = ortho;
  const controls = new OrbitControls(camera, cv);
  controls.minZoom = 0.4; controls.maxZoom = 8; controls.maxPolarAngle = Math.PI * 0.49;
  controls.addEventListener('change', request);
  // 빛: 주변광 + 방향광 1개. toon 4단 = Anatomy shade-0..3 (윗면이 가장 밝음)
  const sun = new THREE.DirectionalLight(0xffffff, 0.284 * Math.PI); sun.position.set(-20, 60, 35);
  scene.add(new THREE.AmbientLight(0xffffff, 0.716 * Math.PI), sun);
  const grad = new THREE.DataTexture(new Uint8Array([0, 57, 121, 255]), 4, 1, THREE.RedFormat);
  grad.minFilter = grad.magFilter = THREE.NearestFilter; grad.needsUpdate = true;
  const canvasTex = (size, draw) => { const c = document.createElement('canvas'); c.width = c.height = size; draw(c.getContext('2d')); return new THREE.CanvasTexture(c); };
  const haloTex = canvasTex(64, g => { const r = g.createRadialGradient(32, 32, 4, 32, 32, 32); r.addColorStop(0, '#fff'); r.addColorStop(1, 'rgba(255,255,255,0)'); g.fillStyle = r; g.fillRect(0, 0, 64, 64); });
  const dotTex = canvasTex(16, g => { g.fillStyle = '#fff'; g.beginPath(); g.arc(8, 8, 7, 0, 7); g.fill(); });
  const dots = n => { const geo = new THREE.BufferGeometry(); geo.setAttribute('position', new THREE.Float32BufferAttribute(new Float32Array(n * 3), 3)); const p = new THREE.Points(geo, new THREE.PointsMaterial({ size: 5, sizeAttenuation: false, map: dotTex, alphaTest: 0.5, transparent: true, depthTest: false })); p.visible = false; p.renderOrder = 5; p.frustumCulled = false; p.raycast = () => {}; return p; };

  let devs = dict(), st = { bits: dict(), words: dict(), axes: dict(), lights: dict(), levels: dict(), alarms: [] };
  const parts = [], tubes = [], strips = [], tw = dict(), tj = dict(); // tj: 엔진 관절 값의 이음 (장치 → 관절 → {from, to, cur, t0, dur})
  let sel = null, flowAll = false, cutAll = false, stripsAll = false, ghostAll = false, active = true, pending = false, frames = 0, blink = true, blinkT = 0, fitX = 400, fitY = 300, inset = { right: 0, bottom: 0 };
  let wp = dict(), wpMesh = [], wpSize = 40; // 부품(workpiece): id → {from, to, cur, t0, dur}, 그리는 상자들, 한 변 mm
  // 형식 2 공작물 (milestone B, TW168~TW178): 종류·지점·매거진, 부품마다 그룹, 화면 위의 꼬리표·괄호·지점 이름
  let W2 = null, pD = null, wkOn = false, ptsAll = false, selPart = null, hovPart = null, pw = dict(), wkInfo = null, ptsObj = null, addrDev = dict();
  const wkEls = [], pbEls = [], ptEls = [], wkW = new Map();
  const seeM = new THREE.LineBasicMaterial({ color: new THREE.Color(0x8a8a8f), depthFunc: THREE.GreaterDepth, depthWrite: false }); // TW175: 앞에 뭔가 있는 곳에서만 보이는 윤곽
  const fenceM = new THREE.LineDashedMaterial({ color: new THREE.Color(0x18181a), dashSize: 4, gapSize: 3 }); // TW169: 규칙 밖 부품의 점선 울타리
  const contactM = new THREE.MeshBasicMaterial({ color: new THREE.Color(0x18181a), transparent: true, opacity: 0.22, depthWrite: false, polygonOffset: true, polygonOffsetFactor: -1, polygonOffsetUnits: -1 }); // TW168: 닿는 자리 표시
  const pointM = new THREE.LineBasicMaterial({ color: new THREE.Color(0x18181a), depthTest: false, transparent: true }); // TW173: 지점 십자는 모든 것 위에
  const v1 = new THREE.Vector3(), v2 = new THREE.Vector3();
  // 모델 위치(고스트, F24): 움직이는 부분의 모서리만 점선으로 그린 복사본. 색만이 아니라 점선으로도 구별된다
  const ghostMat = new THREE.LineDashedMaterial({ dashSize: 5, gapSize: 4, transparent: true, opacity: 0.85, depthTest: false });
  function ghostOf(src) {
    const g = new THREE.Group();
    for (const o of src.children) { if (!o.isMesh) continue; const l = new THREE.LineSegments(new THREE.EdgesGeometry(o.geometry, EDGE_DEG), ghostMat); l.computeLineDistances(); l.position.copy(o.position); l.renderOrder = 4; l.raycast = () => {}; g.add(l); }
    g.visible = false; return g;
  }
  const ghostOn = (D, ax) => ax.ghost != null && (ghostAll || Math.abs(ax.ghost - ax.pos) > num(par(D, 'window_mm'), 2));

  // ---- 도형: 재질 없이 만들고, 모든 메시는 restyle 하나를 지난다 (F38) ----
  const mesh = (geo, o = {}) => { const m = new THREE.Mesh(geo); if (o.pos) m.position.set(...o.pos); const { parent, pos, ...u } = o; Object.assign(m.userData, u); parent.add(m); return m; };
  const box = (w, h, d, o) => mesh(new THREE.BoxGeometry(w, h, d), o);
  const cyl = (r, h, axis, o, seg = 28) => { const g = new THREE.CylinderGeometry(r, r, h, seg); if (axis === 'x') g.rotateZ(Math.PI / 2); if (axis === 'z') g.rotateX(Math.PI / 2); return mesh(g, { round: true, ...o }); };
  // 어떤 모델이든(기본 도형, 나중의 CAD·로봇) 같은 모양으로: toon 재질 + 윤곽선 + 둥근 부품의 실루엣 껍질
  function restyle(root, D) {
    const list = []; root.traverse(o => { if (o.isMesh && !o.userData.raw) list.push(o); });
    let tris = 0, over = false;
    for (const o of list) {
      const u = o.userData, g = o.geometry; tris += (g.index ? g.index.count : g.attributes.position.count) / 3;
      let plane = null;
      if (u.clip && D) { let q = o.parent; while (q && !q.userData.size) q = q.parent; if (q) { plane = q.userData.plane || (q.userData.plane = new THREE.Plane()); if (!D.planes.includes(q)) D.planes.push(q); } }
      const mat = o.material = new THREE.MeshToonMaterial({ gradientMap: grad, side: plane ? THREE.DoubleSide : THREE.FrontSide, transparent: !!u.glass, opacity: u.glass ? 0.3 : 1, depthWrite: !u.glass });
      const P = { m: o, mat, lm: null, hm: null, tone: u.tone || 'mid', fill: u.fill || '--a-top', role: u.role, slot: u.slot, lamp: u.lamp, plane, state: null };
      if (tris <= TRI_CAP) {
        P.lm = new THREE.LineBasicMaterial(); const l = new THREE.LineSegments(new THREE.EdgesGeometry(g, u.edge || EDGE_DEG), P.lm); l.raycast = () => {}; o.add(l);
        if (u.round && g.attributes.normal) { // 실루엣: 법선 방향으로 0.7 mm 부풀린 뒷면 껍질
          const hg = g.clone(), p = hg.attributes.position, n = hg.attributes.normal;
          for (let i = 0; i < p.count; i++) p.setXYZ(i, p.getX(i) + n.getX(i) * 0.7, p.getY(i) + n.getY(i) * 0.7, p.getZ(i) + n.getZ(i) * 0.7);
          P.hm = new THREE.MeshBasicMaterial({ side: THREE.BackSide }); const h = new THREE.Mesh(hg, P.hm); h.userData.raw = true; h.raycast = () => {}; o.add(h);
        }
      } else if (!over) { over = true; console.warn(`twin: model ${D ? D.id : ''} has more than ${TRI_CAP} triangles; the rest gets no outlines`); }
      parts.push(P); if (D) D.parts.push(P);
    }
  }
  function paint(P) {
    const s = P.state;
    P.mat.color.copy(C[P.fill] || C['--a-top']); P.mat.emissive.copy(black); P.mat.emissiveIntensity = 0;
    let line = C[TONES[P.tone]];
    if (s === 'lit') line = C['--a-lit'];
    else if (s === 'coil') { P.mat.emissive.copy(C['--coil']); P.mat.emissiveIntensity = 0.55; line = C['--coil']; }
    else if (s === 'sensor') { P.mat.color.copy(C['--sensor']); P.mat.emissive.copy(C['--sensor']); P.mat.emissiveIntensity = 0.6; line = C['--sensor']; }
    else if (s === 'alarm') { P.mat.emissive.copy(C['--alarm']); P.mat.emissiveIntensity = 0.22; line = C['--alarm']; }
    else if (s === 'liquid') { P.mat.color.copy(C['--coil']).lerp(C['--a-top'], 0.55); line = C['--coil']; }
    else if (s && C[s]) { P.mat.color.copy(C[s]); P.mat.emissive.copy(C[s]); P.mat.emissiveIntensity = 0.7; } // 램프: 제 색
    if (P.lm) P.lm.color.copy(line); if (P.hm) P.hm.color.copy(line);
  }

  // ---- 종류별 도형: 동작(behaviors)으로 고른다. 치수는 종류 목록의 부품 크기 [x,y,z] mm 안에서 ----
  const size = q => q.userData.size;
  const first = D => Object.values(D.pg)[0];
  const par = (D, k) => { const p = D.dev.params && D.dev.params[k]; return p != null ? p : (D.K.params && D.K.params[k] || {}).default; };

  // 전자 밸브(실린더의 한 부품): 몸체, 코일, 스풀 (단면에서 보임)
  function bValve(D) {
    const q = D.pg.valve; if (!q) return;
    const [L, H, W] = size(q), c = { parent: q, tone: 'hi' };
    box(L * 0.92, H * 0.1, W * 0.92, { parent: q, tone: 'mid', pos: [0, H * 0.05, 0] });
    box(L * 0.46, H * 0.6, W * 0.8, { ...c, clip: true, pos: [0, H * 0.4, 0] });
    const coils = [['coil_fwd', -1], ['coil_back', 1]].filter(([s]) => D.K.io[s]);
    for (const [slot, side] of coils) {
      box(L * 0.25, H * 0.64, W * 0.62, { ...c, pos: [side * L * 0.355, H * 0.42, 0], role: 'coil', slot });
      box(L * 0.1, H * 0.1, W * 0.3, { parent: q, tone: 'mid', pos: [side * L * 0.355, H * 0.79, 0] });
    }
    if (coils.length === 1) cyl(H * 0.2, L * 0.16, 'x', { parent: q, tone: 'mid', pos: [L * 0.31, H * 0.4, 0] }); // 복귀 스프링 쪽 마개
    const spool = new THREE.Group(); spool.position.set(0, H * 0.4, 0); q.add(spool);
    cyl(H * 0.07, L * 0.44, 'x', { parent: spool, tone: 'hi' }, 14);
    for (const lx of [-0.14, 0, 0.14]) cyl(H * 0.19, L * 0.07, 'x', { parent: spool, tone: 'hi', pos: [lx * L, 0, 0] }, 18);
    D.upd.push(s => { const sp = (s.axes[D.id] || {}).spool; spool.position.x = sp === 'fwd' ? -L * 0.035 : sp === 'back' ? L * 0.035 : 0; }); // 켜진 코일 쪽으로
  }
  // 공압 실린더·그리퍼: 캡, 배럴, 피스톤과 로드, 리드 센서, 복귀 스프링(단동), 방 안의 공기 점, 움직임 화살표
  function bCylinder(D) {
    const q = D.pg.body || first(D), [L, H, W] = size(q), t = Math.min(18, L * 0.09), r = Math.min(H, W) * 0.42, y = H / 2, c = { parent: q, tone: 'hi', clip: true };
    box(t, H, W, { ...c, pos: [-L / 2 + t / 2, y, 0] }); box(t, H, W, { ...c, pos: [L / 2 - t / 2, y, 0] });
    cyl(r, L - 2 * t, 'x', { ...c, pos: [0, y, 0] });
    const stroke = num(par(D, 'stroke_mm'), 50), inner = L - 2 * t, pt = Math.max(5, inner * 0.06), travel = Math.max(1, Math.min(stroke, inner - pt - 10)), x0 = -L / 2 + t + 5 + pt / 2;
    const rs = D.pg.rod && size(D.pg.rod), out = rs ? rs[0] * 0.35 : L * 0.1, rr = rs ? rs[1] / 2 : r * 0.36, rl = L / 2 - x0 + out;
    const rod = new THREE.Group(); rod.position.set(x0, y, 0); q.add(rod);
    cyl(r * 0.93, pt, 'x', { parent: rod, tone: 'hi' });
    cyl(rr, rl, 'x', { parent: rod, tone: 'hi', pos: [rl / 2, 0, 0] });
    box(6, r * 2.2, r * 2.6, { parent: rod, tone: 'hi', pos: [rl + 3, 0, 0] });
    const ghost = ghostOf(rod); q.add(ghost);
    for (const [slot, sx] of [['sensor_back', -L / 2 + t + 14], ['sensor_fwd', L / 2 - t - 14]]) {
      if (!D.K.io[slot]) continue;
      box(22, 6, 8, { parent: q, tone: 'mid', pos: [sx, y + r + 3, 0] });
      cyl(2.6, 2.4, 'y', { parent: q, tone: 'mid', pos: [sx, y + r + 7.2, 0], role: 'led', slot });
    }
    // 단동: 복귀 스프링 (단면에서만 보임). 고리 7개의 간격만 바꾼다 (늘이는 배율을 쓰면 선 굵기까지 늘어난다)
    const spring = D.K.io.coil_back ? [] : Array.from({ length: 7 }, () => mesh(new THREE.TorusGeometry(r * 0.68, 1.1, 8, 28).rotateY(Math.PI / 2), { parent: q, tone: 'mid', edge: 80, round: true, pos: [0, y, 0] }));
    const air = [dots(24), dots(24)], seed = Array.from({ length: 24 }, () => [Math.random(), Math.random() * 2 - 1, Math.random() * 2 - 1]);
    q.add(...air);
    const am = new THREE.MeshBasicMaterial(), arrow = new THREE.Group(), u = H / 50;
    const sh = new THREE.Mesh(new THREE.CylinderGeometry(2.2 * u, 2.2 * u, 34 * u, 10), am); sh.rotation.z = Math.PI / 2;
    const hd = new THREE.Mesh(new THREE.ConeGeometry(7 * u, 15 * u, 14), am); hd.rotation.z = -Math.PI / 2; hd.position.x = 24 * u;
    for (const m of [sh, hd]) { m.userData.raw = true; m.raycast = () => {}; }
    arrow.add(sh, hd); arrow.visible = false; q.add(arrow);
    const fill = (p, from, to) => { const a = p.geometry.attributes.position; for (let i = 0; i < 24; i++) a.setXYZ(i, from + (to - from) * seed[i][0], y + seed[i][1] * r * 0.55, seed[i][2] * r * 0.55); a.needsUpdate = true; };
    D.upd.push((s, F) => {
      const ax = s.axes[D.id] || {}, k = clamp(num(ax.draw, 0) / stroke, 0, 1), px = x0 + k * travel, back = ax.spool === 'back';
      rod.position.x = px;
      ghost.visible = ghostOn(D, ax); if (ghost.visible) ghost.position.set(x0 + clamp(num(ax.gdraw, 0) / stroke, 0, 1) * travel, y, 0);
      spring.forEach((m, i) => { m.visible = F.cut; m.position.x = px + pt / 2 + (i + 0.5) / spring.length * (L / 2 - t - px - pt / 2); });
      arrow.visible = !!ax.moving; am.color.copy(C['--motion']); arrow.rotation.y = back ? Math.PI : 0;
      arrow.position.set(px + rl + 36 * u + (back ? 10 * u : 0), y + r + 22 * u, 0);
      const show = F.flow && F.cut && !!ax.moving;
      air[0].visible = show; air[1].visible = show && !!D.K.io.coil_back;
      if (show) { fill(air[0], -L / 2 + t + 3, px - pt / 2 - 3); fill(air[1], px + pt / 2 + 3, L / 2 - t - 3); air[0].material.color.copy(C[back ? '--exhaust' : '--coil']); air[1].material.color.copy(C[back ? '--coil' : '--exhaust']); }
    });
  }
  // 로터리 액추에이터: 몸체 + 앞면의 회전 팔
  function bSwing(D) {
    const q = D.pg.body || first(D), [L, H, W] = size(q), r = Math.min(H, W) * 0.32;
    box(L, H, W, { parent: q, tone: 'hi', pos: [0, H / 2, 0] });
    const arm = new THREE.Group(); arm.position.set(L / 2, H / 2, 0); q.add(arm);
    cyl(r, 8, 'x', { parent: arm, tone: 'hi', pos: [4, 0, 0] });
    box(6, r * 2.2, r * 0.5, { parent: arm, tone: 'hi', pos: [11, r * 0.7, 0] });
    const ghost = ghostOf(arm); ghost.position.copy(arm.position); q.add(ghost);
    D.upd.push(s => { const ax = s.axes[D.id] || {}; arm.rotation.x = rad(num(ax.draw, 0)); ghost.visible = ghostOn(D, ax); ghost.rotation.x = rad(num(ax.gdraw, 0)); });
  }
  function bMotor(D) {
    const q = first(D), [L, H, W] = size(q);
    box(L * 0.7, H * 0.14, W * 0.8, { parent: q, tone: 'mid', pos: [-L * 0.12, H * 0.07, 0] });
    cyl(Math.min(H, W) * 0.4, L * 0.72, 'x', { parent: q, tone: 'hi', pos: [-L * 0.12, H * 0.54, 0] });
    const shaft = new THREE.Group(); shaft.position.set(L * 0.24, H * 0.54, 0); q.add(shaft);
    cyl(H * 0.09, L * 0.26, 'x', { parent: shaft, tone: 'hi', pos: [L * 0.13, 0, 0] });
    box(L * 0.1, H * 0.32, H * 0.06, { parent: shaft, tone: 'hi', pos: [L * 0.2, H * 0.1, 0] }); // 도는 것이 보이는 키
    D.upd.push(s => { shaft.rotation.x = rad(num((s.axes[D.id] || {}).draw, 0)); });
  }
  function bTable(D) {
    const q = first(D), [L, H, W] = size(q), r = Math.min(L, W) / 2, n = Math.round(num(par(D, 'stations'), 6));
    cyl(r * 0.4, H * 0.75, 'y', { parent: q, tone: 'mid', pos: [0, H * 0.375, 0] });
    cyl(r * 0.96, H * 0.2, 'y', { parent: q, tone: 'hi', pos: [0, H * 0.85, 0] }, 48);
    for (let i = 0; i < n; i++) cyl(r * 0.1, H * 0.06, 'y', { parent: q, tone: 'mid', pos: [Math.cos(i / n * 6.2832) * r * 0.75, H * 0.98, Math.sin(i / n * 6.2832) * r * 0.75] }, 16);
  }
  function bBelt(D) {
    const q = D.pg.frame || first(D), [L, H, W] = size(q), pitch = 80, n = Math.max(1, Math.floor(L * 0.96 / pitch));
    box(L, H * 0.3, W, { parent: q, tone: 'mid', pos: [0, H * 0.75, 0] });
    for (const x of [-0.42, 0.42]) for (const z of [-0.4, 0.4]) box(W * 0.1, H * 0.6, W * 0.1, { parent: q, tone: 'lo', pos: [x * L, H * 0.3, z * W] });
    box(L * 0.98, H * 0.06, W * 0.84, { parent: q, tone: 'hi', pos: [0, H * 0.93, 0] });
    const marks = Array.from({ length: n }, () => box(4, 1.5, W * 0.84, { parent: q, tone: 'mid', pos: [0, H * 0.965, 0] })); // 벨트 위 줄: 움직임이 보임
    if (D.pg.motor) { const [a, b, c] = size(D.pg.motor); box(a, b, c, { parent: D.pg.motor, tone: 'hi', pos: [0, b / 2, 0] }); }
    D.upd.push(s => { const p = num((s.axes[D.id] || {}).draw, 0), span = n * pitch; marks.forEach((m, i) => { m.position.x = -span / 2 + (((i * pitch + p) % span) + span) % span; }); });
  }
  // 버튼, 셀렉터, 비상 정지: 받침 + 눌리는 머리. NC 접점은 비트가 꺼졌을 때 눌린 것이다
  function bSwitch(D) {
    const q = first(D), [L, H, W] = size(q), r = Math.min(L, W), big = D.dev.kind === 'estop', red = big || par(D, 'color') === 'red';
    box(L, H * 0.6, W, { parent: q, tone: 'mid', pos: [0, H * 0.3, 0] });
    cyl(r * 0.36, H * 0.1, 'y', { parent: q, tone: 'mid', pos: [0, H * 0.65, 0] });
    if (D.K.io.contact_1) {
      const knob = new THREE.Group(); knob.position.set(0, H * 0.7, 0); q.add(knob);
      cyl(r * 0.26, H * 0.2, 'y', { parent: knob, tone: 'hi', pos: [0, H * 0.1, 0] }); box(r * 0.12, H * 0.08, r * 0.5, { parent: knob, tone: 'hi', pos: [0, H * 0.24, 0] });
      D.upd.push((s, F) => { knob.rotation.y = F.bit(D.dev.io && D.dev.io.contact_1) ? 0.6 : F.bit(D.dev.io && D.dev.io.contact_2) ? -0.6 : 0; });
      return;
    }
    const ch = H * (big ? 0.28 : 0.2), cy = H * 0.7 + ch / 2, cap = cyl(r * (big ? 0.46 : 0.27), ch, 'y', { parent: q, tone: 'hi', fill: red ? '--a-stop' : '--a-top', pos: [0, cy, 0] });
    D.upd.push(s => { cap.position.y = cy - (pressed(D, s) ? H * 0.08 : 0); });
  }
  function bLight(D) {
    const q = first(D), [L, H, W] = size(q), r = Math.min(L, W) / 2, slot = Object.keys(D.K.io)[0];
    if (D.K.io.coil_red) { // 타워 램프: 위에서부터 빨강, 노랑, 초록. 맨 위는 부저
      cyl(r * 0.25, H * 0.4, 'y', { parent: q, tone: 'mid', pos: [0, H * 0.2, 0] }, 16);
      [['coil_green', '--lamp-g'], ['coil_yellow', '--lamp-a'], ['coil_red', '--alarm']].forEach(([s, lamp], i) => cyl(r * 0.8, H * 0.17, 'y', { parent: q, tone: 'hi', pos: [0, H * (0.49 + i * 0.18), 0], role: 'lamp', slot: s, lamp }));
      cyl(r * 0.5, H * 0.05, 'y', { parent: q, tone: 'mid', pos: [0, H * 0.97, 0], role: 'lamp', slot: 'coil_buzzer', lamp: '--coil' });
    } else if (D.K.params && D.K.params.color) { // 표시등: 받침 + 둥근 머리 + 켜지면 빛 무리
      const colv = LAMP[par(D, 'color')] || '--lamp-g';
      cyl(r * 0.9, H * 0.4, 'y', { parent: q, tone: 'mid', pos: [0, H * 0.2, 0] });
      mesh(new THREE.SphereGeometry(r * 0.75, 24, 12, 0, Math.PI * 2, 0, Math.PI / 2), { parent: q, tone: 'hi', round: true, pos: [0, H * 0.4, 0], role: 'lamp', slot, lamp: colv });
      const sp = new THREE.Sprite(new THREE.SpriteMaterial({ map: haloTex, transparent: true, depthWrite: false, opacity: 0.55 })); sp.position.set(0, H * 0.55, 0); sp.scale.setScalar(r * 5.5); sp.visible = false; sp.raycast = () => {}; q.add(sp);
      D.sprites.push({ sp, slot, colv });
    } else for (const p of Object.values(D.pg)) { const [a, b, c] = size(p); (Math.abs(a - c) < 1 ? cyl(a / 2, b, 'y', { parent: p, tone: 'hi', pos: [0, b / 2, 0], role: 'lamp', slot, lamp: '--coil' }) : box(a, b, c, { parent: p, tone: 'hi', pos: [0, b / 2, 0], role: 'lamp', slot, lamp: '--coil' })); } // 부저, 진공 패드, 펌프: 켜지면 코일 파랑
  }
  function bSensor(D) {
    const q = first(D), [L, H, W] = size(q);
    box(L, H, W, { parent: q, tone: 'hi', pos: [0, H / 2, 0] });
    cyl(Math.min(L, W) * 0.22, 2.4, 'y', { parent: q, tone: 'mid', pos: [0, H + 1.2, 0], role: 'led', slot: 'output' }, 16);
  }
  function bTank(D) {
    const q = first(D), [L, H, W] = size(q), cap = Math.max(1e-9, num(par(D, 'capacity'), 100));
    box(L, H, W, { parent: q, tone: 'mid', glass: true, pos: [0, H / 2, 0] });
    const liq = box(L * 0.94, 1, W * 0.94, { parent: q, tone: 'mid', role: 'liquid' });
    D.upd.push(s => { const h = clamp(num(s.levels[D.id], 0) / cap, 0, 1) * H * 0.96; liq.visible = h > 0.5; liq.scale.y = Math.max(0.5, h); liq.position.y = h / 2 + 1; });
  }
  // 부품 공급기: 받침판과 네 기둥. 부품 받이: 바닥과 네 벽. 구역 센서: 센서 몸체와 바닥의 감지 구역 테두리 (F28)
  function bSource(D) {
    const q = first(D), [L, H, W] = size(q), t = Math.max(3, L * 0.07);
    box(L, H * 0.12, W, { parent: q, tone: 'hi', pos: [0, H * 0.06, 0] });
    for (const x of [-1, 1]) for (const z of [-1, 1]) box(t, H, t, { parent: q, tone: 'mid', pos: [x * (L - t) / 2, H / 2, z * (W - t) / 2] });
  }
  function bSink(D) {
    const q = first(D), [L, H, W] = size(q), t = Math.max(3, L * 0.07), h = H * 0.6;
    box(L, t, W, { parent: q, tone: 'mid', pos: [0, t / 2, 0] });
    for (const s of [-1, 1]) { box(L, h, t, { parent: q, tone: 'hi', pos: [0, h / 2, s * (W - t) / 2] }); box(t, h, W, { parent: q, tone: 'hi', pos: [s * (L - t) / 2, h / 2, 0] }); }
  }
  function bZone(D) {
    bSensor(D);
    const z = num(par(D, 'zone_mm'), 60), sq = new THREE.LineLoop(new THREE.BufferGeometry().setFromPoints([[-z, -z], [z, -z], [z, z], [-z, z]].map(([x, y]) => new THREE.Vector3(x, 1, y))), ghostMat);
    sq.computeLineDistances(); sq.raycast = () => {}; first(D).add(sq);
  }
  function bPlain(D) { for (const p of Object.values(D.pg)) { const [a, b, c] = size(p); box(a, b, c, { parent: p, tone: 'hi', pos: [0, b / 2, 0] }); } }

  function pressed(D, s) { const a = D.dev.io && (D.dev.io.contact || D.dev.io.contact_1); if (!a || !(a in s.bits)) return false; return par(D, 'contact') === 'nc' ? !s.bits[a] : !!s.bits[a]; }

  function build(dev, cat) {
    const K = cat.kinds[dev.kind] || { behaviors: [], io: {}, params: {} }, lay = cat.layout[dev.kind] || { parts: [{ name: 'body', size: [50, 50, 50], pos: [0, 0, 0] }], ports: [] };
    const g = new THREE.Group(), pose = dev.pose || {}; g.userData.id = dev.id;
    g.position.set(...(pose.pos || [0, 0, 0])); g.rotation.y = rad((pose.rot || [0, 0, 0])[1] || 0);
    const D = { id: dev.id, dev, K, g, parts: [], planes: [], pg: {}, ports: {}, upd: [], sprites: [], io: ioList(dev, K), box: new THREE.Box3() };
    for (const p of lay.parts) { // 부품마다 그룹: 원점이 부품 바닥의 중심
      const o = dev.parts && dev.parts[p.name], q = new THREE.Group(), at = (o && o.pos) || p.pos;
      q.position.set(...at); q.userData.size = p.size; g.add(q); D.pg[p.name] = q;
      D.box.expandByPoint(v1.set(at[0] - p.size[0] / 2, at[1], at[2] - p.size[2] / 2)).expandByPoint(v1.set(at[0] + p.size[0] / 2, at[1] + p.size[1], at[2] + p.size[2] / 2));
    }
    for (const p of lay.ports) D.ports[p.name] = p;
    const has = b => (K.behaviors || []).includes(b);
    if (has('valve')) bValve(D);
    if (has('valve') && has('linear')) bCylinder(D);
    else if (has('valve') && has('rotary')) bSwing(D);
    else if (has('rotary')) (D.pg.table ? bTable : bMotor)(D);
    else if (has('belt')) bBelt(D);
    else if (has('switch')) bSwitch(D);
    else if (has('light')) bLight(D);
    else if (has('sensor')) bSensor(D);
    else if (has('level')) bTank(D);
    else if (has('zone')) bZone(D);
    else if (has('source')) bSource(D);
    else if (has('sink')) bSink(D);
    else bPlain(D);
    for (const p of lay.ports) if (p.kind === 'air' && D.pg[p.part]) box(9, 9, 9, { parent: D.pg[p.part], tone: 'mid', pos: p.at }); // 공기 포트 피팅
    world.add(g); restyle(g, D);
    return D;
  }
  const corners = D => { const b = D.box, out = []; for (let i = 0; i < 8; i++) out.push(new THREE.Vector3(i & 1 ? b.max.x : b.min.x, i & 2 ? b.max.y : b.min.y, i & 4 ? b.max.z : b.min.z).applyMatrix4(D.g.matrixWorld)); return out; };
  function portPos(id, port) { const D = devs[id], p = D && D.ports[port], q = p && D.pg[p.part]; return q ? q.localToWorld(new THREE.Vector3(...p.at)) : null; }
  // 링크: 포트에서 위로, 완만한 아치, 다시 아래로 (F39). 공기는 튜브와 흐름 점, 전선은 가는 선
  function addLink(L) {
    const [da, pa] = String(L.from).split('.'), [db, pb] = String(L.to).split('.'), A = portPos(da, pa), B = portPos(db, pb); if (!A || !B) return;
    const air = L.kind === 'air', up = Math.max(A.y, B.y) + (air ? 24 : 10), mid = A.clone().lerp(B, 0.5); mid.y = up + (air ? 8 : 3) + A.distanceTo(B) * 0.06;
    const curve = new THREE.CatmullRomCurve3([A, new THREE.Vector3(A.x, up, A.z), mid, new THREE.Vector3(B.x, up, B.z), B]);
    let pts = null;
    if (air) {
      const g = new THREE.Group(), m = mesh(new THREE.TubeGeometry(curve, 40, 2.6, 10), { parent: g, tone: 'mid', edge: 80, round: true }); m.raycast = () => {};
      world.add(g); restyle(g, null); pts = dots(10); world.add(pts);
    } else { const l = new THREE.Line(new THREE.BufferGeometry().setFromPoints(curve.getPoints(24)), new THREE.LineBasicMaterial({ color: C['--a-lo'] })); l.raycast = () => {}; world.add(l); }
    tubes.push({ curve, pts, ends: [[da, pa], [db, pb]], vdir: pa.startsWith('valve') ? 1 : pb.startsWith('valve') ? -1 : 1 });
  }
  function clear() {
    tc.detach(); adj = null;
    world.traverse(o => { if (o.geometry) o.geometry.dispose(); if (o.material) o.material.dispose(); });
    world.clear(); parts.length = 0; tubes.length = 0; for (const s of strips) s.box.remove(); strips.length = 0; devs = dict();
    for (const id in tj) delete tj[id];
    wp = dict(); wpMesh = []; for (const id in atags) { atags[id].remove(); delete atags[id]; }
    W2 = null; pD = null; pw = dict(); wkInfo = null; ptsObj = null; addrDev = dict(); selPart = hovPart = null;
  }
  // 부품(F28): 작은 상자. 벨트가 나르는 부품은 벨트 윗면에 올려 그린다 (서버의 y는 공급기의 y 그대로다)
  function partY(p) { const D = devs[p.holder]; return D && (D.K.behaviors || []).includes('belt') ? D.g.position.y + size(D.pg.frame || first(D))[1] * 0.96 : num(p.y, 0); }
  function drawParts(now) {
    if (sc2) return drawParts2(now);
    let more = false, i = 0;
    for (const id in wp) {
      const t = wp[id], k = t.dur > 0 ? clamp((now - t.t0) / t.dur, 0, 1) : 1; t.cur = t.from.map((a, j) => a + (t.to[j] - a) * k);
      if (k < 1 && t.from.some((a, j) => a !== t.to[j])) more = true;
      let g = wpMesh[i];
      if (!g) { g = wpMesh[i] = new THREE.Group(); box(wpSize, wpSize * 0.6, wpSize, { parent: g, tone: 'hi', fill: '--a-s3', pos: [0, wpSize * 0.3, 0] }).raycast = () => {}; world.add(g); restyle(g, null); }
      g.visible = true; g.position.set(...t.cur); i++;
    }
    for (; i < wpMesh.length; i++) wpMesh[i].visible = false;
    return more;
  }
  // ---- 형식 2 장면 (`resolved`가 있는 것, A5b): 장치마다 그룹, 틀마다 그룹, 외형, 표시등. 형식 1 코드는 위의 것 그대로다 ----
  let sc2 = false, hovId = null, hovLamp = null, known = new Set(), lampM = null, bkEls = [], ringEl = null, ltagEl = null;
  const skp = []; // 장치와 색조마다 재질 한 벌 (TW142)
  const cs = (n, d) => new THREE.Color(cssv(n) || d);
  const lineVar = t => (t === 'black' || t === 'dark' ? '--sk-line-inv' : t === 'clear' ? '--sk-line-clear' : t === 'mark' ? '--sk-line-mark' : '--sk-line');
  const SK = dict({ plain: { fill: cs('--a-top', '#fff'), line: cs('--sk-line', '#3d3d43') } });
  for (const t of SKIN_TONES) SK[t] = { fill: cs('--sk-' + t, '#5a5d65'), line: cs(lineVar(t), '#3d3d43') };
  const SKX = { off: cs('--sk-lamp-off', '#4a4c52'), offLine: cs('--sk-line-inv', '#b9bcc3'), mark: cs('--sk-mark', '#ffb020'), belt: cs('--sk-belt-mark', '#7d8088') };
  const clearOp = parseFloat(cssv('--sk-clear-op')) || 0.3;
  // 12개 수(x축, y축, z축, 원점) → 행렬. 모양이 틀리면 단위 행렬
  const m12 = a => { const m = new THREE.Matrix4(); if (Array.isArray(a) && a.length === 12 && a.every(x => typeof x === 'number' && isFinite(x))) { m.makeBasis(new THREE.Vector3(a[0], a[1], a[2]), new THREE.Vector3(a[3], a[4], a[5]), new THREE.Vector3(a[6], a[7], a[8])); m.setPosition(a[9], a[10], a[11]); } return m; };
  const putM = (g, m) => m.decompose(g.position, g.quaternion, g.scale);
  // 관절 값 v에서 관절 틀의 변환 (서버 mech::motion과 같은 식: 직동은 축 방향 이동, 회전은 축 둘레, 평면은 (x, z) 이동과 y 둘레 회전)
  function jointM(J, v) {
    const a = new THREE.Vector3(...(Array.isArray(J.axis) ? J.axis : [1, 0, 0]).map(x => num(x, 0))), s = Array.isArray(v) ? v : [v], d = num(s[0], 0) - num(J.zero, 0);
    if (J.type === 'fixed') return new THREE.Matrix4(); // 고정 관절: 부모 틀을 그대로 따라간다
    if (J.type === 'prismatic') return new THREE.Matrix4().makeTranslation(a.x * d, a.y * d, a.z * d);
    if (J.type === 'revolute') return new THREE.Matrix4().makeRotationAxis(a.normalize(), rad(d));
    return new THREE.Matrix4().makeTranslation(num(s[0], 0), 0, num(s[1], 0)).multiply(new THREE.Matrix4().makeRotationY(rad(num(s[2], 0))));
  }
  const v3 = (a, d = 0) => (Array.isArray(a) ? [num(a[0], d), num(a[1], d), num(a[2], d)] : [d, d, d]);
  function itemGeo(it) {
    const f = (v, d) => clamp(num(v, d), 0.01, 100000);
    if (it.t === 'box') { const s = Array.isArray(it.size) ? it.size : []; return new THREE.BoxGeometry(f(s[0], 10), f(s[1], 10), f(s[2], 10)); }
    const d = f(it.d, 10), d2 = it.d2 == null ? d : f(it.d2, d), g = new THREE.CylinderGeometry(d2 / 2, d / 2, f(it.len, 10), d >= 40 ? 32 : 20); // d = 낮은 쪽 지름, d2 = 높은 쪽
    if (it.axis === 'z') g.rotateX(Math.PI / 2); else if (it.axis !== 'y') g.rotateZ(-Math.PI / 2);
    return g;
  }
  // 한 메시: 장치·색조 재질 + 윤곽선(30°) + 둥근 것은 실루엣 껍질. `clear`는 껍질 없음, `mark`는 음영 없음
  function skMesh(D, geo, tone, parent, pos, rot, round, shell = 0.7) {
    if (!SK[tone]) tone = 'dark';
    let e = D.skm[tone];
    if (!e) { e = D.skm[tone] = { D, tone, mat: tone === 'mark' ? new THREE.MeshBasicMaterial() : new THREE.MeshToonMaterial({ gradientMap: grad, transparent: tone === 'clear', opacity: tone === 'clear' ? clearOp : 1, depthWrite: tone !== 'clear' }), lm: new THREE.LineBasicMaterial(), hm: null }; skp.push(e); }
    const m = new THREE.Mesh(geo, e.mat); m.position.set(...pos); if (rot) m.rotation.set(rad(rot[0]), rad(rot[1]), rad(rot[2]), 'YZX'); parent.add(m);
    const n = (geo.index ? geo.index.count : geo.attributes.position.count) / 3;
    if (D.tris + n <= TRI_CAP) {
      D.tris += n; const l = new THREE.LineSegments(new THREE.EdgesGeometry(geo, EDGE_DEG), e.lm); l.raycast = () => {}; m.add(l);
      if (round && tone !== 'clear' && geo.attributes.normal) {
        const hg = geo.clone(), p = hg.attributes.position, nr = hg.attributes.normal;
        for (let i = 0; i < p.count; i++) p.setXYZ(i, p.getX(i) + nr.getX(i) * shell, p.getY(i) + nr.getY(i) * shell, p.getZ(i) + nr.getZ(i) * shell);
        e.hm = e.hm || new THREE.MeshBasicMaterial({ side: THREE.BackSide }); const h = new THREE.Mesh(hg, e.hm); h.raycast = () => {}; m.add(h);
      }
    } else if (!D.over) { D.over = true; console.warn(`twin: device ${D.id} has more than ${TRI_CAP} triangles; the rest gets no outlines`); }
    return m;
  }
  const lampAddr = (from, k) => { const m = /^([XYM])([0-9A-F]+)$/i.exec(String(from)); if (!m) return null; const hex = m[1] !== 'M', n = parseInt(m[2], hex ? 16 : 10) + k; return m[1].toUpperCase() + n.toString(hex ? 16 : 10).toUpperCase(); };
  function build2(dev, rd, K) {
    const g = new THREE.Group(); g.userData.id = dev.id;
    const D = { id: dev.id, dev, K, rd, g, parts: [], planes: [], pg: dict(), ports: dict(), upd: [], sprites: [], io: ioList(dev, K), box: new THREE.Box3(), fg: dict({ base: g }), jt: dict(), jn: [], mv: null, mvPos: null, groups: [], lamps: [], belt: null, skm: dict(), tris: 0, over: false };
    for (const [name, f] of Object.entries(rd.frames || {})) {
      if (name === 'base') continue;
      const fg = new THREE.Group(), base = m12(f.xf); putM(fg, base); g.add(fg); D.fg[name] = fg;
      if (f.moving) D.mv = { g: fg, base, name };
    }
    // 엔진 관절 틀(2.1b): 틀 목록은 관절 순서라 부모가 앞에 온다. K = 부모 시작 자리의 역 × 이 틀의 시작 자리 × 시작 값 변환의 역
    // (서버의 frames 객체는 이름순이라 부모가 앞에 온다고 믿을 수 없다: 부모가 준비된 틀부터 되풀이해 넣는다)
    for (let todo = Object.entries(rd.frames || {}).filter(([, f]) => f.joint && ['prismatic', 'revolute', 'planar', 'fixed'].includes(f.joint.type)), n = -1; todo.length && todo.length !== n;) {
      n = todo.length;
      todo = todo.filter(([name, f]) => {
        const J = f.joint; if (J.parent && !D.jt[J.parent]) return true;
        const K = (J.parent ? D.jt[J.parent].start.clone().invert() : new THREE.Matrix4()).multiply(m12(f.xf)).multiply(jointM(J, J.start).invert());
        D.jt[name] = { j: J, K, start: m12(f.xf) }; D.jn.push(name); return false;
      });
    }
    // 장치 상자 (resolved의 상자, 쉬는 자리). 화면 상자, 입출력 줄, twin_render 상자, 시점 맞춤이 쓴다
    for (const b of rd.boxes || []) {
      const s = v3(b.size, 1), p = v3(b.pos), r = v3(b.rot), q = new THREE.Quaternion().setFromEuler(new THREE.Euler(rad(r[0]), rad(r[1]), rad(r[2]), 'YZX')), M = new THREE.Matrix4().compose(new THREE.Vector3(...p), q, new THREE.Vector3(1, 1, 1));
      for (let i = 0; i < 8; i++) D.box.expandByPoint(v1.set(i & 1 ? s[0] / 2 : -s[0] / 2, i & 2 ? s[1] / 2 : -s[1] / 2, i & 4 ? s[2] / 2 : -s[2] / 2).applyMatrix4(M));
    }
    const skin = dev.skin && Array.isArray(dev.skin.groups) ? dev.skin.groups : null;
    if (skin) {
      for (const gr of skin.slice(0, 16)) {
        const gg = new THREE.Group(), host = D.fg[gr.frame] || g; host.add(gg);
        D.groups.push({ name: gr.name, g: gg, show: gr.show, hide: gr.hide });
        for (const it of (Array.isArray(gr.items) ? gr.items : []).slice(0, 64)) if (it && (it.t === 'box' || it.t === 'cyl')) skMesh(D, itemGeo(it), SKIN_TONES.includes(it.tone) || it.tone == null ? it.tone || 'metal' : 'dark', gg, v3(it.pos), it.rot ? v3(it.rot) : null, it.t === 'cyl');
      }
    } else {
      for (const b of rd.boxes || []) { // 외형이 없는 장치: resolved의 상자. 엔진 관절이 있으면 유리처럼 그려 안의 관절 표시를 가리지 않는다 (TW186)
        const s = v3(b.size, 1).map(x => clamp(x, 0.1, 100000)), geo = b.shape === 'cyl' ? new THREE.CylinderGeometry(s[1] / 2, s[1] / 2, s[0], 20).rotateZ(Math.PI / 2) : new THREE.BoxGeometry(...s);
        let host = g, p = v3(b.pos);
        if (b.moving && D.mv) { host = D.mv.g; const o = D.mv.base.elements; p = [p[0] - o[12], p[1] - o[13], p[2] - o[14]]; }
        skMesh(D, geo, D.jn.length ? 'clear' : 'plain', host, p, b.rot ? v3(b.rot) : null, b.shape === 'cyl');
      }
      if (D.jn.length) jointMarks(D);
    }
    // 표시등 줄: 켜진 것과 아닌 것은 크기와 밝기로 구별한다
    for (const row of (Array.isArray(dev.indicators) ? dev.indicators : []).slice(0, 8)) {
      const host = D.fg[row.frame] || g, at = v3(row.at), st2 = v3(row.step), dia = clamp(num(row.d, 5), 1, 50);
      const pins = Array.isArray(row.pins) ? row.pins.slice(0, 64) : null; // 핀 형식(공개 화면): 램프는 목록 순서로 핀 하나씩. plc-mcp 쪽은 서버가 주소로 풀어 `from`+`count`로 준다
      for (let k = 0; k < (pins ? pins.length : clamp(num(row.count, 0), 0, 64)); k++) {
        const addr = pins ? (typeof pins[k] === 'string' ? pins[k] : null) : lampAddr(row.from, k), m = new THREE.Mesh(new THREE.CylinderGeometry(dia / 2, dia / 2, 1.2, 14), lampM.off);
        m.position.set(at[0] + st2[0] * k, at[1] + st2[1] * k + 0.6, at[2] + st2[2] * k); host.add(m);
        D.lamps.push({ addr, mesh: m, on: false, inScene: !!addr && known.has(addr) });
      }
    }
    // 벨트 줄: 진행 방향을 가리키는 쐐기, 45 mm마다. 쉴 때는 회색, 돌 때는 호박색
    if (rd.form === 'belt' && D.fg.surface) {
      const len = num(rd.dims && rd.dims.length_mm && rd.dims.length_mm.v, 600), n = clamp(Math.floor(len / 45), 1, 200), bm = new THREE.MeshBasicMaterial({ color: SKX.belt }), marks = [];
      const bar = (z, a) => { const m = new THREE.Mesh(new THREE.BoxGeometry(26, 0.8, 3), bm); m.rotation.y = rad(a); m.position.z = z; m.raycast = () => {}; return m; };
      for (let i = 0; i < n; i++) { const c = new THREE.Group(); c.add(bar(6.1, 28), bar(-6.1, -28)); c.position.y = 0.5; D.fg.surface.add(c); marks.push(c); }
      D.belt = { marks, bm, span: n * 45, run: false };
    }
    return D;
  }
  // 외형 없는 엔진 장치의 관절 표시 (설계 45 2.2, TW185): 직동 = 레일 8 x 8 (min~max, 부모 쪽에 고정) + 블록 24, 회전 = 원판 40 x 8 + 흰 바늘,
  // 평면 = 200 x 200 사각 + 앞 방향 막대, 고정 = 점 6. 앞 관절과는 막대(지름 8)로 잇는다. 모두 관절 틀에 달려 값을 따라 움직인다
  function jointMarks(D) {
    const ext = D.box.isEmpty() ? [40, 40, 40] : D.box.getSize(new THREE.Vector3()).toArray();
    for (const nm of D.jn) {
      const J = D.jt[nm], j = J.j, fg = D.fg[nm], ax = v3(j.axis), ai = ax.findIndex(a => Math.abs(a) > 0.5);
      const along = geo => (ai === 0 ? geo.rotateZ(Math.PI / 2) : ai === 2 ? geo.rotateX(Math.PI / 2) : geo); // y로 만든 모양을 관절 축으로
      if (j.type === 'prismatic') {
        skMesh(D, new THREE.BoxGeometry(24, 24, 24), 'dark', fg, [0, 0, 0]);
        if (typeof j.min === 'number' && typeof j.max === 'number' && j.max > j.min) {
          const c = (j.min + j.max) / 2 - num(j.zero, 0); J.rail = new THREE.Group(); D.g.add(J.rail);
          skMesh(D, along(new THREE.BoxGeometry(8, clamp(j.max - j.min, 1, 40000), 8)), 'dark', J.rail, ax.map(a => a * c));
        }
      } else if (j.type === 'revolute') {
        skMesh(D, along(new THREE.CylinderGeometry(20, 20, 8, 20)), 'dark', fg, [0, 0, 0], null, true);
        // 바늘: 축에 수직. 첫 관절(앞 관절 없음)은 장치 상자 반만큼 길게 해서 회전대처럼 큰 장치의 돎이 보이게 한다
        const pi = ai === 1 ? 0 : 1, oi = 3 - ai - pi, r = j.parent ? 20 : clamp(Math.min(ext[pi], ext[oi]) / 2, 20, 600), s = [4, 4, 4], p = [0, 0, 0];
        s[pi] = r; s[ai < 0 ? 1 : ai] = 10; p[pi] = r / 2;
        skMesh(D, new THREE.BoxGeometry(...s), 'white', fg, p);
      } else if (j.type === 'planar') {
        skMesh(D, new THREE.BoxGeometry(200, 2, 200), 'clear', fg, [0, 1, 0]);
        skMesh(D, new THREE.BoxGeometry(100, 4, 4), 'dark', fg, [50, 3, 0]);
      } else skMesh(D, new THREE.SphereGeometry(3, 12, 8), 'dark', fg, [0, 0, 0]);
      const o = new THREE.Vector3().setFromMatrixPosition(J.K), L = o.length();
      if (j.parent && L > 1) {
        const geo = new THREE.CylinderGeometry(4, 4, L, 10).applyQuaternion(new THREE.Quaternion().setFromUnitVectors(new THREE.Vector3(0, 1, 0), o.clone().divideScalar(L)));
        skMesh(D, geo, 'dark', D.fg[j.parent], o.multiplyScalar(0.5).toArray(), null, true);
      }
    }
  }
  // ---- 형식 2 공작물 (TW168~TW178). 부품의 자리는 서버가 준 세계 mm 그대로다. 쪽은 통의 쌓임 자리(U3)만 계산한다 ----
  const typeOf = p => (W2 && p && W2.types[p.type]) || null;
  function buildW2(sc, list) {
    const w = sc && sc.workpieces; if (!w || typeof w.types !== 'object' || !w.types) return null;
    const types = dict();
    for (const [n, t] of Object.entries(w.types)) if (t && typeof t === 'object') { const sq = t.shape === 'square'; types[n] = { name: n, round: !sq, s: clamp(num(sq ? t.side_mm : t.d_mm, 26), 2, 500), h: clamp(num(t.h_mm, 20), 1, 500), tone: t.tone, material: t.material, basis: t.basis }; }
    const points = dict(), plist = [];
    for (const q of Array.isArray(sc.points) ? sc.points : []) if (q && typeof q.name === 'string' && typeof q.on === 'string') { const o = { name: q.name, on: q.on, pos: v3(q.pos), catch: clamp(num(q.catch_mm, 2), 0, 200), holds: clamp(num(q.holds, 1), 1, 50) }; points[q.name] = o; plist.push(o); }
    const stacks = []; for (const d of list) if (d.stack && typeof d.stack.point === 'string') stacks.push({ dev: d.id, label: d.label || d.id, point: d.stack.point, content: Array.isArray(d.stack.content) ? d.stack.content : [], refill: d.stack.refill });
    return { types, points, plist, stacks };
  }
  // 지점의 세계 자리 (붙은 틀의 지금 자리를 따라간다)
  function ptWorld(o, out) {
    const [did, fr] = o.on.split('.'), D = devs[did], g = D && (D.fg[fr || 'base'] || D.g); if (!g) return null;
    g.updateWorldMatrix(true, false); return g.localToWorld(out.set(o.pos[0], o.pos[1], o.pos[2]));
  }
  // 통(holds가 1보다 큰 지점)의 쌓임: 들어온 차례(arrival)대로 한 층 7개, 가운데 하나와 둘레 여섯 (TW176). 그림만의 자리다
  function pile(parts) {
    const res = dict(), by = dict(), cols = new Set(W2.stacks.map(q => q.point));
    parts.forEach((p, i) => {
      const pt = p.at && W2.points[p.at]; if (p.state !== 'rest') return;
      if (cols.has(p.at)) res[p.id] = { d: [0, 0, 0], base: false, see: true };
      if (pt && pt.holds > 1) (by[p.at] = by[p.at] || []).push(i);
    });
    for (const name in by) {
      const pt = W2.points[name], L = by[name].sort((a, b) => (num(parts[a].arrival, 1e9) - num(parts[b].arrival, 1e9)) || a - b);
      L.forEach((idx, k) => {
        const p = parts[idx], ty = typeOf(p); if (!ty) return;
        const layer = Math.floor(k / 7), j = k % 7, r = Math.min(ty.s + 1, pt.catch - ty.s / 2);
        let d; if (r <= 0) d = [0, k * ty.h, 0]; else if (j === 0) d = [0, layer * ty.h, 0]; else { const a = rad(60 * (j - 1) + 30 * (layer % 2)); d = [r * Math.cos(a), layer * ty.h, r * Math.sin(a)]; }
        res[p.id] = { d, base: layer === 0, see: true };
      });
    }
    return res;
  }
  // 새 상태 → 부품마다 {from,to,cur,...}와 꼬리표용 요약 (wkInfo)
  function partsState(s, prev, now, dur) {
    const out = dict(), parts = Array.isArray(s.parts) ? s.parts.filter(p => p && p.id != null) : [], off = pile(parts), v = new THREE.Vector3();
    world.updateMatrixWorld(true);
    const info = { lists: dict(), bins: dict(), left: [], forced: Array.isArray(s.forced) ? s.forced.filter(a => typeof a === 'string').slice(0, 40) : [] };
    for (const q of W2.stacks) { info.lists[q.dev] = []; const o = W2.points[q.point], w = o && ptWorld(o, v); q.wxz = w ? [w.x, w.z] : null; }
    for (const p of parts) {
      const o = off[p.id] || { d: [0, 0, 0], base: true, see: false }, to = [num(p.x, 0) + o.d[0], num(p.y, 0) + o.d[1], num(p.z, 0) + o.d[2]], q = prev[p.id];
      out[p.id] = { from: q ? q.cur : to, to, cur: q ? q.cur : to, t0: now, dur, p, base: o.base, see: o.see || p.state === 'stack' || p.state === 'left' };
      if (p.state === 'left') info.left.push(p.id);
      if (p.state === 'rest' && p.at) { const q2 = W2.stacks.find(x => x.point === p.at); if (q2) info.lists[q2.dev].push(p); const pt = W2.points[p.at]; if (pt && pt.holds > 1) info.bins[p.at] = (info.bins[p.at] || 0) + 1; }
      if (p.state === 'stack') { let b = null, bd = 900; for (const q2 of W2.stacks) if (q2.wxz) { const d = (q2.wxz[0] - num(p.x, 0)) ** 2 + (q2.wxz[1] - num(p.z, 0)) ** 2; if (d < bd) { bd = d; b = q2; } } if (b) info.lists[b.dev].push(p); }
    }
    for (const id in info.lists) info.lists[id].sort((a, b) => num(a.y, 0) - num(b.y, 0) || a.id - b.id); // 아래부터
    wkInfo = info; return out;
  }
  function makePart(ty, tone, key) {
    const s = ty.s, h = ty.h, g = new THREE.Group(), t = SK[tone] && tone !== 'mark' && tone !== 'plain' ? tone : 'dark', n0 = pD.tris;
    const geo = ty.round ? new THREE.CylinderGeometry(s / 2, s / 2, h, 36) : new THREE.BoxGeometry(s, h, s);
    skMesh(pD, geo, t, g, [0, h / 2, 0], null, ty.round, 1.0);
    const e = pD.skm[t], pts = [];
    if (ty.round) for (let i = 0; i < 36; i++) pts.push(new THREE.Vector3(Math.cos(i / 36 * 2 * Math.PI) * 0.3 * s, 0, Math.sin(i / 36 * 2 * Math.PI) * 0.3 * s));
    else for (const [a, b] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) pts.push(new THREE.Vector3(a * 0.3 * s, 0, b * 0.3 * s));
    const mark = new THREE.LineLoop(new THREE.BufferGeometry().setFromPoints(pts), e.lm); mark.position.y = h + 0.3; mark.raycast = () => {}; g.add(mark); // 윗면의 부품 표시
    const cg = ty.round ? new THREE.CircleGeometry(0.65 * s, 36) : new THREE.PlaneGeometry(1.3 * s, 1.3 * s); cg.rotateX(-Math.PI / 2);
    const contact = new THREE.Mesh(cg, contactM); contact.position.y = 0.3; contact.renderOrder = 1; contact.raycast = () => {}; g.add(contact);
    const see = new THREE.LineSegments(new THREE.EdgesGeometry(geo, EDGE_DEG), seeM); see.position.y = h / 2; see.renderOrder = 3; see.raycast = () => {}; see.visible = false; g.add(see);
    const fence = new THREE.LineSegments(new THREE.EdgesGeometry(new THREE.BoxGeometry(s + 8, h + 6, s + 8)), fenceM); fence.computeLineDistances(); fence.position.y = h / 2; fence.raycast = () => {}; fence.visible = false; g.add(fence);
    return { g, key, contact, see, fence, tris: pD.tris - n0 };
  }
  function dropPart(e) { e.g.removeFromParent(); e.g.traverse(o => { if (o.geometry) o.geometry.dispose(); }); pD.tris = Math.max(0, pD.tris - e.tris); }
  function drawParts2(now) {
    let more = false; const seen = new Set();
    for (const id in wp) {
      const t = wp[id], p = t.p, ty = typeOf(p); if (!ty) continue;
      const k = t.dur > 0 ? clamp((now - t.t0) / t.dur, 0, 1) : 1; t.cur = t.from.map((a, j) => a + (t.to[j] - a) * k);
      if (k < 1 && t.from.some((a, j) => a !== t.to[j])) more = true;
      const key = p.type + '|' + p.tone; let e = pw[id];
      if (!e || e.key !== key) { if (e) dropPart(e); e = pw[id] = makePart(ty, p.tone, key); world.add(e.g); }
      seen.add(id); e.g.visible = true; e.g.position.set(t.cur[0], t.cur[1], t.cur[2]);
      e.contact.visible = (p.state === 'rest' || p.state === 'belt' || p.state === 'left') && t.base; e.see.visible = !!t.see; e.fence.visible = p.state === 'left';
    }
    for (const id in pw) if (!seen.has(id)) { dropPart(pw[id]); delete pw[id]; }
    return more;
  }
  // 부품의 화면 상자와 가까운 부품 (TW177: 화면 중심 12 px 안이면 가장 가까운 부품)
  function partBox(t) {
    const ty = typeOf(t.p); if (!ty) return null; let x0 = 1e9, x1 = -1e9, y0 = 1e9, y1 = -1e9; const w = host.clientWidth, h = host.clientHeight;
    for (let i = 0; i < 8; i++) { v1.set(t.cur[0] + (i & 1 ? ty.s / 2 : -ty.s / 2), t.cur[1] + (i & 2 ? ty.h : 0), t.cur[2] + (i & 4 ? ty.s / 2 : -ty.s / 2)).project(camera); const sx = (v1.x + 1) / 2 * w, sy = (1 - v1.y) / 2 * h; x0 = Math.min(x0, sx); x1 = Math.max(x1, sx); y0 = Math.min(y0, sy); y1 = Math.max(y1, sy); }
    return { x0, x1, y0, y1 };
  }
  function partNear(ex, ey) {
    if (!wkOn || !W2) return null; const r = cv.getBoundingClientRect(); let best = null, bd = 144;
    for (const id in wp) { const t = wp[id], ty = typeOf(t.p); if (!ty) continue; v1.set(t.cur[0], t.cur[1] + ty.h / 2, t.cur[2]).project(camera); const d = (ex - r.left - (v1.x + 1) / 2 * r.width) ** 2 + (ey - r.top - (1 - v1.y) / 2 * r.height) ** 2; if (d <= bd) { bd = d; best = id; } }
    return best;
  }
  // 화면 위의 겹침: 개수 꼬리표, 규칙 밖 꼬리표, 강제 꼬리표 (TW169, TW170, TW172), 부품 괄호 (TW177), 지점 이름 (TW173)
  function placeWk() {
    const want = [], pb = [];
    if (wkOn && W2 && wkInfo) {
      const top = id => { const D = devs[id]; if (!D || D.box.isEmpty()) return null; const r = screenBox(D); return [(r.x0 + r.x1) / 2, r.y0 - 4]; };
      for (const q of W2.stacks) { const a = top(q.dev), n = (wkInfo.lists[q.dev] || []).length; if (a) want.push({ t: n ? T('r.stack.n', { label: q.label, n }) : T('r.stack.empty', { label: q.label }), c: n ? 'cnt' : 'cnt zero', a }); }
      for (const q of W2.plist) if (q.holds > 1) { const n = wkInfo.bins[q.name] || 0, a = n && top(q.on.split('.')[0]); if (a) want.push({ t: T('r.count', { n }), c: 'cnt', a }); }
      for (const ad of wkInfo.forced) { const id = addrDev[ad], a = id && id !== sel && top(id); if (a) want.push({ t: `${ad} ${T('r.forced')}`, c: 'frc', a }); }
      for (const id of wkInfo.left.slice(0, 3)) { const t = wp[id], ty = t && typeOf(t.p); if (!ty) continue; v1.set(t.cur[0], t.cur[1] + ty.h + 3, t.cur[2]).project(camera); want.push({ t: T('r.left'), c: 'lft', a: [(v1.x + 1) / 2 * host.clientWidth, (1 - v1.y) / 2 * host.clientHeight - 5] }); }
      if (selPart != null && wp[selPart]) pb.push([selPart, 'brk pt']);
      if (hovPart != null && hovPart !== selPart && wp[hovPart]) pb.push([hovPart, 'brk pt hov']);
    }
    while (wkEls.length < want.length) wkEls.push(tagLayer.appendChild(el('span', 'wtag')));
    const placed = [];
    wkEls.forEach((e, i) => {
      const w = want[i]; e.hidden = !w; if (!w) return;
      const cls = 'wtag ' + w.c; if (e.className !== cls) e.className = cls; if (e.textContent !== w.t) e.textContent = w.t;
      const W = e.offsetWidth, H = e.offsetHeight, x = w.a[0] - W / 2; let y = w.a[1] - H;
      for (let n = 0; n < 12; n++) { const o = placed.find(p => x < p.x + p.w && p.x < x + W && y < p.y + p.h && p.y < y + H); if (!o) break; y -= 18; } // 앞 꼬리표를 가리면 한 줄 위로
      placed.push({ x, y, w: W, h: H }); e.style.transform = `translate(${x.toFixed(1)}px, ${y.toFixed(1)}px)`;
    });
    while (pbEls.length < pb.length) pbEls.push(tagLayer.appendChild(el('span', 'brk')));
    pbEls.forEach((e, i) => { const w = pb[i]; e.hidden = !w; if (!w) return; const r = partBox(wp[w[0]]); if (!r) { e.hidden = true; return; } e.className = w[1]; Object.assign(e.style, { width: (r.x1 - r.x0 + 8).toFixed(0) + 'px', height: (r.y1 - r.y0 + 8).toFixed(0) + 'px', transform: `translate(${(r.x0 - 4).toFixed(1)}px, ${(r.y0 - 4).toFixed(1)}px)` }); });
  }
  // 지점: 십자(모든 것 위에)와 이름. 기본은 안 그린다. 고른 장치의 지점은 항상 그린다 (TW173)
  function placePoints() {
    const vis = wkOn && W2 ? W2.plist.filter(q => ptsAll || q.on.split('.')[0] === sel) : [];
    if (!ptsObj && W2) { const g = new THREE.BufferGeometry(); g.setAttribute('position', new THREE.BufferAttribute(new Float32Array(32 * 18), 3)); ptsObj = new THREE.LineSegments(g, pointM); ptsObj.renderOrder = 10; ptsObj.frustumCulled = false; ptsObj.raycast = () => {}; world.add(ptsObj); }
    const names = [], v = new THREE.Vector3();
    if (ptsObj) {
      const a = ptsObj.geometry.attributes.position; let n = 0;
      for (const q of vis.slice(0, 32)) {
        if (!ptWorld(q, v)) continue; const { x, y, z } = v;
        for (const l of [[x - 6, y, z, x + 6, y, z], [x, y, z - 6, x, y, z + 6], [x, y, z, x, y + 6, z]]) { a.setXYZ(n++, l[0], l[1], l[2]); a.setXYZ(n++, l[3], l[4], l[5]); }
        v.project(camera); names.push([q.name, (v.x + 1) / 2 * host.clientWidth + 7, (1 - v.y) / 2 * host.clientHeight]);
      }
      a.needsUpdate = true; ptsObj.geometry.setDrawRange(0, n); ptsObj.visible = n > 0;
    }
    while (ptEls.length < names.length) ptEls.push(tagLayer.appendChild(el('span', 'pname')));
    const placed = [];
    ptEls.forEach((e, i) => {
      const w = names[i]; e.hidden = !w; if (!w) return; if (e.textContent !== w[0]) e.textContent = w[0];
      const W = e.offsetWidth, H = e.offsetHeight, x = w[1], y = w[2] - H / 2;
      if (placed.some(p => x < p.x + p.w + 2 && p.x < x + W + 2 && y < p.y + p.h && p.y < y + H)) { e.hidden = true; return; } // 겹치는 이름은 숨긴다 (십자는 남는다)
      placed.push({ x, y, w: W, h: H }); e.style.transform = `translate(${x.toFixed(1)}px, ${y.toFixed(1)}px)`;
    });
  }
  function addFloor(b) {
    const M = 70, w = b.max.x - b.min.x + 2 * M, d = b.max.z - b.min.z + 2 * M, base = new THREE.Group();
    base.position.set((b.min.x + b.max.x) / 2, 0, (b.min.z + b.max.z) / 2);
    box(w, 14, d, { parent: base, tone: 'lo', pos: [0, -7, 0] });
    for (const x of [-1, 1]) for (const z of [-1, 1]) box(30, 12, 30, { parent: base, tone: 'lo', pos: [x * (w / 2 - 30), -20, z * (d / 2 - 30)] });
    const halo = new THREE.Mesh(new THREE.PlaneGeometry(w * 1.3, d * 1.5), new THREE.MeshBasicMaterial({ map: haloTex, transparent: true, depthWrite: false, color: C['--a-halo'], opacity: parseFloat(cssv('--a-halo-op')) || 0.3 }));
    halo.userData.raw = true; halo.rotation.x = -Math.PI / 2; halo.position.y = -26; halo.raycast = () => {}; base.add(halo);
    for (const m of base.children) m.raycast = () => {};
    world.add(base); restyle(base, null);
  }
  // 엔진 관절 사슬의 도달 범위 (F1, TW208): 뿌리 관절의 시작 자리를 중심으로, 마디 길이 합 + 직동 관절의 max + 여유 60 mm인 정육면체를 `b`에 넣는다.
  // 바닥(y 0) 아래는 자른다. 평면 관절은 길을 미리 알 수 없어 더하지 않는다. 마디 길이는 시작 자리의 관절 틀 사이 거리다(회전은 거리를 바꾸지 않는다).
  // shortcut: 자식 장치(도구)의 상자는 따로 재지 않고 여유 60 mm로 갈음한다. 큰 도구가 생기면 그 상자의 반 대각선을 더한다.
  function reachBox(r, b) {
    const fr = (r && r.frames) || {}, org = n => { const x = fr[n] && fr[n].xf; return Array.isArray(x) && x.length === 12 ? [x[9], x[10], x[11]] : null; };
    const jt = n => (fr[n] && fr[n].joint) || null, par = n => { const j = jt(n); return j && j.parent && jt(j.parent) ? j.parent : null; };
    const pl = (n, d = 0) => {
      const j = jt(n); if (!j || d > 16) return 0; const a = org(n), p = par(n), o = p && org(p);
      return (a && o ? Math.hypot(a[0] - o[0], a[1] - o[1], a[2] - o[2]) : 0) + (j.type === 'prismatic' ? Math.max(0, num(j.max, 0)) : 0) + (p ? pl(p, d + 1) : 0);
    };
    const rootOf = (n, d = 0) => (par(n) && d < 16 ? rootOf(par(n), d + 1) : n), tree = dict();
    for (const n in fr) if (jt(n)) { const k = rootOf(n); tree[k] = Math.max(tree[k] || 0, pl(n)); }
    for (const k in tree) {
      const o = org(k); if (!o) continue;
      const c = v1.set(o[0], o[1], o[2]).applyMatrix4(m12(r.world)), R = tree[k] + 60;
      b.expandByPoint(new THREE.Vector3(c.x - R, Math.max(0, c.y - R), c.z - R)); b.expandByPoint(new THREE.Vector3(c.x + R, c.y + R, c.z + R));
    }
  }
  function load2(sc, cat, rd) {
    const keep = sel; clear(); sc2 = true; skp.length = 0; known = new Set(); hovId = null; hovLamp = null;
    lampM = { off: new THREE.MeshBasicMaterial({ color: SKX.off }), on: new THREE.MeshBasicMaterial({ color: SKX.mark }) };
    const byId = dict(); for (const r of (rd && rd.devices) || []) if (r && r.id) byId[r.id] = r;
    const list = ((sc && sc.devices) || []).filter(d => d && d.id && byId[d.id]);
    for (const d of list) { for (const a of Object.values(d.io || {})) known.add(a); for (const s of d.signals || []) if (!s.type || s.type === 'bit') known.add(s.device); }
    for (const d of list) devs[d.id] = build2(d, byId[d.id], cat.kinds[d.kind] || { behaviors: [], io: {}, params: {} });
    { const ink = cs('--ink', '#18181a'); seeM.color.copy(C['--a-mid']); fenceM.color.copy(ink); contactM.color.copy(ink); pointM.color.copy(ink); }
    pD = { id: '__parts', skm: dict(), tris: 0, over: false, io: [] }; W2 = buildW2(sc, list); addrDev = dict();
    for (const d of list) { for (const a of Object.values(d.io || {})) addrDev[a] = d.id; for (const s of d.signals || []) if (s && s.device) addrDev[s.device] = d.id; }
    for (const D of Object.values(devs)) { // 부모 틀의 그룹 아래에 단다: 부모의 움직이는 틀을 따라 움직인다
      const p = D.rd.parent, PD = p && devs[p.device], host = PD && p.frame && PD.fg[p.frame];
      // `local`는 기준 틀(datum)의 변환이라 쓰지 않는다. 부모 틀의 세계 변환의 역행렬 × 이 장치 base의 세계 변환 = 부모 틀 안의 자리
      (host || world).add(D.g);
      putM(D.g, host ? m12(PD.rd.world).multiply(m12(PD.rd.frames[p.frame].xf)).invert().multiply(m12(D.rd.world)) : m12(D.rd.world));
    }
    world.updateMatrixWorld(true);
    const b = new THREE.Box3(); for (const D of Object.values(devs)) if (!D.box.isEmpty()) for (const p of corners(D)) b.expandByPoint(p);
    if (b.isEmpty()) b.set(new THREE.Vector3(-300, 0, -200), new THREE.Vector3(300, 100, 200));
    for (const D of Object.values(devs)) reachBox(D.rd, b); // F1: 엔진 관절 사슬이 닿는 범위도 맞춤 상자에 넣는다
    addFloor(b);
    for (const D of Object.values(devs)) {
      if (!D.io.length) continue;
      const { box: sb, items } = lampRow(D.io, 'strip', D.dev.label || D.id, LO); tagLayer.append(sb);
      let ax = null; // PF4: 관절 값이 있는 상태에서는 이 글을 숨긴다 (축이 움직이는데 "읽지 않음"이라고 하지 않게)
      if (D.rd.form === 'linear') { sb.classList.add('two'); ax = sb.appendChild(el('span', 'ax', T('r.axis'))); }
      strips.push({ D, box: sb, items, ax, w: sb.offsetWidth, h: sb.offsetHeight }); sb.classList.add('hide');
    }
    sel = devs[keep] ? keep : null;
    return b;
  }
  // 상태 반영 (형식 2): 움직이는 틀, show/hide 그룹, 표시등, 벨트 줄, 경보 색
  function apply2(now) {
    const bit = a => !!(a && st.bits[a]), alarmed = new Set((st.alarms || []).map(a => a.device)); let flowing = false;
    for (const D of Object.values(devs)) {
      const ax = st.axes[D.id], f = D.rd.form;
      const mf = D.rd.mech && D.rd.mech.form; // 엔진의 형태 관절이 있는 선형 축: 축 값에서 원점(home)을 뺀 만큼 g 방향으로
      if (D.mv && (f === 'cyl' || f === 'jaw' || (f === 'linear' && mf))) { // 축 위치(mm)만큼 자기 +x로. 엔진이 없는 서보 축은 상태가 없어 원점 자리 그대로다
        const p = ax ? num(ax.draw, num(ax.pos, 0)) : 0, dx = mf ? (p - num(mf.home, 0)) * num(mf.g, 1) : p;
        if (p !== D.mvPos) { D.mvPos = p; putM(D.mv.g, new THREE.Matrix4().makeTranslation(dx, 0, 0).multiply(D.mv.base)); }
      }
      if (D.jn.length) { // 엔진 관절 틀: 값이 없으면 시작 값(= 서버가 준 쉬는 자리)
        const jv = tj[D.id] || dict(), F = dict();
        const cdx = mf ? ((ax ? num(ax.draw, num(ax.pos, 0)) : 0) - num(mf.home, 0)) * num(mf.g, 1) : 0; // 형태 관절(carriage)이 가는 만큼
        for (const nm of D.jn) {
          const J = D.jt[nm], zero = (J.j.parent ? F[J.j.parent].clone() : J.j.on_form ? new THREE.Matrix4().makeTranslation(cdx, 0, 0) : new THREE.Matrix4()).multiply(J.K), full = zero.clone().multiply(jointM(J.j, jv[nm] ? jv[nm].cur : J.j.start));
          F[nm] = full; putM(D.fg[nm], full); if (J.rail) putM(J.rail, zero); // 레일은 관절 값 0의 자리(부모 쪽)에 머문다
        }
      }
      for (const gr of D.groups) gr.g.visible = gr.show ? bit(gr.show) : gr.hide ? !bit(gr.hide) : true;
      for (const l of D.lamps) { const on = l.inScene && bit(l.addr); if (on !== l.on) { l.on = on; l.mesh.material = on ? lampM.on : lampM.off; l.mesh.scale.set(on ? 1.6 : 1, 1, on ? 1.6 : 1); } }
      if (D.belt) {
        const run = (!!ax && !!ax.moving) || ['run', 'run_fwd', 'run_back'].some(s => bit(D.dev.io && D.dev.io[s])), dir = ax && ax.spool === 'back' ? -1 : 1;
        D.belt.bm.color.copy(run ? SKX.mark : SKX.belt);
        const off = run ? (typeof (ax || {}).draw === 'number' ? ax.draw : (now / 1000) * 60 * dir) : 0;
        D.belt.marks.forEach((c, i) => { c.position.x = -D.belt.span / 2 + 22.5 + ((((i * 45 + off) % D.belt.span) + D.belt.span) % D.belt.span); });
        if (run && !reduced) flowing = true;
      }
    }
    const al = id => alarmed.has(id) && (blink || reduced);
    for (const e of skp) {
      const S = SK[e.tone]; let line = S.line; e.mat.color.copy(S.fill);
      if (e.tone === 'plain') { if (e.D.io.some(i => i.out && !i.word && bit(i.addr))) line = C['--coil']; else if (e.D.io.some(i => !i.out && !i.word && bit(i.addr))) line = C['--sensor']; }
      if (al(e.D.id) && e.tone !== 'mark') { e.mat.color.lerp(C['--alarm'], 0.35); line = C['--alarm']; }
      e.lm.color.copy(line); if (e.hm) e.hm.color.copy(line);
    }
    for (const P of parts) paint(P); // 바닥판과 부품 상자
    return flowing;
  }
  const vis = o => { for (; o; o = o.parent) if (!o.visible) return false; return true; };
  // 가장 가까운 표시등 (마우스 10 px 안)
  function lampNear(x, y) {
    let best = null, bd = 100; const w = host.clientWidth, h = host.clientHeight;
    for (const D of Object.values(devs)) for (const l of D.lamps) { l.mesh.getWorldPosition(v1).project(camera); if (Math.abs(v1.z) > 1) continue; const d = (x - (v1.x + 1) / 2 * w) ** 2 + (y - (1 - v1.y) / 2 * h) ** 2; if (d < bd) { bd = d; best = { D, l }; } }
    return best;
  }
  // 화면 위의 겹침: 선택·가리킴·경보 모서리 괄호와 표시등 고리·꼬리표 (HTML이라 그림을 바꾸지 않는다)
  function placeSk() {
    const al = new Set((st.alarms || []).map(a => a.device)), want = [];
    for (const id of al) if (devs[id]) want.push([id, 'brk alm']);
    if (sel && devs[sel] && !al.has(sel)) want.push([sel, 'brk']);
    if (hovId && devs[hovId] && hovId !== sel && !al.has(hovId)) want.push([hovId, 'brk hov']);
    while (bkEls.length < want.length) bkEls.push(tagLayer.appendChild(el('span', 'brk')));
    bkEls.forEach((e, i) => {
      const w = want[i]; e.hidden = !w; if (!w) return;
      const r = screenBox(devs[w[0]]); e.className = w[1];
      Object.assign(e.style, { width: (r.x1 - r.x0 + 12).toFixed(0) + 'px', height: (r.y1 - r.y0 + 12).toFixed(0) + 'px', transform: `translate(${(r.x0 - 6).toFixed(1)}px, ${(r.y0 - 6).toFixed(1)}px)` });
    });
    if (!ringEl) { ringEl = tagLayer.appendChild(el('span', 'lring')); ltagEl = tagLayer.appendChild(el('span', 'ltag')); }
    const hl = hovLamp && devs[hovLamp.D.id] ? hovLamp : null; ringEl.hidden = ltagEl.hidden = !hl;
    if (hl) {
      hl.l.mesh.getWorldPosition(v1).project(camera); const x = (v1.x + 1) / 2 * host.clientWidth, y = (1 - v1.y) / 2 * host.clientHeight, a = hl.l.addr || '?', on = a in st.bits;
      ringEl.style.transform = `translate(${x.toFixed(1)}px, ${y.toFixed(1)}px)`; ringEl.style.borderStyle = hl.l.inScene ? 'solid' : 'dashed';
      const text = !hl.l.inScene ? T('r.lamp.out', { addr: a }) : `${a} ${!on ? T('r.unknown') : st.bits[a] ? T('r.on') : T('r.off')}`;
      if (ltagEl.textContent !== text) ltagEl.textContent = text; ltagEl.style.textDecoration = hl.l.inScene ? '' : 'line-through';
      ltagEl.style.transform = `translate(${x.toFixed(1)}px, ${(y - 14).toFixed(1)}px) translate(-50%, -100%)`;
    }
  }

  // 장면 올리기: 장치, 바닥판, 링크, 입출력 줄
  function load(sc, cat, links) {
    const keep = sel; clear();
    for (const dev of (sc && sc.devices) || []) if (dev && dev.id) devs[dev.id] = build(dev, cat);
    world.updateMatrixWorld(true);
    const b = new THREE.Box3(); for (const D of Object.values(devs)) for (const p of corners(D)) b.expandByPoint(p);
    if (b.isEmpty()) b.set(new THREE.Vector3(-300, 0, -200), new THREE.Vector3(300, 100, 200));
    const M = 70, w = b.max.x - b.min.x + 2 * M, d = b.max.z - b.min.z + 2 * M, cx = (b.min.x + b.max.x) / 2, cz = (b.min.z + b.max.z) / 2, base = new THREE.Group();
    base.position.set(cx, 0, cz);
    box(w, 14, d, { parent: base, tone: 'lo', pos: [0, -7, 0] });
    for (const x of [-1, 1]) for (const z of [-1, 1]) box(30, 12, 30, { parent: base, tone: 'lo', pos: [x * (w / 2 - 30), -20, z * (d / 2 - 30)] });
    const step = Math.max(50, d / 24); for (let z = -d / 2 + step; z < d / 2 - 1; z += step) box(w, 0.4, 3.5, { parent: base, tone: 'faint', pos: [0, 0.2, z] });
    const halo = new THREE.Mesh(new THREE.PlaneGeometry(w * 1.3, d * 1.5), new THREE.MeshBasicMaterial({ map: haloTex, transparent: true, depthWrite: false, color: C['--a-halo'], opacity: parseFloat(cssv('--a-halo-op')) || 0.3 }));
    halo.userData.raw = true; halo.rotation.x = -Math.PI / 2; halo.position.y = -26; halo.raycast = () => {}; base.add(halo);
    for (const m of base.children) m.raycast = () => {};
    world.add(base); restyle(base, null);
    for (const L of links || []) addLink(L);
    for (const D of Object.values(devs)) {
      if (!D.io.length) continue;
      const { box: sb, items } = lampRow(D.io, 'strip', D.dev.label || D.id, LO); tagLayer.append(sb);
      strips.push({ D, box: sb, items, w: sb.offsetWidth, h: sb.offsetHeight }); sb.classList.add('hide');
    }
    sel = devs[keep] ? keep : null;
    const src = Object.values(devs).find(D => (D.K.behaviors || []).includes('source')); wpSize = src ? num(par(src, 'part_mm'), 40) : 40;
    return b;
  }
  // 시점 맞춤: 장면 상자가 화면의 약 70%를 채운다 (craft.md 6절)
  function fit(b) {
    b = b || new THREE.Box3().setFromObject(world);
    const c = b.getCenter(new THREE.Vector3()), dist = Math.max(3000, b.getSize(v1).length() * 3), az = rad(45), elv = rad(30);
    controls.target.copy(c);
    ortho.position.set(c.x + dist * Math.cos(elv) * Math.sin(az), c.y + dist * Math.sin(elv), c.z + dist * Math.cos(elv) * Math.cos(az));
    ortho.zoom = 1; ortho.lookAt(c); ortho.updateMatrixWorld(true);
    fitX = fitY = 1;
    for (let i = 0; i < 8; i++) { v1.set(i & 1 ? b.max.x : b.min.x, i & 2 ? b.max.y : b.min.y - 30, i & 4 ? b.max.z : b.min.z).applyMatrix4(ortho.matrixWorldInverse); fitX = Math.max(fitX, Math.abs(v1.x)); fitY = Math.max(fitY, Math.abs(v1.y)); }
    persp.position.copy(c).addScaledVector(v1.copy(ortho.position).sub(c).normalize(), fitY / Math.tan(rad(persp.fov / 2)) * 1.5);
    resize(); controls.update();
  }
  function resize() {
    const w = host.clientWidth, h = host.clientHeight; if (!w || !h) return;
    renderer.setSize(w, h, false);
    // 쪽이 가린 오른쪽 폭과 아래 높이(카드, 재생 막대)는 빼고 남은 칸 안에 맞춘다 (F1, TW208). 기본값 0이면 식은 옛 식과 같다.
    const fw = Math.max(1, w - inset.right), fh = Math.max(1, h - inset.bottom);
    const fill = fw < 700 ? 0.9 : 0.72, upp = Math.max(fitX * 2 / (fw * fill), fitY * 2 / (fh * (sc2 ? 0.84 : 0.7))); // 형식 2: 뒤판 때문에 키가 큰 장면이라 높이의 84% (TW154)
    ortho.left = -fw / 2 * upp; ortho.right = (w - fw / 2) * upp; ortho.top = fh / 2 * upp; ortho.bottom = (fh / 2 - h) * upp; ortho.updateProjectionMatrix();
    persp.aspect = w / h; persp.updateProjectionMatrix(); request();
  }
  new ResizeObserver(resize).observe(host);

  // ---- 상태를 그림에 반영 ----
  function apply(now) {
    if (sc2) return apply2(now);
    const bit = a => !!(a && st.bits[a]), alarmed = new Set((st.alarms || []).map(a => a.device)), ph = (now / 600) % 1;
    camera.getWorldDirection(v2);
    let flowing = false;
    for (const D of Object.values(devs)) {
      const on = sel === D.id, F = { cut: cutAll || on, flow: flowAll || on, bit }, io = D.dev.io || {}, al = alarmed.has(D.id) && (blink || reduced);
      if (F.cut) for (const q of D.planes) { q.getWorldPosition(v1); const n = q.userData.plane.normal.set(0, 0, 1).transformDirection(q.matrixWorld); if (n.dot(v2) < 0) n.negate(); q.userData.plane.constant = -n.dot(v1); } // 카메라 쪽 절반을 뗀다
      for (const P of D.parts) {
        const a = P.slot && io[P.slot];
        P.state = P.role === 'led' ? (bit(a) ? 'sensor' : null) : P.role === 'coil' && bit(a) ? 'coil' : P.role === 'lamp' ? (bit(a) ? P.lamp : null) : P.role === 'liquid' ? 'liquid' : al && P.tone === 'hi' && !P.role ? 'alarm' : on ? 'lit' : null;
        if (P.plane) { const pl = F.cut ? [P.plane] : NONE; P.mat.clippingPlanes = pl; if (P.lm) P.lm.clippingPlanes = pl; if (P.hm) P.hm.clippingPlanes = pl; }
      }
      for (const f of D.upd) f(st, F);
      for (const s of D.sprites) { s.sp.visible = bit(io[s.slot]); s.sp.material.color.copy(C[s.colv]); }
    }
    // 흐름 점: 그 링크의 장치가 움직이는 동안만. 공급 = 파랑(밸브에서 실린더로), 배기 = 주황(반대로)
    for (const T of tubes) {
      if (!T.pts) continue;
      let on = false, supply = true;
      for (const [d, port] of T.ends) {
        const ax = st.axes[d], A = port === 'cap' || port === 'valve_a', B = port === 'rod' || port === 'valve_b', P = port === 'valve_p', R = port === 'valve_r' || port === 'vacuum';
        if (!ax || !ax.moving || !(A || B || P || R)) continue;
        on = true; supply = P || (!R && (ax.spool === 'fwd') === A);
      }
      const show = on && (flowAll || T.ends.some(([d]) => d === sel)); T.pts.visible = show; if (!show) continue;
      flowing = true; T.pts.material.color.copy(C[supply ? '--coil' : '--exhaust']);
      const a = T.pts.geometry.attributes.position, dir = supply ? T.vdir : -T.vdir;
      for (let i = 0; i < 10; i++) { let u = ((i + ph) / 10) % 1; if (dir < 0) u = 1 - u; T.curve.getPointAt(u, v1); a.setXYZ(i, v1.x, v1.y, v1.z); }
      a.needsUpdate = true;
    }
    for (const P of parts) paint(P);
    ghostMat.color.copy(C['--a-hi']);
    return flowing;
  }
  // 장치의 화면 상자 (부품 상자 8개 꼭짓점을 투영)
  function screenBox(D) {
    let x0 = 1e9, x1 = -1e9, y0 = 1e9, y1 = -1e9; const w = host.clientWidth, h = host.clientHeight;
    for (const p of corners(D)) { p.project(camera); const sx = (p.x + 1) / 2 * w, sy = (1 - p.y) / 2 * h; x0 = Math.min(x0, sx); x1 = Math.max(x1, sx); y0 = Math.min(y0, sy); y1 = Math.max(y1, sy); }
    return { x0, x1, y0, y1 };
  }
  // 입출력 줄: HTML이라 시점이 돌아도 읽힌다. 겹치면 아래로 한 줄씩 민다
  const tagLayer = el('div', 'tags'); host.append(tagLayer);
  function placeStrips() {
    const vis = [];
    const many = sc2 && stripsAll && strips.length > 12; // 줄이 많으면 램프만 (TW155)
    for (const s of strips) { const on = stripsAll || sel === s.D.id; s.box.classList.toggle('hide', !on); if (s.ax) s.ax.hidden = !!(st.joints && st.joints[s.D.id]); if (on && sc2) { s.box.classList.toggle('mini', many && sel !== s.D.id && hovId !== s.D.id); s.w = s.box.offsetWidth; s.h = s.box.offsetHeight; } if (on) { const r = screenBox(s.D); s.x = (r.x0 + r.x1) / 2 - s.w / 2; s.y = r.y1 + 4; vis.push(s); lampPaint(s.items, st, LO); } }
    vis.sort((a, b) => a.y - b.y || a.x - b.x);
    for (let i = 0; i < vis.length; i++) {
      const s = vis[i];
      for (let n = 0; n < 12; n++) { const o = vis.slice(0, i).find(p => s.x < p.x + p.w + 4 && p.x < s.x + s.w + 4 && s.y < p.y + p.h + 2 && p.y < s.y + s.h + 2); if (!o) break; s.y = o.y + o.h + 2; }
      s.box.style.transform = `translate(${s.x.toFixed(1)}px, ${s.y.toFixed(1)}px)`;
    }
    // 경보 꼬리표: 경보가 있는 장치 위에 글로도 알린다 (색과 깜빡임만으로 알리지 않는다)
    const want = dict(); for (const a of st.alarms || []) if (devs[a.device] && !want[a.device]) want[a.device] = a.tag || T('r.alarm');
    for (const id in atags) if (!want[id]) { atags[id].remove(); delete atags[id]; }
    for (const id in want) {
      const t = atags[id] || (atags[id] = tagLayer.appendChild(el('span', 'atag'))), text = '▲ ' + want[id], r = screenBox(devs[id]);
      if (t.textContent !== text) t.textContent = text;
      t.style.transform = `translate(${((r.x0 + r.x1) / 2).toFixed(1)}px, ${(r.y0 - 6).toFixed(1)}px) translate(-50%, -100%)`;
    }
  }
  const atags = dict();
  function request() { if (active && !pending) { pending = true; requestAnimationFrame(frame); } }
  // 경보 깜빡임: 경보가 있고 화면이 보이는 동안만 0.5초마다 한 프레임
  function syncBlink() {
    const al = active && !reduced && (st.alarms || []).length > 0;
    if (al && !blinkT) blinkT = setInterval(() => { blink = !blink; request(); }, 500);
    if (!al && blinkT) { clearInterval(blinkT); blinkT = 0; blink = true; }
  }
  function frame(now) {
    pending = false; if (!active) return;
    let more = false;
    for (const id in tw) { // 읽기 사이의 위치를 잇는다 (움직일 때만 프레임이 이어진다)
      const ax = st.axes[id], t = tw[id]; if (!ax) continue;
      const k = t.dur > 0 ? clamp((now - t.t0) / t.dur, 0, 1) : 1; ax.draw = t.from + (t.to - t.from) * k; if (k < 1 && t.from !== t.to) more = true;
      if (t.gt != null) { ax.gdraw = t.gf + (t.gt - t.gf) * k; if (k < 1 && t.gf !== t.gt) more = true; }
    }
    for (const dv in tj) for (const nm in tj[dv]) { // 엔진 관절 값을 읽기 사이에 잇는다 (회전은 가까운 쪽으로)
      const t = tj[dv][nm], k = t.dur > 0 ? clamp((now - t.t0) / t.dur, 0, 1) : 1, J = devs[dv] && devs[dv].jt[nm], wrap = J && J.j.wrap;
      t.cur = t.from.map((a, i) => { let d = (t.to[i] ?? a) - a; if ((J && J.j.type === 'planar' ? i === 2 : wrap) && i < 3) d = ((((d + 180) % 360) + 360) % 360) - 180; return a + d * k; });
      if (k < 1 && t.cur.some((c, i) => c !== (t.to[i] ?? c))) more = true;
    }
    if (drawParts(now)) more = true;
    const flowing = apply(now);
    if (sc2 && W2) placePoints();
    renderer.render(scene, camera); placeStrips(); if (sc2) { placeSk(); placeWk(); } frames++;
    if (opts.onFrame) opts.onFrame();
    if (more || (flowing && !reduced)) request();
  }

  // 편집 손잡이 (바닥 위에서만 옮김)
  const tc = new TransformControls(camera, cv);
  tc.showY = false; tc.setSize(0.8); tc.addEventListener('change', request);
  // 형식 2 편집 (A7): 손잡이는 장치 그룹이 아니라 같은 부모 틀 아래의 빈 물체(proxy)에 붙는다. 그래서 세 화살표가 `on` 틀의 축이 된다.
  // 끌 때마다 장치 그룹을 옮겨 미리 보이고(그 위의 장치는 틀 그룹을 따라간다), 끝나면 pose에 더할 변화량만 page에 준다.
  let adj = null, adjMode = 'move';
  const yawOf = q => 2 * Math.atan2(q.y, q.w) * 180 / Math.PI;
  function adjAttach(id) {
    const D = devs[id]; if (!D) return;
    D.g.updateMatrix(); const x = D.rd.frames && D.rd.frames[D.rd.datum] && D.rd.frames[D.rd.datum].xf, m0 = D.g.matrix.clone();
    const dp = new THREE.Vector3(x ? num(x[9], 0) : 0, x ? num(x[10], 0) : 0, x ? num(x[11], 0) : 0).applyMatrix4(m0); // 기준 틀(datum) 원점, 부모 틀 안의 자리
    const proxy = new THREE.Object3D(); proxy.position.copy(dp); proxy.userData.id = id; D.g.parent.add(proxy);
    adj = { D, proxy, dp, m0 };
    tc.setSpace('local'); tc.setMode(adjMode === 'turn' ? 'rotate' : 'translate'); tc.showX = tc.showZ = adjMode !== 'turn'; tc.showY = true; tc.attach(proxy);
  }
  function adjDetach() { if (adj) { adj.proxy.removeFromParent(); adj = null; } }
  function adjPreview() {
    if (!adj) return;
    const d = adj.proxy.position.clone().sub(adj.dp), th = adjMode === 'turn' ? rad(yawOf(adj.proxy.quaternion)) : 0;
    const m = new THREE.Matrix4().makeTranslation(adj.dp.x + d.x, adj.dp.y + d.y, adj.dp.z + d.z).multiply(new THREE.Matrix4().makeRotationY(th)).multiply(new THREE.Matrix4().makeTranslation(-adj.dp.x, -adj.dp.y, -adj.dp.z)).multiply(adj.m0);
    putM(adj.D.g, m); request();
  }
  tc.addEventListener('objectChange', adjPreview);
  tc.addEventListener('dragging-changed', e => {
    controls.enabled = !e.value;
    if (sc2) { // 끝: 변화량 = 손잡이 물체가 시작점에서 움직인 만큼 (mm와 도는 1 단위로 반올림)
      if (e.value || !adj) return;
      const id = adj.D.id, m0 = adj.m0, d = adj.proxy.position.clone().sub(adj.dp), dyaw = adjMode === 'turn' ? Math.round(yawOf(adj.proxy.quaternion)) : 0, dpos = [Math.round(d.x), Math.round(d.y), Math.round(d.z)];
      adjDetach();
      if (dyaw || dpos.some(Boolean)) { if (opts.onAdjust) opts.onAdjust(id, { dpos: dpos.some(Boolean) ? dpos : null, dyaw: dyaw || null }); } else { putM(devs[id].g, m0); adjAttach(id); request(); } // 바뀐 것이 없으면 제자리로
      return;
    }
    if (!e.value && tc.object && opts.onMove) { const p = tc.object.position; opts.onMove(tc.object.userData.id, [Math.round(p.x), Math.round(p.y), Math.round(p.z)]); }
  });
  scene.add(tc.getHelper());

  function setHandleMode(m) { adjMode = m === 'turn' ? 'turn' : 'move'; if (adj) { const id = adj.D.id; putM(adj.D.g, adj.m0); adjDetach(); tc.detach(); adjAttach(id); } request(); }
  // 시험용: 손잡이를 끈 것과 같은 길(objectChange, dragging-changed)로 변화량을 준다
  function dragTest(id, a) {
    if (!sc2 || !devs[id]) return false;
    if (!adj || adj.D.id !== id) { sel = id; tc.detach(); adjDetach(); adjAttach(id); }
    setHandleMode(a.dyaw ? 'turn' : 'move');
    if (a.dpos) adj.proxy.position.add(new THREE.Vector3(...a.dpos));
    if (a.dyaw) adj.proxy.rotation.y = rad(a.dyaw);
    tc.dispatchEvent({ type: 'objectChange' }); tc.dispatchEvent({ type: 'dragging-changed', value: false }); return true;
  }

  // 고르기와 누르기: 누른 장치를 알리고(시뮬의 버튼), 끌지 않았으면 고른 것으로 본다
  const ray = new THREE.Raycaster(), pt = new THREE.Vector2(); let down = null, downId = null;
  function hit(e) {
    const r = cv.getBoundingClientRect(); pt.set((e.clientX - r.left) / r.width * 2 - 1, -(e.clientY - r.top) / r.height * 2 + 1); ray.setFromCamera(pt, camera);
    let o = (ray.intersectObjects(Object.values(devs).map(D => D.g), true).find(i => !sc2 || vis(i.object)) || {}).object; while (o && !o.userData.id) o = o.parent;
    return o ? o.userData.id : null;
  }
  let downPart = null; // 부품이 12 px 안에 있으면 장치보다 먼저 고른다 (TW177)
  cv.addEventListener('pointerdown', e => { if (tc.axis || e.button) return; down = [e.clientX, e.clientY]; downPart = partNear(e.clientX, e.clientY); downId = downPart != null ? null : hit(e); if (downId && opts.onDown) opts.onDown(downId); });
  const up = e => {
    if (!down) return;
    const id = downId, pid = downPart, still = Math.hypot(e.clientX - down[0], e.clientY - down[1]) <= 4; down = null; downId = null; downPart = null;
    if (id && opts.onUp) opts.onUp(id);
    if (still && e.type === 'pointerup') { if (pid != null && opts.onPickPart) opts.onPickPart(pid); else if (opts.onPick) opts.onPick(id); }
  };
  cv.addEventListener('pointerup', up); cv.addEventListener('pointercancel', up);
  // 가리킴 (마우스만, 형식 2): 괄호와 가까운 표시등의 고리·꼬리표
  cv.addEventListener('pointermove', e => {
    if (!sc2 || e.pointerType !== 'mouse' || down) return;
    const r = cv.getBoundingClientRect(), id = hit(e), l = lampNear(e.clientX - r.left, e.clientY - r.top), near = l && (() => { l.l.mesh.getWorldPosition(v1).project(camera); return Math.hypot(e.clientX - r.left - (v1.x + 1) / 2 * r.width, e.clientY - r.top - (1 - v1.y) / 2 * r.height) <= 10; })();
    const nl = near ? l : null, hp = partNear(e.clientX, e.clientY);
    if (id !== hovId || (nl && nl.l) !== (hovLamp && hovLamp.l) || hp !== hovPart) { hovId = id; hovLamp = nl; hovPart = hp; request(); }
  });
  cv.addEventListener('pointerleave', () => { if (hovId || hovLamp || hovPart != null) { hovId = null; hovLamp = null; hovPart = null; request(); } });

  return {
    request, frames: () => frames, selected: () => sel, busy: () => !!tc.dragging, // 손잡이를 끄는 중이면 쪽이 장면을 바꾸지 않는다
    // resolved가 있으면 형식 2 그리기, 없으면 형식 1 (오늘의 코드)
    // 형식 2 장면이 resolved 없이 오면 형식 1 코드로 그리지 않는다(빈 장면)
    load(sc, cat, links, refit, resolved) { if (sc && sc.schema === 2 && !resolved) sc = { devices: [] }; const b =resolved ? load2(sc, cat, resolved) : (sc2 = false, load(sc, cat, links)); if (refit) fit(b); request(); },
    schema2: () => sc2, lampAddr,
    // 공작물 (TW168~TW178): 켜고 끄기, 지점 보기, 고른 부품, 장면의 종류·지점·매거진, 지금의 매거진 내용과 요약
    work: {
      set(o) { if ('on' in o) wkOn = !!o.on; if ('points' in o) ptsAll = !!o.points; if ('part' in o) selPart = o.part == null ? null : String(o.part); request(); },
      types: () => (W2 ? W2.types : null), points: () => (W2 ? W2.plist : []), stacks: () => (W2 ? W2.stacks : []), info: () => wkInfo,
      probe: () => ({ parts: Object.keys(wp).length, meshes: Object.keys(pw).length, tris: pD ? pD.tris : 0, tags: wkEls.filter(e => !e.hidden).map(e => e.textContent), crosses: ptsObj && ptsObj.visible ? ptsObj.geometry.drawRange.count / 6 : 0 }),
      place(id) { const t = wp[id]; return t ? t.cur.map(x => Math.round(x * 10) / 10) : null; },
      // 시험용: 부품 중심의 화면 자리 (CSS px, 그림 칸 기준)
      screen(id) { const t = wp[id], ty = t && typeOf(t.p); if (!ty) return null; v1.set(t.cur[0], t.cur[1] + ty.h / 2, t.cur[2]).project(camera); return { x: Math.round((v1.x + 1) / 2 * host.clientWidth), y: Math.round((1 - v1.y) / 2 * host.clientHeight) }; },
      // 벨트 위 부품의 들어오는 끝에서 잰 거리 (카드의 "자리")
      belt(id) {
        const t = wp[id]; if (!t) return null; world.updateMatrixWorld(true);
        for (const D of Object.values(devs)) {
          if (!D.belt || !D.fg.surface) continue; const len = num(D.rd.dims && D.rd.dims.length_mm && D.rd.dims.length_mm.v, 600), l = D.fg.surface.worldToLocal(new THREE.Vector3(t.cur[0], t.cur[1], t.cur[2]));
          if (Math.abs(l.y) < 10 && Math.abs(l.x) <= len / 2 + 30) return { dev: D.id, s: l.x + len / 2 };
        }
        return null;
      },
    },
    addrKnown: a => known.has(a),
    // 시험과 편집(A7)이 읽는 것: 틀 그룹의 세계 자리, 외형 그룹과 표시등의 상태
    probe: {
      frame(id, f) { const g = devs[id] && devs[id].fg[f]; if (!g) return null; world.updateMatrixWorld(true); return g.getWorldPosition(new THREE.Vector3()).toArray().map(x => Math.round(x * 100) / 100); },
      // 세계 점의 화면 자리(CSS px, 그림 칸 기준). 시점 맞춤이 관절 사슬을 칸 안에 넣는지 보는 시험용
      project(p) { camera.updateMatrixWorld(true); v1.set(p[0], p[1], p[2]).project(camera); return [Math.round((v1.x + 1) / 2 * host.clientWidth), Math.round((1 - v1.y) / 2 * host.clientHeight)]; },
      group(id, name) { const gr = devs[id] && devs[id].groups.find(x => x.name === name); return gr ? vis(gr.g) : null; },
      lamp(id, addr) { const l = devs[id] && devs[id].lamps.find(x => x.addr === addr); return l ? { lit: l.on, inScene: l.inScene } : null; },
      belt(id) { const b = devs[id] && devs[id].belt; return b ? { n: b.marks.length, color: '#' + b.bm.color.getHexString() } : null; },
      box(id) { const D = devs[id]; if (!D) return null; world.updateMatrixWorld(true); const b = new THREE.Box3(); for (const p of corners(D)) b.expandByPoint(p); return { min: b.min.toArray(), max: b.max.toArray() }; },
      // 외형 그룹들의 세계 상자 (자식 장치는 틀 그룹 아래라 들어가지 않는다). 회전한 장치의 외형이 서 있는지 보는 검사용
      skinBox(id) { const D = devs[id]; if (!D || !D.groups.length) return null; world.updateMatrixWorld(true); const b = new THREE.Box3(); for (const gr of D.groups) b.expandByObject(gr.g); return b.isEmpty() ? null : { min: b.min.toArray().map(x => Math.round(x)), max: b.max.toArray().map(x => Math.round(x)) }; },
      // 시험용: 시점을 한 점으로 옮기고 확대한다 (사진용 가까운 그림)
      look(p, zoom) { const d = new THREE.Vector3(...p).sub(controls.target); ortho.position.add(d); controls.target.add(d); ortho.zoom = zoom; ortho.updateProjectionMatrix(); controls.update(); request(); },
      drag: dragTest, handle: () => (adj ? { id: adj.D.id, mode: adjMode, axes: [tc.showX, tc.showY, tc.showZ] } : null),
      tones: () => Object.fromEntries(SKIN_TONES.map(t => ['#' + SK[t].fill.getHexString(), t])),
    },
    // 새 상태. dur 동안 앞의 위치에서 새 위치로 잇는다
    setState(s, dur = 0) {
      const now = performance.now();
      for (const id in s.axes) {
        const old = st.axes[id], a = s.axes[id], from = old && old.draw != null ? old.draw : a.pos, gf = old && old.gdraw != null ? old.gdraw : a.ghost;
        tw[id] = { from, to: a.pos, gf, gt: a.ghost, t0: now, dur }; a.draw = from; a.gdraw = gf;
      }
      const jr = s.joints && typeof s.joints === 'object' ? s.joints : null;
      for (const dv in tj) if (!jr || !(dv in jr)) delete tj[dv]; // 이 상태에 관절 값이 없으면(라이브, 재생) 쉬는 자리로
      for (const dv in jr || {}) {
        const row = jr[dv]; if (!row || typeof row !== 'object') continue;
        const o = tj[dv] = tj[dv] || dict();
        for (const nm in row) { const to = [].concat(row[nm]).map(x => num(x, 0)), old = o[nm], from = old ? old.cur : to; o[nm] = { from, to, cur: from, t0: now, dur }; }
      }
      const prev = wp; wp = dict();
      if (sc2 && W2) wp = partsState(s, prev, now, dur); else
      for (const p of s.parts || []) { const to = [num(p.x, 0), partY(p), num(p.z, 0)], o = prev[p.id]; wp[p.id] = { from: o ? o.cur : to, to, cur: o ? o.cur : to, t0: now, dur }; }
      st = s; syncBlink(); request();
    },
    ghost(on) { ghostAll = on; request(); },
    // AI에게 줄 그림(F7): 사진의 시점에서 한 장 그리고, 장치마다 그림 안의 상자(0~1)를 준다.
    // 사람이 보는 카메라와 조작은 건드리지 않는다(다른 카메라 객체로 그리고, 크기와 화소 비율만 되돌린다).
    shot(cam, aspect, maxPx) {
      const w = Math.max(1, Math.round(aspect >= 1 ? maxPx : maxPx * aspect)), h = Math.max(1, Math.round(aspect >= 1 ? maxPx / aspect : maxPx));
      const sc = new THREE.PerspectiveCamera(cam.fov, w / h, 10, 400000); sc.position.set(...cam.pos); sc.lookAt(...cam.target); sc.updateMatrixWorld(true);
      const pr = renderer.getPixelRatio(), keep = camera, helper = tc.getHelper(), hv = helper.visible, out = document.createElement('canvas'), boxes = [];
      try {
        camera = sc; helper.visible = false; renderer.setPixelRatio(1); renderer.setSize(w, h, false);
        world.updateMatrixWorld(true); apply(performance.now()); renderer.render(scene, sc);
        out.width = w; out.height = h; const g = out.getContext('2d'); g.fillStyle = cssv('--a-paper') || '#fff'; g.fillRect(0, 0, w, h); g.drawImage(cv, 0, 0, w, h); // 같은 턴 안에 옮겨야 그림이 남아 있다
        for (const D of Object.values(devs)) {
          let x0 = 1, x1 = 0, y0 = 1, y1 = 0, n = 0;
          for (const p of corners(D)) { p.project(sc); if (Math.abs(p.z) > 1) continue; n++; x0 = Math.min(x0, (p.x + 1) / 2); x1 = Math.max(x1, (p.x + 1) / 2); y0 = Math.min(y0, (1 - p.y) / 2); y1 = Math.max(y1, (1 - p.y) / 2); }
          x0 = clamp(x0, 0, 1); x1 = clamp(x1, 0, 1); y0 = clamp(y0, 0, 1); y1 = clamp(y1, 0, 1);
          if (n && x1 > x0 && y1 > y0) boxes.push({ device: D.id, x: +x0.toFixed(4), y: +y0.toFixed(4), w: +(x1 - x0).toFixed(4), h: +(y1 - y0).toFixed(4) }); // 그림 밖의 장치는 뺀다
        }
      } finally { camera = keep; helper.visible = hv; renderer.setPixelRatio(pr); resize(); }
      return { canvas: out, width: w, height: h, boxes };
    },
    select(id, handles) {
      sel = devs[id] ? id : null; tc.detach(); adjDetach();
      if (sc2) { if (sel && handles) adjAttach(sel); } // 형식 2: 부모 틀 축의 손잡이 (A7)
      else { tc.setSpace('world'); tc.setMode('translate'); tc.showX = tc.showZ = true; tc.showY = false; if (sel && handles) tc.attach(devs[sel].g); } // 형식 1: 오늘의 바닥 손잡이
      request();
    },
    // 손잡이 모드: 'move'(옮기기, 세 화살표) 또는 'turn'(돌리기, y축 고리만)
    handleMode: setHandleMode,
    screen(id) { const D = devs[id]; if (!D) return null; const r = screenBox(D); return { x: r.x1, y: (r.y0 + r.y1) / 2, y1: r.y1 }; },
    flow(on) { flowAll = on; request(); }, cut(on) { cutAll = on; request(); }, strips(on) { stripsAll = on; request(); },
    fit() { fit(); request(); },
    // 쪽이 그림 위에 얹은 것의 크기(오른쪽 카드 폭, 아래 막대 높이): 맞춤이 그 밖의 칸에 맞춘다 (F1)
    setInset(o) { inset = { right: Math.max(0, num(o && o.right, 0)), bottom: Math.max(0, num(o && o.bottom, 0)) }; resize(); },
    // 사진 맞춤: 원근 카메라로 바꾼다. 저장하는 시점은 {pos, target, fov}
    photo(on, fov) { camera = on ? persp : ortho; if (fov) { persp.fov = fov; persp.updateProjectionMatrix(); } controls.object = camera; tc.camera = camera; controls.update(); request(); },
    camera: () => ({ pos: persp.position.toArray().map(Math.round), target: controls.target.toArray().map(Math.round), fov: Math.round(persp.fov) }),
    setCamera(c) { persp.position.set(...c.pos); controls.target.set(...c.target); persp.fov = c.fov; persp.updateProjectionMatrix(); controls.update(); request(); },
    // 숨은 탭: 그리지 않는다
    active(on) { active = on; syncBlink(); if (on) request(); },
  };
}
