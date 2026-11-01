// Menu d'un pack installé (clic droit sur une carte de l'accueil, ou bouton burger de « Fichiers & mods ») et confirmation de désinstallation.
// Mêmes garde-fous partout : jamais pendant le jeu ni une installation ; le backend refuse aussi (jeu, monde ouvert) et sauvegarde les mondes.
import * as api from '../api.js';
import { state, emit } from '../state.js';
import { $, $$, toast } from '../util.js';

let menuPack = null, menuFrom = null, dlgPack = null, dlgFrom = null;

/** Élément à refocaliser : l'ancien s'il est encore dans la page, sinon la carte du pack (l'accueil se redessine souvent). */
const back = (el, id) => (el?.isConnected ? el : document.querySelector(`#packs [data-k="${CSS.escape(id || '')}"]`));

const busyMsg = () => (state.inGame ? 'Ferme le jeu avant cette action.' : state.install ? 'Attends la fin de l’installation en cours.' : '');

export async function repairPack(id) {
  const m = busyMsg(); if (m) return toast(m);
  state.install = { packId: id, stage: 'java', done: 0, total: 1 }; emit('install');
  try { await api.packRepair(id); } catch (e) { state.install = null; emit('install'); toast(api.errMsg(e)); }
}

export function closeMenu(refocus = true) {
  const m = $('#pkMenu'); if (m.hidden) return;
  m.hidden = true; $('#fileBurger').setAttribute('aria-expanded', 'false');
  if (refocus) back(menuFrom, menuPack)?.focus();
  menuPack = null;
}

/** Ouvre le menu du pack `id` en (x, y) (coordonnées fenêtre) ; `from` reprend le focus à la fermeture. */
export function openMenu(id, x, y, from) {
  const m = $('#pkMenu'); menuPack = id; menuFrom = from; m.hidden = false;
  const w = m.offsetWidth, h = m.offsetHeight;
  m.style.left = Math.max(8, Math.min(x, innerWidth - w - 8)) + 'px'; m.style.top = Math.max(8, Math.min(y, innerHeight - h - 8)) + 'px';
  $('button', m).focus();
}

export function askUninstall(id, from) {
  const p = state.packs.find((x) => x.id === id); if (!p) return;
  const m = busyMsg(); if (m) return toast(m);
  dlgPack = id; dlgFrom = from; $('#dlgT').textContent = `Désinstaller ${p.name} ?`; $('#uninst').hidden = false; $('#uninstNo').focus();
}

function closeDialog(refocus = true) {
  $('#uninst').hidden = true;
  if (refocus) back(dlgFrom, dlgPack)?.focus();
  dlgPack = null;
}

async function doUninstall() {
  const id = dlgPack; closeDialog(false); if (!id) return;
  const p = state.packs.find((x) => x.id === id);
  try {
    await api.packUninstall(id);
    toast(`${p?.name || 'Pack'} désinstallé : tes mondes sont dans les sauvegardes`);
    const next = state.packs.find((x) => x.installed && x.id !== id);
    if (state.pack === id && next) emit('selectPack', next.id);
    emit('reloadPacks');
  } catch (e) { toast(api.errMsg(e)); }
}

export function initPackMenu() {
  const menu = $('#pkMenu');
  menu.addEventListener('click', (e) => {
    const b = e.target.closest('[data-a]'); if (!b) return;
    const id = menuPack, from = menuFrom; closeMenu(false);
    if (b.dataset.a === 'open') api.openFolder(id, 'instance').catch((er) => toast(api.errMsg(er)));
    else if (b.dataset.a === 'repair') repairPack(id);
    else askUninstall(id, from);
  });
  menu.addEventListener('keydown', (e) => {
    const items = $$('button', menu), i = items.indexOf(document.activeElement);
    const n = { ArrowDown: (i + 1) % items.length, ArrowUp: (i - 1 + items.length) % items.length, Home: 0, End: items.length - 1 }[e.key];
    if (n !== undefined) { e.preventDefault(); items[n].focus(); } else if (e.key === 'Tab') { e.preventDefault(); closeMenu(); }
  });
  document.addEventListener('click', (e) => { if (!menu.hidden && !e.target.closest('#pkMenu, #fileBurger')) closeMenu(false); });
  document.addEventListener('contextmenu', (e) => { if (!menu.hidden && !e.target.closest('#pkMenu')) closeMenu(false); }, true);
  const dlg = $('#uninst');
  $('#uninstOk').addEventListener('click', doUninstall);
  $('#uninstNo').addEventListener('click', () => closeDialog());
  dlg.addEventListener('click', (e) => { if (e.target === dlg) closeDialog(); });
  dlg.addEventListener('keydown', (e) => {
    if (e.key === 'Tab') { const a = [$('#uninstNo'), $('#uninstOk')], i = a.indexOf(document.activeElement); e.preventDefault(); a[(i + 1) % 2].focus(); }
  });
  document.addEventListener('keydown', (e) => {
    if (e.key !== 'Escape') return;
    if (!dlg.hidden) closeDialog(); else if (!menu.hidden) closeMenu();
  });
}
