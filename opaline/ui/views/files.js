import * as api from '../api.js';
import { state, current, emit } from '../state.js';
import { $, esc, toast, tex } from '../util.js';
import { MODTEX, loaderName } from '../data.js';
import { statusOf, startInstall, changelogHtml } from './home.js';

const busy = () => !!state.install || state.inGame; // pas d'installation/réparation pendant une partie ou une autre installation
export function syncBusy() {
  const p = current(); if (!p) return;
  $('#btnRepair').disabled = !p.installed || !p.supported || busy();
  const a = $('#packCard [data-act]'); if (a) a.disabled = busy();
}

const FOLDERS = [['mods', 'Dossier mods', 'barrel_side'], ['shaderpacks', 'Dossier shaderpacks', 'glowstone_dust'], ['screenshots', 'Captures d’écran', 'painting'], ['logs', 'Logs', 'writable_book'], ['instance', 'Instance', 'furnace_front']];
const SIDE = { client: 'Client', server: 'Serveur' };

export async function renderFiles() {
  const p = current(); if (!p) return;
  const st = statusOf(p);
  $('#filePacks').innerHTML = state.packs.map((q) => `<button role="radio" aria-checked="${q.id === p.id}" data-k="${esc(q.id)}">${esc(q.name)}</button>`).join('');
  $('#folders').innerHTML = FOLDERS.map((f) => `<button data-f="${f[0]}" ${p.installed ? '' : 'disabled'}><span class="ic">${tex(f[2], 32, 'gear')}</span>${f[1]}</button>`).join('');
  let mods = []; if (p.installed && p.supported) { try { mods = await api.packMods(p.id); } catch (e) { toast(api.errMsg(e)); } }
  if (current()?.id !== p.id) return;
  $('#modsBox').innerHTML = mods.length
    ? `<table style="margin-top:8px"><thead><tr><th>Nom</th><th>Version</th><th>Côté</th><th>Hash</th></tr></thead><tbody>${mods.map((m) => `<tr><td><span style="display:flex;align-items:center;gap:10px">${tex(MODTEX[m.name] || 'diamond', 32)}${esc(m.name)}</span></td><td>${esc(m.version)}</td><td><span class="tag">${SIDE[m.side] || 'Les deux'}</span></td><td>${m.verified ? '<span class="ok">✓ Vérifié</span>' : '<span style="color:var(--warn);font-weight:700">! À vérifier</span>'}</td></tr>`).join('')}</tbody></table>`
    : `<p class="mute small" style="margin-top:10px">${p.supported ? `Pack non installé. Les ${Number(p.modsCount)} mods se téléchargent à l’installation.` : 'Pack non pris en charge par cette version d’Opaline.'}</p>`;
  let chg = ''; if (p.installed && p.updateAvailable) { try { chg = `<div class="note" style="margin-top:0"><h3>Changelog des mods</h3>${changelogHtml(await api.packChangelog(p.id))}</div>`; } catch { /* sans changelog */ } }
  if (current()?.id !== p.id) return;
  const pill = st === 'upd' ? 'Mise à jour disponible' : st === 'ok' ? 'À jour' : st === 'unsupported' ? 'Non pris en charge' : 'Non installé';
  $('#packCard').innerHTML = `<h2>Pack ${esc(p.name)}</h2><p style="font-size:30px;font-weight:800;margin:10px 0 6px">${esc(p.mc)} · ${esc(loaderName(p.loader))}</p><span class="pill ${st === 'upd' ? 's-upd' : st === 'ok' ? '' : 's-no'}">${pill}</span><p class="small mute" style="margin:12px 0">${esc(p.description)}</p>${chg}`
    + (st === 'upd' ? '<button class="btn primary" style="margin-top:14px" data-act="1">Mettre à jour</button>' : st === 'no' ? '<button class="btn primary" style="margin-top:14px" data-act="1">Installer</button>' : '');
  syncBusy();
}

export function initFiles() {
  $('#folders').addEventListener('click', async (e) => {
    const b = e.target.closest('[data-f]'); if (!b) return;
    try { await api.openFolder(current().id, b.dataset.f); } catch (er) { toast(api.errMsg(er)); }
  });
  $('#packCard').addEventListener('click', (e) => { if (e.target.closest('[data-act]')) startInstall(current().id); });
  $('#btnRepair').addEventListener('click', async () => {
    const p = current(); if (busy()) return toast('Ferme le jeu avant de réparer.'); state.install = { packId: p.id, stage: 'mods', done: 0, total: 1 }; emit('install');
    try { await api.packRepair(p.id); } catch (e) { state.install = null; emit('install'); toast(api.errMsg(e)); }
  });
}
