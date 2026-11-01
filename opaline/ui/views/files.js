import * as api from '../api.js';
import { renderCopy } from './settings.js';
import { state, current, emit } from '../state.js';
import { $, $$, esc, toast, tex } from '../util.js';
import { MODTEX, loaderName } from '../data.js';
import { statusOf, startInstall, changelogHtml } from './home.js';
import { repairPack, askUninstall, openMenu, closeMenu } from './packmenu.js';
import { renderExtras, initExtras, syncExtrasBusy, hitsHtml } from './extras.js';

const LIMIT = 10;
let showAll = false, shownFor = null; // « Tout afficher » : mémorisé tant qu'on reste sur le même pack

const fmtSize = (b) => (b >= 1e9 ? (b / 1e9).toFixed(1) + ' Go' : b >= 1e6 ? Math.round(b / 1e6) + ' Mo' : Math.max(1, Math.round(b / 1e3)) + ' Ko');
const fmtDate = (iso) => new Date(iso).toLocaleString('fr-FR', { dateStyle: 'medium', timeStyle: 'short' });
let confirming = null; // id de la sauvegarde dont la restauration attend une confirmation

async function renderBackups() {
  const p = current(); if (!p) return;
  let list = []; try { list = await api.backupsList(p.id); } catch (e) { toast(api.errMsg(e)); }
  if (current()?.id !== p.id) return;
  const row = (b) => `<li><span><b>${esc(fmtDate(b.createdAt))}</b>${b.keep ? ' <span class="tag">Avant désinstallation</span>' : ''} <span class="mute small">· ${esc(fmtSize(b.sizeBytes))}</span></span>${confirming === b.id
    ? `<span class="bkc"><span class="small">Remplacer tes mondes actuels par cette sauvegarde ? Les mondes actuels seront d’abord sauvegardés.</span><button class="btn sm primary" data-ok="${esc(b.id)}">Oui, restaurer</button><button class="btn sm" data-no="1">Annuler</button></span>`
    : `<button class="btn sm" data-bk="${esc(b.id)}" ${busy() ? 'disabled' : ''}>Restaurer</button>`}</li>`;
  $('#backupsBox').innerHTML = list.length ? `<ul class="bk">${list.map(row).join('')}</ul>` : '<p class="mute small" style="margin-top:10px">Aucune sauvegarde pour l’instant.</p>';
}

const busy = () => !!state.install || state.inGame; // pas d'installation/réparation pendant une partie ou une autre installation
export function syncBusy() {
  const p = current(); if (!p) return;
  $('#btnRepair').disabled = !p.installed || !p.supported || busy();
  $('#btnUninst').disabled = !p.installed || busy();
  $('#fileBurger').disabled = !p.installed;
  $$('#backupsBox [data-bk]').forEach((b) => (b.disabled = busy()));
  $('#shSel').disabled = busy(); $$('#shaderCard button').forEach((b) => { b.disabled = busy(); });
  syncExtrasBusy();
  const a = $('#packCard [data-act]'); if (a) a.disabled = busy();
}

const FOLDERS = [['mods', 'Mods', 'barrel_side'], ['shaderpacks', 'Shaders & resource packs', 'glowstone_dust'], ['screenshots', 'Captures', 'painting'], ['logs', 'Journal', 'writable_book'], ['instance', 'Instance', 'furnace_front']];
let tab = 'mods'; try { tab = localStorage.getItem('opaline.filesTab') || tab; } catch { /* onglet par défaut */ }
let shots = [], shotAt = -1;
const ADD = { mods: '#extraFile', shaderpacks: '#packsFile' }; // onglets où « Ajouter un fichier… » a un sens

function showTab() {
  $$('#folders [data-f]').forEach((b) => b.setAttribute('aria-selected', b.dataset.f === tab));
  $$('#v-files .ftab').forEach((d) => (d.hidden = d.dataset.tab !== tab));
  if (tab === 'screenshots') renderShots(); else if (tab === 'logs') renderLog();
}

