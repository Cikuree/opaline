import * as api from '../api.js';
import { state, current, hasWidget, on, who, official } from '../state.js';
import { $, esc, ago, toast, headHtml, hydrateHeads } from '../util.js';

const row = (f) => {
  const off = !f.online, st = off ? 'hors ligne' : 'en ligne', sub = off ? 'Hors ligne' + (f.lastSeen ? ' · Vu ' + ago(f.lastSeen) : '') : f.inGame ? 'En jeu' : 'En ligne';
  const act = `<div class="plact"><button type="button" class="${f.friend ? 'ghost' : ''}" data-fa="${f.friend ? 'rm' : 'add'}" data-u="${esc(f.uuid)}">${f.friend ? 'Retirer' : 'Ajouter'}</button></div>`;
  return `<div class="plrow"><button type="button" class="pl ${off ? 'off' : ''}" data-tip="${esc(f.name)} · ${st}" aria-label="${esc(f.name)}, ${st}. ${esc(sub)}"><div class="avwrap"><div class="avatar">${headHtml(f.name, f.uuid, 44)}</div><span class="dot ${off ? 'idle' : ''}" role="img" aria-label="${off ? 'Hors ligne' : 'En ligne'}"></span></div>${!off && f.inGame ? '<span class="ingame">En jeu</span>' : ''}<div class="tx"><div class="n">${esc(f.name)}</div><div class="s">${esc(sub)}</div></div></button>${act}</div>`;
};

export function renderMe() {
  const p = who(), signed = !official() && p;
  $('#meName').textContent = p?.name || ''; $('#meAvatar').innerHTML = p ? headHtml(p.name, p.uuid, 44) : '';
  $('#meDot').hidden = !signed; $('#meGame').hidden = !state.inGame; $('#meSt').textContent = official() ? 'Launcher Minecraft' : state.inGame ? 'En jeu' : 'En ligne';
  hydrateHeads($('#meAvatar'));
}

export function renderFriends() {
  if (!hasWidget(current(), 'server')) { // pack sans serveur : pas d'amis
    $('#online').innerHTML = '<p class="xs mute tx" style="padding:6px 0">Pas de serveur pour ce pack.</p>'; $('#offline').innerHTML = ''; $('#onCount').textContent = 0;
    return;
  }
  const on_ = state.friends.filter((f) => f.online), off = state.friends.filter((f) => !f.online);
  $('#online').innerHTML = on_.length ? on_.map(row).join('') : '<p class="xs mute tx" style="padding:6px 0;text-align:center">Aucun ami en ligne.</p>';
  $('#offline').innerHTML = off.map(row).join(''); $('#onCount').textContent = on_.length;
  hydrateHeads($('#col'));
}

let refreshing = false;
export async function refreshFriends() {
  if (refreshing) return;
  refreshing = true;
  try { state.friends = await api.friends(); } catch { state.friends = []; } finally { refreshing = false; }
  renderFriends();
}
/** Après un ajout ou un retrait : attend la fin d'un relevé en cours puis en relance un, pour ne pas afficher une liste périmée. */
const refreshNow = async () => { while (refreshing) await new Promise((r) => setTimeout(r, 100)); await refreshFriends(); };

async function onAction(e) {
  const b = e.target.closest('[data-fa]'); if (!b) return;
  const f = state.friends.find((x) => x.uuid === b.dataset.u); if (!f) return;
  try {
    if (b.dataset.fa === 'add') { await api.friendAdd(f.name); toast(f.name + ' ajouté'); } else await api.friendRemove(f.uuid);
    await refreshNow();
  } catch (err) { toast(api.errMsg(err)); }
}

export const initFriends = () => {
  on('game', renderMe); on('packs', renderFriends);
  const form = $('#addForm');
  $('#friendAdd').addEventListener('click', () => {
    if ($('.window').classList.contains('colmini')) $('#colExpand').click(); // la colonne réduite n'a pas la place du champ
    form.hidden = !form.hidden; $('#friendAdd').setAttribute('aria-expanded', String(!form.hidden));
    if (!form.hidden) $('#addName').focus();
  });
  form.addEventListener('submit', async (e) => {
    e.preventDefault();
    const n = $('#addName').value.trim(); if (!n) return;
    try { await api.friendAdd(n); $('#addName').value = ''; form.hidden = true; $('#friendAdd').setAttribute('aria-expanded', 'false'); toast(n + ' ajouté'); await refreshNow(); } catch (err) { toast(api.errMsg(err)); }
  });
  $('#col').addEventListener('click', onAction);
};
