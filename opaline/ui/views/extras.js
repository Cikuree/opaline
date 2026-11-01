import * as api from '../api.js';
import { state, current } from '../state.js';
import { $, $$, esc, toast, tex } from '../util.js';

const busy = () => !!state.install || state.inGame;
const fmtSize = (b) => (b >= 1e6 ? (b / 1e6).toFixed(1) + ' Mo' : Math.max(1, Math.round(b / 1e3)) + ' Ko');
const fmtDl = (n) => (n >= 1e6 ? (n / 1e6).toFixed(1).replace('.0', '') + ' M' : n >= 1e3 ? Math.round(n / 1e3) + ' k' : String(n));
const LIMIT = 10;
let showAll = false, shownFor = null;
const isJar = (n) => /\.jar$/i.test(n);

export function syncExtrasBusy() {
  $$('#extraCard button').forEach((b) => { if (b.id !== 'mrGo' || busy()) b.disabled = busy(); });
}

export async function renderExtras() {
  const p = current(); if (!p) return;
  const ok = p.installed && p.supported;
  $('#extraCard').hidden = !ok; if (!ok) return;
  let list = []; try { list = await api.extraModsList(p.id); } catch (e) { toast(api.errMsg(e)); }
  if (current()?.id !== p.id) return;
  if (shownFor !== p.id) { shownFor = p.id; showAll = false; }
  const row = (m) => `<li><span class="xt"><b>${esc(m.title || m.file)}</b> <span class="mute small">${m.title ? esc(m.file) + ' · ' : ''}${esc(fmtSize(m.sizeBytes))}</span>${m.auto ? `<span class="tag" style="margin-left:6px">Dépendance${m.requiredBy?.length ? ' de ' + esc(m.requiredBy.join(', ')) : ''}</span>` : ''}${m.warning ? `<span class="small xw">${esc(m.warning)}</span>` : ''}</span><button class="btn sm" data-rm="${esc(m.file)}">Retirer</button></li>`;
  $('#extraBox').innerHTML = list.length ? `<ul class="bk">${(showAll ? list : list.slice(0, LIMIT)).map(row).join('')}</ul>${list.length > LIMIT ? `<button class="btn sm more" id="extraMore">${showAll ? 'Réduire' : `Tout afficher (${list.length})`}</button>` : ''}` : '<p class="mute small" style="margin-top:10px">Aucun mod perso pour l’instant.</p>';
  syncExtrasBusy();
}

async function addFiles(files) {
  const p = current(); if (!p || busy()) return toast('Ferme le jeu avant de modifier Mes mods.');
  for (const f of files) {
    if (!isJar(f.name)) { toast(`« ${f.name} » : seuls les fichiers .jar sont acceptés.`); continue; }
    try {
      const data = await new Promise((res, rej) => { const r = new FileReader(); r.onload = () => res(String(r.result).split(',')[1] || ''); r.onerror = () => rej('Fichier illisible'); r.readAsDataURL(f); });
      await api.extraModImport(p.id, f.name, data); toast(`« ${f.name} » ajouté`);
    } catch (e) { toast(api.errMsg(e)); }
  }
  renderExtras();
}

async function addPaths(paths) {
  const p = current(); if (!p || busy()) return toast('Ferme le jeu avant de modifier Mes mods.');
  for (const path of paths) {
    const name = path.split(/[\\/]/).pop();
    if (!isJar(name)) { toast(`« ${name} » : seuls les fichiers .jar sont acceptés.`); continue; }
    try { await api.extraModAdd(p.id, path); toast(`« ${name} » ajouté`); } catch (e) { toast(api.errMsg(e)); }
  }
  renderExtras();
}

async function search() {
  const p = current(), q = $('#mrQ').value.trim(); if (!p || !q) return;
  $('#mrBox').innerHTML = '<p class="mute small" style="margin-top:10px">Recherche…</p>';
  try {
    const hits = await api.modrinthSearch(p.id, q);
    $('#mrBox').innerHTML = hits.length ? `<ul class="bk">${hits.map((h) => `<li><span class="mrh">${h.iconUrl && /^https:\/\/cdn\.modrinth\.com\//.test(h.iconUrl) ? `<img class="px" src="${esc(h.iconUrl)}" alt="" width="40" height="40" referrerpolicy="no-referrer">` : tex('diamond', 40)}<span class="xt"><b>${esc(h.title)}</b><span class="small mute">${esc(h.description)}</span><span class="xs mute">${esc(fmtDl(h.downloads))} téléchargements</span></span></span><button class="btn sm primary" data-add="${esc(h.projectId)}" data-t="${esc(h.title)}" ${busy() ? 'disabled' : ''}>Ajouter</button></li>`).join('')}</ul>` : '<p class="mute small" style="margin-top:10px">Aucun résultat compatible avec ce pack.</p>';
  } catch (e) { $('#mrBox').innerHTML = ''; toast(api.errMsg(e)); }
}

export function initExtras() {
  $('#extraAdd').addEventListener('click', () => $('#extraFile').click());
  $('#extraFile').addEventListener('change', (e) => { addFiles([...e.target.files]); e.target.value = ''; });
  const drop = $('#extraDrop');
  ['dragenter', 'dragover'].forEach((t) => drop.addEventListener(t, (e) => { e.preventDefault(); drop.classList.add('on'); }));
  ['dragleave', 'drop'].forEach((t) => drop.addEventListener(t, () => drop.classList.remove('on')));
  drop.addEventListener('drop', (e) => { e.preventDefault(); addFiles([...e.dataTransfer.files]); });
  // Application Tauri : la fenêtre intercepte le glisser-déposer et fournit des chemins (HTML5 ne reçoit rien).
  api.listen('tauri://drag-drop', (d) => { if ($('#v-files').classList.contains('on') && d?.paths?.length) addPaths(d.paths); });
  $('#mrGo').addEventListener('click', search);
  $('#mrQ').addEventListener('keydown', (e) => { if (e.key === 'Enter') search(); });
  $('#mrBox').addEventListener('click', async (e) => {
    const b = e.target.closest('[data-add]'); if (!b) return;
    const p = current(); b.disabled = true; b.textContent = 'Ajout…';
    try {
      const r = await api.modrinthAdd(p.id, b.dataset.add);
      const t = r.added.title;
      toast(r.deps.length ? `${t} ajouté avec : ${r.deps.map((d) => d.title).join(', ')}` : `${t} ajouté`);
      $('#extraMsg').textContent = [r.deps.length ? `${t} ajouté avec : ${r.deps.map((d) => d.title).join(', ')}.` : `${t} ajouté.`, r.provided.length ? `Déjà fourni, non réinstallé : ${r.provided.join(', ')}.` : ''].filter(Boolean).join(' ');
      b.textContent = 'Ajouté'; renderExtras();
    } catch (er) { b.disabled = false; b.textContent = 'Ajouter'; toast(api.errMsg(er)); }
  });
  $('#extraBox').addEventListener('click', async (e) => {
    if (e.target.closest('#extraMore')) { showAll = !showAll; return renderExtras(); }
    const b = e.target.closest('[data-rm]'); if (!b) return;
    b.disabled = true;
    try { const gone = await api.extraModRemove(current().id, b.dataset.rm); toast(gone.length > 1 ? `Retiré : ${gone.join(', ')} (dépendances comprises)` : 'Mod retiré'); } catch (er) { toast(api.errMsg(er)); }
    renderExtras();
  });
}