async function renderShots() {
  const p = current(); if (!p) return;
  try { shots = p.installed ? await api.screenshotsList(p.id) : []; } catch (e) { shots = []; toast(api.errMsg(e)); }
  if (current()?.id !== p.id) return;
  $('#shotsBox').innerHTML = shots.length
    ? `<div class="shots">${shots.map((f, i) => `<button data-shot="${i}" aria-label="${esc(f.name)}"><img src="${esc(f.url)}" alt="" loading="lazy"></button>`).join('')}</div>`
    : '<p class="mute small" style="margin-top:10px">Aucune capture pour l’instant. En jeu, F2 en prend une.</p>';
}

async function renderLog() {
  const p = current(); if (!p) return;
  let t = ''; try { t = p.installed ? await api.logTail(p.id) : ''; } catch (e) { toast(api.errMsg(e)); }
  if (current()?.id !== p.id) return;
  $('#logBox').textContent = t || 'Aucun journal : lance le jeu une première fois.';
  $('#logBox').scrollTop = $('#logBox').scrollHeight;
}

function openShot(i) {
  shotAt = (i + shots.length) % shots.length; const f = shots[shotAt];
  $('#lbImg').src = f.url; $('#lbName').textContent = f.name; $('#shotLb').hidden = false; $('#lbClose').focus();
}
const closeShot = () => { $('#shotLb').hidden = true; shotAt = -1; };
const SIDE = { client: 'Client', server: 'Serveur' };

export async function renderFiles() {
  renderCopy();
  const p = current(); if (!p) return;
  const inst = state.packs.find((q) => q.installed);
  if (!p.installed && inst && state.pack === p.id) return emit('selectPack', inst.id); // le sélecteur ne liste que les packs installés
  const st = statusOf(p);
  if (shownFor !== p.id) { shownFor = p.id; showAll = false; }
  $('#filePacks').innerHTML = state.packs.filter((q) => q.installed).map((q) => `<button role="radio" aria-checked="${q.id === p.id}" data-k="${esc(q.id)}">${esc(q.name)}</button>`).join('');
  $('#folders').innerHTML = FOLDERS.map((f) => `<button role="tab" data-f="${f[0]}" ${p.installed ? '' : 'disabled'}><span class="ic">${tex(f[2], 32)}</span>${f[1]}</button>`).join('');
  showTab();
  let mods = []; if (p.installed && p.supported) { try { mods = await api.packMods(p.id); } catch (e) { toast(api.errMsg(e)); } }
  if (current()?.id !== p.id) return;
  const shown = showAll ? mods : mods.slice(0, LIMIT);
  $('#modsBox').innerHTML = mods.length
    ? `<table style="margin-top:8px"><thead><tr><th>Nom</th><th>Version</th><th>Côté</th><th>Hash</th></tr></thead><tbody>${shown.map((m) => `<tr><td><span style="display:flex;align-items:center;gap:10px">${tex(MODTEX[m.name] || 'diamond', 32)}${esc(m.name)}</span></td><td>${esc(m.version)}</td><td><span class="tag">${SIDE[m.side] || 'Les deux'}</span></td><td>${m.verified ? '<span class="ok">✓ Vérifié</span>' : '<span style="color:var(--warn);font-weight:700">! À vérifier</span>'}</td></tr>`).join('')}</tbody></table>${mods.length > LIMIT ? `<button class="btn sm more" id="modsMore">${showAll ? 'Réduire' : `Tout afficher (${mods.length})`}</button>` : ''}`
    : `<p class="mute small" style="margin-top:10px">${p.supported ? `Pack non installé. Les ${Number(p.modsCount)} mods se téléchargent à l’installation.` : 'Pack non pris en charge par cette version d’Opaline.'}</p>`;
  let chg = ''; if (p.installed && p.updateAvailable) { try { chg = `<div class="note" style="margin-top:0"><h3>Changelog des mods</h3>${changelogHtml(await api.packChangelog(p.id))}</div>`; } catch { /* sans changelog */ } }
  if (current()?.id !== p.id) return;
  const pill = st === 'upd' ? 'Mise à jour disponible' : st === 'ok' ? 'À jour' : st === 'unsupported' ? 'Non pris en charge' : 'Non installé';
  $('#packCard').innerHTML = `<h2>Pack ${esc(p.name)}</h2><p style="font-size:30px;font-weight:800;margin:10px 0 6px">${esc(p.mc)} · ${esc(loaderName(p.loader))}</p><span class="pill ${st === 'upd' ? 's-upd' : st === 'ok' ? '' : 's-no'}">${pill}</span><p class="small mute" style="margin:12px 0">${esc(p.description)}</p>${chg}`
    + (st === 'upd' ? '<button class="btn primary" style="margin-top:14px" data-act="1">Mettre à jour</button>' : st === 'no' ? '<button class="btn primary" style="margin-top:14px" data-act="1">Installer</button>' : '');
  syncBusy(); renderBackups(); renderExtras(); renderShaders();
}

