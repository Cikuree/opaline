import { state } from '../state.js';
import { $, esc } from '../util.js';
import { fmtPlay } from './home.js';

// Temps de jeu par pack : une forme organique par pack (taille = part du temps), le total au centre.
const COLORS = ['#f07a5a', '#a66ad6', '#f3b9b4', '#e8a15a', '#7fb3d5'];
const N = 64, C = 150; // points par forme, centre du viewBox 300×300
let raf = 0;

function blob(r, seed, t) {
  const ox = Math.cos(seed) * 9, oy = Math.sin(seed) * 9; // chaque forme décalée d'un côté : même la plus petite dépasse
  const pts = [];
  for (let k = 0; k < N; k++) {
    const a = (k / N) * Math.PI * 2;
    const w = 0.1 * Math.sin(3 * a + t * 0.6 + seed) + 0.07 * Math.sin(5 * a - t * 0.45 + seed * 2) + 0.04 * Math.sin(7 * a + t * 0.8 + seed * 3);
    pts.push([C + ox + Math.cos(a) * r * (1 + w), C + oy + Math.sin(a) * r * (1 + w)]);
  }
  // Catmull-Rom fermé vers des courbes de Bézier
  let d = `M${pts[0][0].toFixed(1)},${pts[0][1].toFixed(1)}`;
  for (let k = 0; k < N; k++) {
    const p0 = pts[(k - 1 + N) % N], p1 = pts[k], p2 = pts[(k + 1) % N], p3 = pts[(k + 2) % N];
    d += `C${(p1[0] + (p2[0] - p0[0]) / 6).toFixed(1)},${(p1[1] + (p2[1] - p0[1]) / 6).toFixed(1)} ${(p2[0] - (p3[0] - p1[0]) / 6).toFixed(1)},${(p2[1] - (p3[1] - p1[1]) / 6).toFixed(1)} ${p2[0].toFixed(1)},${p2[1].toFixed(1)}`;
  }
  return d + 'Z';
}

export function renderPlaytime() {
  cancelAnimationFrame(raf);
  const rows = state.packs.map((p, i) => ({ p, i, s: state.playtime?.[p.id]?.totalSecs || 0 })).filter((r) => r.s > 0).sort((a, b) => b.s - a.s);
  const total = rows.reduce((n, r) => n + r.s, 0), box = $('#ptBox');
  if (!total) { box.innerHTML = '<p class="mute small" style="margin-top:10px">Aucune session mesurée pour l’instant. Le temps de jeu est compté quand Opaline lance le jeu.</p>'; return; }
  const radius = (r) => 94 + 24 * Math.sqrt(r.s / total); // la plus grosse part dépasse le plus du disque central
  box.innerHTML = `<div class="pt"><svg viewBox="0 0 300 300" aria-hidden="true">${rows.map((r, k) => `<path data-k="${k}" fill="${COLORS[r.i % COLORS.length]}" fill-opacity="${0.78 - k * 0.12}"/>`).join('')}<circle cx="150" cy="150" r="82" fill="#200b10"/><text x="150" y="134" text-anchor="middle" class="ptl">Temps total</text><text x="150" y="174" text-anchor="middle" class="ptv">${esc(fmtPlay(total))}</text></svg>
    <ul class="ptlist">${rows.map((r) => `<li><span class="ptd" style="background:${COLORS[r.i % COLORS.length]}"></span><b>${esc(r.p.name)}</b><span class="mute">${esc(fmtPlay(r.s))} · ${Math.round((100 * r.s) / total)} %</span></li>`).join('')}</ul></div>`;
  const paths = [...box.querySelectorAll('path')], still = matchMedia('(prefers-reduced-motion: reduce)').matches;
  const draw = (ms) => {
    const t = ms / 1000;
    if (box.offsetParent) paths.forEach((el, k) => el.setAttribute('d', blob(radius(rows[k]), k * 1.7 + 0.4, t)));
    if (!still && box.isConnected) raf = requestAnimationFrame(draw); // rAF est suspendu fenêtre cachée
  };
  draw(0);
}
