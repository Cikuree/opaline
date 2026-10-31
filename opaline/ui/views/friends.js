import * as api from '../api.js';
import { state, current, hasWidget, on, who, official } from '../state.js';
import { $, esc, ago, headHtml, hydrateHeads } from '../util.js';

const row = (f) => {
  const off = !f.online, st = off ? 'hors ligne' : 'en ligne', sub = off ? 'Hors ligne' + (f.lastSeen ? ' · Vu ' + ago(f.lastSeen) : '') : f.inGame ? 'En jeu' : 'En ligne';
  return `<button type="button" class="pl ${off ? 'off' : ''}" data-tip="${esc(f.name)} · ${st}" aria-label="${esc(f.name)}, ${st}. ${esc(sub)}"><div class="avwrap"><div class="avatar">${headHtml(f.name, f.uuid, 44)}</div><span class="dot ${off ? 'idle' : ''}" role="img" aria-label="${off ? 'Hors ligne' : 'En ligne'}"></span></div>${!off && f.inGame ? '<span class="ingame">En jeu</span>' : ''}<div class="tx"><div class="n">${esc(f.name)}</div><div class="s">${esc(sub)}</div></div></button>`;
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

export async function refreshFriends() {
  try { state.friends = await api.friends(); } catch { state.friends = []; }
  renderFriends();
}
export const initFriends = () => { on('game', renderMe); on('packs', renderFriends); };
