// Original: crates/twin-page/ui/twin-core.js. Copied for docs/; recopy when the original changes.
// 두 3D 화면이 같이 쓰는 함수 (reusable-parts 3.1e, 설계 47의 6절): plc-mcp의 3D 탭(`twin.html`)과 twin-modeling-mcp의 공개 화면(`twin-model.html`).
// 이 파일에는 화면 글이 없다(글은 `t(키, 값)`으로 들어온다). 서버와 말하는 길, 상태 모양, 숫자 글, 그림 요청 답을 한 곳에 둔다. `/vendor/twin-core.js`로 나간다.
import { dict } from '/vendor/twin-scene.js';

export const $ = id => document.getElementById(id);
export const el = (tag, cls, text) => { const e = document.createElement(tag); if (cls) e.className = cls; if (text != null) e.textContent = text; return e; };
export const enc = encodeURIComponent, clamp01 = v => Math.min(1, Math.max(0, v));
// 브라우저 저장소. 접두사는 화면마다 다르다(`twin.`, `twinModel.`). 저장하지 못해도 화면은 된다
export const makeKeep = prefix => ({
  get(k, d) { try { const v = localStorage.getItem(prefix + k); return v == null ? d : v; } catch (e) { return d; } },
  set(k, v) { try { localStorage.setItem(prefix + k, v); } catch (e) { /* 저장 못 해도 화면은 된다 */ } },
});

// 서버와 말하는 길. `lang`(글자 또는 함수)이 있으면 머리글 `X-Page-Lang`을 보낸다(서버가 그 말로 오류 글을 준다, TW207).
// `translate`는 오류 글을 사람의 글로 바꾸고(plc-mcp: 정규식 표, 공개 화면: 그대로), `net`은 서버에 닿지 않을 때의 글이다.
export function makeApi({ token, lang, translate, net }) {
  const val = x => (typeof x === 'function' ? x() : x);
  return async function api(method, path, body, blob) {
    const o = { method, headers: { 'X-Dashboard-Token': token } };
    const l = val(lang); if (l) o.headers['X-Page-Lang'] = l;
    if (body instanceof Blob) { o.body = body; o.headers['Content-Type'] = 'image/jpeg'; }
    else if (body !== undefined) { o.body = JSON.stringify(body); o.headers['Content-Type'] = 'application/json'; }
    try {
      const r = await fetch(path, o);
      if (blob && r.ok) return { ok: true, blob: await r.blob() };
      const v = await r.json().catch(() => ({}));
      return r.ok ? { ok: true, v } : { ok: false, status: r.status, error: translate(v.error || 'HTTP ' + r.status) };
    } catch (e) { return { ok: false, status: 0, error: val(net) }; }
  };
}

// 시뮬의 가상 입력 한 번 (누르기, 핀 값 넣기)
export const simInput = (api, sim, device, value) => api('POST', '/api/ladder/sim/input', { sim, device, value });

// 숫자 글: 소수 한 자리, 관절 값 한 줄 (설계 45 2.2): 회전 "45.0°", 직동과 형태 "120.0 mm", 평면 "x 628.3 mm · z -314.2 mm · 90.0°"
export const f1 = x => (Math.round((Number(x) || 0) * 10) / 10 + 0).toFixed(1);
export const jointText = (type, v) => (type === 'planar' ? (v = Array.isArray(v) ? v : [], `x ${f1(v[0])} mm · z ${f1(v[1])} mm · ${f1(v[2])}°`) : type === 'revolute' ? `${f1(v)}°` : `${f1(v)} mm`);
// `{이름}` 자리를 값으로 바꾼다
export const fmt = (s, v) => { if (v) for (const n in v) s = s.split('{' + n + '}').join(v[n]); return s; };
// 구동기 상태 글 (설계 45 2.4). 서버는 부호만 준다. `t(키, 값)`이 글을 준다
export const drvText = (d, moving, t) => d.type === 'playback' ? (d.running ? t('drv.playback', { n: d.program }) : t('drv.stop')) : d.type === 'step' ? (d.moving ? t('drv.step') : t('drv.stop'))
  : d.type === 'coils' ? ({ fwd: t('drv.coils.fwd'), back: t('drv.coils.back') }[d.spool] || t('drv.coils.none')) : moving ? t('drv.follow') : t('drv.follow.at');

// 시뮬의 답(`sim/*`)을 그림의 상태 모양으로. 알람은 화면이 더한다
export const stateFrom = v => {
  const p = v.plant || {};
  return { bits: dict(v.bits), words: dict(v.words), axes: dict(p.axes), lights: dict(p.lights), levels: dict(p.levels), plant: p.alarms || [], parts: p.parts || [], forced: Array.isArray(p.forced) ? p.forced : [], stacks: dict(p.stacks), joints: p.joints && typeof p.joints === 'object' ? dict(p.joints) : null, jmov: dict(p.joints_moving), drv: dict(p.drivers), partLog: Array.isArray(p.part_log) ? p.part_log : [], logCut: !!p.part_log_truncated };
};

// 장면 목록의 도장: 보이는 장면이나 초안의 파일이 바뀌었는지 (PT14)
export const stampKey = it => (it ? `${it.scene_ms}|${it.draft_ms}` : '');
// 고장 목록을 `sim/start`의 `faults` 모양으로
export const faultListOf = faults => Object.entries(faults).map(([device, f]) => ({ device, fault: f.fault, ...(f.fault === 'slow' ? { factor: f.factor } : {}), ...(f.addr ? { [f.fault === 'coil_dead' ? 'coil' : 'sensor']: f.addr } : {}) }));

// AI에게 줄 그림 한 장: 요청의 시점으로 그리고 JPEG(1 MiB 이하)와 장치 상자를 보낸다. 그림을 못 보내도 상자는 보낸다
export async function renderShot(api, twin, q, aspect) {
  const id = '?id=' + enc(q.id);
  const shot = twin.shot(q.camera, aspect, Math.min(1280, Number(q.max_px) || 1280));
  let blob = null; for (const quality of [0.85, 0.7, 0.5]) { blob = await new Promise(res => shot.canvas.toBlob(res, 'image/jpeg', quality)); if (blob && blob.size <= 1048576) break; blob = null; }
  if (blob) await api('POST', '/api/twin/render/image' + id, blob);
  await api('POST', '/api/twin/render/answer' + id, { width: shot.width, height: shot.height, boxes: shot.boxes });
}
export const renderFail = (api, q, reason) => api('POST', '/api/twin/render/fail?id=' + enc(q.id), { reason: String(reason).slice(0, 80) });
