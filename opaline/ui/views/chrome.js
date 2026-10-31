// Barre de fenêtre, navigation du rail, colonne d'amis repliable.
import * as api from '../api.js';
import { emit } from '../state.js';
import { $, $$, LS } from '../util.js';

const win = $('#app');
let view = 'home';
export const currentView = () => view;

export function show(v, signed = true) {
  if (!signed) v = 'login';
  view = v;
  $$('.view').forEach((e) => e.classList.toggle('on', e.id === 'v-' + v));
  $$('#rail button[data-view]').forEach((b) => (b.dataset.view === v ? b.setAttribute('aria-current', 'page') : b.removeAttribute('aria-current')));
  $('#main').scrollTop = 0;
  emit('view', v);
}

const drawer = (o) => { win.classList.toggle('drawer', o); $('#colToggle').setAttribute('aria-expanded', o); };
const mq = matchMedia('(max-width:1180px)');
function applyCol() {
  const v = LS.get('opaline.col'), mini = v ? v === 'mini' : mq.matches, b = $('#colExpand');
  win.classList.toggle('colmini', mini); b.setAttribute('aria-expanded', !mini);
  const t = mini ? 'Afficher les amis' : 'Réduire la liste'; b.setAttribute('aria-label', t); b.title = t;
}

export function initChrome(signedRef) {
  $$('#rail button[data-view]').forEach((b) => b.addEventListener('click', () => show(b.dataset.view, signedRef())));
  $('#rail').addEventListener('keydown', (e) => {
    const bs = $$('#rail button:not([hidden])'), i = bs.indexOf(document.activeElement); if (i < 0) return;
    const n = { ArrowDown: (i + 1) % bs.length, ArrowUp: (i - 1 + bs.length) % bs.length, Home: 0, End: bs.length - 1 }[e.key];
    if (n !== undefined) { e.preventDefault(); bs[n].focus(); }
  });
  document.addEventListener('click', (e) => { const g = e.target.closest('[data-go]'); if (g) show(g.dataset.go, signedRef()); });

  $('#colToggle').addEventListener('click', () => drawer(!win.classList.contains('drawer')));
  document.addEventListener('keydown', (e) => { if (e.key === 'Escape' && win.classList.contains('drawer')) { drawer(false); $('#colToggle').focus(); } });
  $('#main').addEventListener('click', () => drawer(false));
  $('#colExpand').addEventListener('click', () => { LS.set('opaline.col', win.classList.contains('colmini') ? 'full' : 'mini'); applyCol(); });
  mq.addEventListener('change', applyCol);
  $('#col').addEventListener('click', (e) => { if (win.classList.contains('colmini') && e.target.closest('.pl')) { LS.set('opaline.col', 'full'); applyCol(); } });
  applyCol();

  // Fenêtre : Tauri (decorations: false) gère le glisser via data-tauri-drag-region ; en démo on simule agrandir/restaurer.
  const setMax = (m) => { win.classList.toggle('max', m); const b = $('#wMax'); b.setAttribute('aria-label', m ? 'Restaurer' : 'Agrandir'); b.firstElementChild.firstElementChild.setAttribute('href', m ? '#i-restore' : '#i-max'); };
  $('#wMin').addEventListener('click', () => api.windowApi.minimize());
  $('#wClose').addEventListener('click', () => api.windowApi.close());
  $('#wMax').addEventListener('click', async () => { if (api.isTauri) await api.windowApi.toggleMaximize(); else setMax(!win.classList.contains('max')); });
  if (api.isTauri) { document.body.classList.add('tauri'); const sync = async () => setMax(await api.windowApi.isMaximized()); api.windowApi.onResized(sync); sync(); }
  else $('#titlebar').addEventListener('dblclick', (e) => { if (!e.target.closest('.wbtns')) setMax(!win.classList.contains('max')); });
}
