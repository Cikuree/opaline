import * as api from '../api.js';
import { state, official } from '../state.js';
import { $, $$, esc, toast, setSw } from '../util.js';
import { PERF, COPY_CATS } from '../data.js';

const maxGb = () => Math.max(2, Math.floor((state.settings?.systemRamMb || 16384) / 1024));
const gbOf = (mb) => Math.round(mb / 1024);
const clamp = (g) => Math.min(maxGb(), Math.max(2, g));

function memUi(gb) {
  const max = maxGb(), v = $('#mem');
  v.max = max; v.value = gb;
  $('#memV').textContent = gb + ' Go';
  // zone recommandée : 6 à 8 Go, bornée par la RAM détectée
  const lo = Math.min(6, max), hi = Math.min(8, max), pc = (g) => ((g - 2) / Math.max(1, max - 2)) * 100;
  $('#zone').style.setProperty('--z1', pc(lo) + '%'); $('#zone').style.setProperty('--z2', pc(hi) + '%');
  const ram = Math.round((state.settings?.systemRamMb || 0) / 1024);
  $('#memH').textContent = `RAM détectée : ${ram} Go. Zone recommandée : ${lo} à ${hi} Go (en vert).` + (gb > max - 4 && max > 6 ? ' Attention : peu de mémoire resterait pour le système.' : gb < 4 ? ' Attention : trop peu pour Minecraft avec des mods.' : '');
}

function renderCopy() {
  const inst = state.packs.filter((p) => p.installed), keep = (id) => ($(id).value || '');
  const opts = (sel, other) => { const v = keep(sel); $(sel).innerHTML = inst.map((p, i) => `<option value="${esc(p.id)}">${esc(p.name)}</option>`).join(''); $(sel).value = inst.some((p) => p.id === v) ? v : (inst[other]?.id || ''); };
  opts('#cpFrom', 0); opts('#cpTo', 1);
  if (!$('#cpCats').children.length) $('#cpCats').innerHTML = COPY_CATS.map(([k, l]) => `<label class="small" style="display:flex;gap:6px;align-items:center"><input type="checkbox" value="${k}" checked>${l}</label>`).join('');
  $('#cpGo').disabled = inst.length < 2;
  if (inst.length < 2) $('#cpMsg').textContent = 'Il faut au moins deux packs installés.';
}

export function renderSettings() {
  const s = state.settings; if (!s) return;
  renderCopy();
  $('#perf').innerHTML = PERF.map((p) => `<button role="radio" aria-checked="${p.id === s.perfMode}" data-p="${p.id}"><h3>${p.name}</h3><span class="xs" style="color:var(--coral2);font-weight:700">${p.sub}</span><ul><li>${p.gb ? 'Mémoire ' + clamp(p.gb) + ' Go' : 'Mémoire choisie à la main'}</li></ul></button>`).join('');
  memUi(clamp(gbOf(s.memoryMb)));
  setSw($('#noRemember'), !!s.noRemember);
  const via = s.launchVia === 'official' ? 'official' : 'opaline';
  $$('#launchVia [data-v]').forEach((b) => b.setAttribute('aria-checked', b.dataset.v === via));
  setSw($('#joinServer'), !!s.joinServer); setSw($('#notifyServer'), !!s.notifyServer); setSw($('#discordPresence'), !!s.discordPresence); setSw($('#trayOnGame'), !!s.trayOnGame);
  // Le Launcher officiel ne prend pas d'arguments de jeu de notre part : pas de connexion automatique dans ce mode.
  $('#jsH').textContent = official() || via === 'official'
    ? 'Indisponible avec le Launcher Minecraft : choisis le serveur dans Multijoueur (il y est ajouté).'
    : 'Le jeu se connecte tout seul au serveur du pack (Minecraft 1.20 ou plus). Sans serveur, ou si son adresse n’est pas encore connue, tu arrives au menu principal.';
  $('#lvH').textContent = official()
    ? 'Sans connexion Microsoft dans Opaline, le jeu passe toujours par le Launcher Minecraft.'
    : via === 'official' ? 'JOUER prépare le profil du pack dans le Launcher Minecraft, qui lance le jeu.' : 'Opaline lance le jeu directement avec ton compte.';
}

async function save(patch) {
  try { state.settings = await api.settingsSet(patch); } catch (e) { toast(api.errMsg(e)); state.settings = await api.settingsGet().catch(() => state.settings); }
  renderSettings();
}

export function initSettings() {
  $('#cpGo').addEventListener('click', async () => {
    const from = $('#cpFrom').value, to = $('#cpTo').value, cats = [...document.querySelectorAll('#cpCats input:checked')].map((i) => i.value);
    if (from === to) return ($('#cpMsg').textContent = 'Choisis deux packs différents.');
    if (!cats.length) return ($('#cpMsg').textContent = 'Coche au moins une catégorie.');
    if (state.inGame) return ($('#cpMsg').textContent = 'Ferme le jeu avant de copier des réglages.');
    $('#cpGo').disabled = true; $('#cpMsg').textContent = '';
    try { const n = await api.settingsCopy(from, to, cats); $('#cpMsg').textContent = `${n} réglage${n > 1 ? 's' : ''} copié${n > 1 ? 's' : ''}.`; toast('Réglages copiés'); } catch (e) { $('#cpMsg').textContent = api.errMsg(e); }
    $('#cpGo').disabled = false;
  });
  $('#perf').addEventListener('click', async (e) => {
    const b = e.target.closest('[data-p]'); if (!b) return;
    const p = PERF.find((x) => x.id === b.dataset.p);
    await save(p.gb ? { perfMode: p.id, memoryMb: clamp(p.gb) * 1024 } : { perfMode: 'custom' });
    toast('Mode ' + p.name + ' sélectionné');
  });
  $('#mem').addEventListener('input', () => memUi(+$('#mem').value));
  $('#mem').addEventListener('change', () => save({ perfMode: 'custom', memoryMb: +$('#mem').value * 1024 }));
  $('#noRemember').addEventListener('click', (e) => save({ noRemember: e.currentTarget.getAttribute('aria-checked') !== 'true' }));
  $('#joinServer').addEventListener('click', (e) => save({ joinServer: e.currentTarget.getAttribute('aria-checked') !== 'true' }));
  $('#notifyServer').addEventListener('click', (e) => save({ notifyServer: e.currentTarget.getAttribute('aria-checked') !== 'true' }));
  $('#discordPresence').addEventListener('click', (e) => save({ discordPresence: e.currentTarget.getAttribute('aria-checked') !== 'true' }));
  $('#trayOnGame').addEventListener('click', (e) => save({ trayOnGame: e.currentTarget.getAttribute('aria-checked') !== 'true' }));
  // Sans connexion Opaline, JOUER passe de toute façon par le Launcher Minecraft : ce choix ne vaut qu'une fois connecté.
  $('#launchVia').addEventListener('click', (e) => { const b = e.target.closest('[data-v]'); if (b) save({ launchVia: b.dataset.v }); });
}