async function renderShaders() {
  const p = current(); if (!p) return;
  let v = null; if (p.installed && p.supported) { try { v = await api.shadersGet(p.id); } catch { /* section masquée */ } }
  if (current()?.id !== p.id) return;
  $('#shaderCard').hidden = !v; if (!v) return;
  $('#shOn').hidden = $('#shSel').hidden = !v.available; $('#shOff').hidden = v.available; $('#shIris').disabled = busy();
  if (!v.available) { $('#shMsg').textContent = ''; return; }
  $$('#shOn button').forEach((b) => { b.disabled = busy(); });
  $('#shSel').innerHTML = '<option value="">Aucun</option>' + v.files.map((f) => `<option value="${esc(f)}">${esc(f.replace(/\.zip$/i, ''))}</option>`).join('');
  $('#shSel').value = v.active || ''; $('#shSel').disabled = busy(); $('#shMsg').textContent = '';
}

export function initFiles() {
  $('#shSel').addEventListener('change', async (e) => {
    const p = current(); if (!p) return;
    try { await api.shaderSet(p.id, e.target.value || null); $('#shMsg').textContent = e.target.value ? 'Shader appliqué au prochain lancement.' : 'Shaders désactivés au prochain lancement.'; } catch (er) { toast(api.errMsg(er)); renderShaders(); }
  });
  initExtras(renderShaders);
  $('#shIris').addEventListener('click', async (e) => {
    const p = current(); e.target.disabled = true;
    try { await api.modrinthAdd(p.id, 'iris'); toast('Iris installé : les shaders sont prêts'); } catch (er) { toast(api.errMsg(er)); }
    renderShaders();
  });
  const shSearch = async () => {
    const p = current(), q = $('#shQ').value.trim(); if (!p || !q) return;
    $('#shBox').innerHTML = '<p class="mute small" style="margin-top:10px">Recherche…</p>';
    try { const hits = await api.shaderSearch(p.id, q); $('#shBox').innerHTML = hits.length ? hitsHtml(hits) : '<p class="mute small" style="margin-top:10px">Aucun shader compatible avec ce pack.</p>'; } catch (e) { $('#shBox').innerHTML = ''; toast(api.errMsg(e)); }
  };
  $('#shGo').addEventListener('click', shSearch);
  $('#shQ').addEventListener('keydown', (e) => { if (e.key === 'Enter') shSearch(); });
  $('#shZip').addEventListener('click', () => $('#packsFile').click());
  $('#shBox').addEventListener('click', async (e) => {
    const b = e.target.closest('[data-add]'); if (!b) return;
    const p = current(); b.disabled = true; b.textContent = 'Ajout…';
    try { await api.shaderAdd(p.id, b.dataset.add); toast(`${b.dataset.t} ajouté et activé`); b.textContent = 'Ajouté'; renderShaders(); renderExtras(); } catch (er) { b.disabled = false; b.textContent = 'Ajouter'; toast(api.errMsg(er)); }
  });
  $('#modsBox').addEventListener('click', (e) => { if (e.target.closest('#modsMore')) { showAll = !showAll; renderFiles(); } });
  $('#backupsOpen').addEventListener('click', async () => { try { await api.openFolder(current().id, 'backups'); } catch (er) { toast(api.errMsg(er)); } });
  $('#backupsBox').addEventListener('click', async (e) => {
    const t = e.target.closest('button'); if (!t) return;
    if (t.dataset.bk) { confirming = t.dataset.bk; return renderBackups(); }
    if (t.dataset.no) { confirming = null; return renderBackups(); }
    if (t.dataset.ok) {
      const id = current().id; confirming = null; t.disabled = true;
      try { await api.backupRestore(id, t.dataset.ok); toast('Mondes restaurés'); } catch (er) { toast(api.errMsg(er)); }
      renderBackups();
    }
  });
  $('#folders').addEventListener('click', (e) => {
    const b = e.target.closest('[data-f]'); if (!b) return;
    tab = b.dataset.f; try { localStorage.setItem('opaline.filesTab', tab); } catch { /* non mémorisé */ }
    showTab();
  });
  // Clic droit sur un onglet : ouvre son dossier
  $('#folders').addEventListener('contextmenu', async (e) => {
    const b = e.target.closest('[data-f]'); if (!b || b.disabled) return;
    e.preventDefault();
    const m = $('#tabMenu'); m.dataset.f = b.dataset.f; m.hidden = false;
    $('[data-m="add"]', m).hidden = !ADD[b.dataset.f];
    m.style.left = Math.min(e.clientX, innerWidth - m.offsetWidth - 8) + 'px'; m.style.top = Math.min(e.clientY, innerHeight - m.offsetHeight - 8) + 'px';
    $('button', m).focus();
  });
  const closeTabMenu = () => { $('#tabMenu').hidden = true; };
  $('#tabMenu').addEventListener('click', async (e) => {
    const it = e.target.closest('[data-m]'); if (!it) return;
    const f = $('#tabMenu').dataset.f; closeTabMenu();
    if (it.dataset.m === 'add') return $(ADD[f]).click();
    try { await api.openFolder(current().id, f); } catch (er) { toast(api.errMsg(er)); }
  });
  document.addEventListener('pointerdown', (e) => { if (!e.target.closest('#tabMenu')) closeTabMenu(); });
  document.addEventListener('keydown', (e) => { if (e.key === 'Escape') closeTabMenu(); });
  $('#v-files').addEventListener('click', async (e) => {
    const b = e.target.closest('[data-open]'); if (!b) return;
    try { await api.openFolder(current().id, b.dataset.open); } catch (er) { toast(api.errMsg(er)); }
  });
  $('#shotsBox').addEventListener('click', (e) => { const b = e.target.closest('[data-shot]'); if (b) openShot(Number(b.dataset.shot)); });
  $('#shotLb').addEventListener('click', (e) => {
    if (e.target === e.currentTarget || e.target.closest('#lbClose')) return closeShot();
    const n = e.target.closest('[data-d]'); if (n) openShot(shotAt + Number(n.dataset.d));
  });
  document.addEventListener('keydown', (e) => {
    if ($('#shotLb').hidden) return;
    if (e.key === 'Escape') closeShot(); else if (e.key === 'ArrowLeft') openShot(shotAt - 1); else if (e.key === 'ArrowRight') openShot(shotAt + 1);
  });
  $('#lbCopy').addEventListener('click', async () => {
    try {
      const png = await new Promise((res, rej) => { const im = new Image(); im.onload = () => { const c = document.createElement('canvas'); c.width = im.naturalWidth; c.height = im.naturalHeight; c.getContext('2d').drawImage(im, 0, 0); c.toBlob(res, 'image/png'); }; im.onerror = rej; im.src = $('#lbImg').src; });
      await navigator.clipboard.write([new ClipboardItem({ 'image/png': png })]); toast('Image copiée');
    } catch { toast('Copie impossible ici : ouvre le dossier.'); }
  });
  $('#logCopy').addEventListener('click', async () => { try { await navigator.clipboard.writeText($('#logBox').textContent); toast('Journal copié'); } catch { toast('Copie impossible'); } });
  $('#packCard').addEventListener('click', (e) => { if (e.target.closest('[data-act]')) startInstall(current().id); });
  $('#btnRepair').addEventListener('click', () => { const p = current(); if (p) repairPack(p.id); });
  $('#btnUninst').addEventListener('click', (e) => { const p = current(); if (p) askUninstall(p.id, e.currentTarget); });
  $('#fileBurger').addEventListener('click', (e) => {
    const b = e.currentTarget, p = current(); if (!p) return;
    if (b.getAttribute('aria-expanded') === 'true') return closeMenu();
    b.setAttribute('aria-expanded', 'true'); const r = b.getBoundingClientRect(); openMenu(p.id, r.right - 200, r.bottom + 6, b);
  });
}
