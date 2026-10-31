import { ICONS, HEADS, HGRID } from './data.js';
import * as api from './api.js';

export const $ = (s, r = document) => r.querySelector(s);
export const $$ = (s, r = document) => [...r.querySelectorAll(s)];
export const esc = (t) => String(t ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
export const LS = {
  get(k) { try { return localStorage.getItem(k); } catch { return null; } },
  set(k, v) { try { localStorage.setItem(k, v); } catch { /* stockage indisponible */ } },
};

let tt;
export function toast(m) { const t = $('#toast'); t.textContent = m; t.classList.add('on'); clearTimeout(tt); tt = setTimeout(() => t.classList.remove('on'), 2800); }
export function copy(text, msg) {
  try { navigator.clipboard?.writeText(text).catch(() => {}); } catch { /* presse-papiers refusé */ }
  toast(msg);
}

// ---- durées ----
export const hms = (ms) => { const s = Math.max(0, Math.floor(ms / 1000)); return `${Math.floor(s / 3600)}:${String(Math.floor(s / 60) % 60).padStart(2, '0')}:${String(s % 60).padStart(2, '0')}`; };
export function ago(v) {
  if (!v) return '';
  const t = Date.parse(v); if (Number.isNaN(t)) return String(v);
  const m = Math.round((Date.now() - t) / 60000);
  if (m < 2) return 'à l’instant'; if (m < 60) return `il y a ${m} min`;
  if (m < 60 * 24) return `il y a ${Math.round(m / 60)} h`; return `il y a ${Math.round(m / 1440)} j`;
}

// ---- pixel-art de repli ----
const rects = (grid, pal) => grid.map((row, y) => [...row].map((c, x) => (pal[c] ? `<rect x="${x}" y="${y}" width="1" height="1" fill="${pal[c]}"/>` : '')).join('')).join('');
export const iconSvg = (n, s = 36) => { const i = ICONS[n] || ICONS.diamond; return `<svg class="px" aria-hidden="true" viewBox="0 0 8 8" width="${s}" height="${s}">${rects(i.g, i.p)}</svg>`; };
const hash = (s) => [...String(s)].reduce((a, c) => (a * 31 + c.charCodeAt(0)) >>> 0, 7);
const headPal = (n) => { const [h, s, e] = HEADS[hash(n) % HEADS.length]; return { h, s, w: '#fff', b: e, n: '#b5806a', m: '#8a4d3c' }; };
const pixelHead = (name, s) => `<svg class="px" aria-hidden="true" viewBox="0 0 8 8" width="${s}" height="${s}">${rects(HGRID, headPal(name))}</svg>`;

/** Texture Minecraft `<nom>.png` (client.jar de l'utilisateur) ; repli pixel-art si absente. */
export function tex(name, size = 32, fb = 'diamond', cls = '') {
  const u = api.texUrl(name);
  return u ? `<img class="mt ${cls}" src="${esc(u)}" alt="" width="${size}" height="${size}" data-fb="${fb}">` : iconSvg(fb, size).replace('class="px"', `class="px mt ${cls}"`);
}
document.addEventListener('error', (e) => {
  const i = e.target;
  if (i.tagName === 'IMG' && i.dataset.fb) { const cls = i.className; i.outerHTML = iconSvg(i.dataset.fb, i.width || 32).replace('class="px"', `class="px ${cls}"`); }
}, true);

// ---- têtes de joueurs : player_head (mis en cache côté backend), pixel-art le temps du chargement ----
const heads = new Map();
export const headHtml = (name, uuid, s = 44) => `<span data-head="${esc(uuid || '')}" data-s="${s}" style="display:contents">${pixelHead(name, s)}</span>`;
export function hydrateHeads(root = document) {
  $$('[data-head]', root).forEach(async (el) => {
    const uuid = el.dataset.head; if (!uuid || el.dataset.done) return; el.dataset.done = '1';
    if (!heads.has(uuid)) heads.set(uuid, api.playerHead(uuid).then((r) => r.dataUrl).catch(() => null));
    const url = await heads.get(uuid);
    if (url && /^data:image\/png;base64,[A-Za-z0-9+/=]+$/.test(url) && el.isConnected) el.innerHTML = `<img class="px" src="${url}" alt="" width="${Number(el.dataset.s)}" height="${Number(el.dataset.s)}">`;
  });
}

// ---- barre de progression (grille) ----
export function setProgress(on, title = '', sub = '', pct = 0) {
  const p = $('#prog'); p.classList.toggle('on', on); if (!on) return;
  $('#progT').textContent = title; $('#progS').textContent = sub;
  const v = Math.round(Math.min(100, Math.max(0, pct)));
  $('#bar').style.width = v + '%'; $('#pct').textContent = v + ' %'; $('#track').setAttribute('aria-valuenow', v);
}

export const setSw = (el, v) => el.setAttribute('aria-checked', v);
