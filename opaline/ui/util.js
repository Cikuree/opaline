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

// ---- textures Minecraft (client.jar) : emplacement neutre tant qu'elles ne sont pas prêtes ----
const slot = (size, cls, name = '') => `<span class="mt ph ${cls}" ${name ? `data-tex="${esc(name)}" data-s="${size}" data-c="${esc(cls)}"` : ''} aria-hidden="true" style="width:${size}px;height:${size}px"></span>`;

/** Texture Minecraft `<nom>.png` ; emplacement neutre si indisponible. */
export function tex(name, size = 32, cls = '') {
  const u = api.texUrl(name);
  return u ? `<img class="mt ${esc(cls)}" src="${esc(u)}" alt="" width="${size}" height="${size}" data-tex="${esc(name)}" data-s="${size}" data-c="${esc(cls)}">` : slot(size, cls, name);
}
const retex = (el) => { el.outerHTML = tex(el.dataset.tex, Number(el.dataset.s) || 32, el.dataset.c || ''); };
/** Recharge toutes les images de texture déjà affichées (après `textures://ready`). */
export const refreshTextures = () => $$('[data-tex]').forEach(retex);
document.addEventListener('error', (e) => {
  const i = e.target;
  if (i.tagName === 'IMG' && i.dataset.tex) i.outerHTML = slot(Number(i.dataset.s) || 32, i.dataset.c || '');
}, true);

// ---- têtes de joueurs : player_head (mis en cache côté backend), emplacement neutre le temps du chargement ----
const heads = new Map();
export const headHtml = (name, uuid, s = 44) => `<span data-head="${esc(uuid || '')}" data-s="${s}" style="display:contents">${slot(s, '')}</span>`;
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
