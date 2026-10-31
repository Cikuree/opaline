import * as api from '../api.js';
import { state, current, emit, on, who, official } from '../state.js';
import { $, esc, ago, toast, headHtml, hydrateHeads } from '../util.js';
import { startInstall, playPack } from './home.js';

const pack = (id) => state.packs.find((p) => p.id === id);
const packName = (id) => pack(id)?.name || id;
/** Version du pack de l'ami différente de la mienne (les deux connues). */
const differs = (f) => { const p = pack(f.packId); return !!(f.inGame && p?.installedVersionHash && f.packVersion && p.installedVersionHash !== f.packVersion); };

const subOf = (f) => {
  if (f.online) return f.inGame ? 'En jeu' + (f.packId ? ' · ' + packName(f.packId) : '') : 'En ligne';
  const seen = f.lastSeen ? ' · Vu ' + ago(f.lastSeen) : '';
  return (f.friend && state.p2p.state !== 'ok' ? 'Présence inconnue' : 'Hors ligne') + seen;
};

const actions = (f) => {
  const a = [];
  if (f.online && f.inGame && pack(f.packId)?.supported) a.push(['join', 'Rejoindre', '']);
  if (f.p2p && f.friend && current()) a.push(['invite', 'Inviter', 'ghost']);
  a.push(f.friend ? ['rm', 'Retirer', 'ghost'] : ['add', 'Ajouter', '']);
  const warn = differs(f) ? '<span class="pw">Version du pack différente de la tienne</span>' : '';
  return `<div class="plact">${warn}${a.map(([k, l, c]) => `<button type="button" class="${c}" data-fa="${k}" data-u="${esc(f.uuid)}">${l}</button>`).join('')}</div>`;
};

const row = (f) => {
  const off = !f.online, st = off ? 'hors ligne' : 'en ligne', sub = subOf(f) + (differs(f) ? ' (version du pack différente)' : '');
  return `<div class="plrow"><button type="button" class="pl ${off ? 'off' : ''}" data-tip="${esc(f.name)} · ${esc(sub)}" aria-label="${esc(f.name)}, ${st}. ${esc(sub)}"><div class="avwrap"><div class="avatar">${headHtml(f.name, f.uuid, 44)}</div><span class="dot ${off ? 'idle' : ''}" role="img" aria-label="${off ? 'Hors ligne' : 'En ligne'}"></span></div>${!off && f.inGame ? '<span class="ingame">En jeu</span>' : ''}<div class="tx"><div class="n">${esc(f.name)}</div><div class="s">${esc(sub)}</div></div></button>${actions(f)}</div>`;
};

export function renderMe() {
  const p = who(), signed = !official() && p;
  $('#meName').textContent = p?.name || ''; $('#meAvatar').innerHTML = p ? headHtml(p.name, p.uuid, 44) : '';
  $('#meDot').hidden = !signed; $('#meGame').hidden = !state.inGame; $('#meSt').textContent = official() ? 'Launcher Minecraft' : state.inGame ? 'En jeu' : 'En ligne';
  hydrateHeads($('#meAvatar'));
}

const NOTES = {
  noIdentity: 'Saisis ton pseudo (Compte) pour voir tes amis sur Tailscale.',
  noTailscale: 'Présence inconnue : Tailscale n’est pas connecté.',
};

export function renderFriends() {
  const sug = state.friends.filter((f) => f.p2p && !f.friend), rest = state.friends.filter((f) => !(f.p2p && !f.friend));
  const on_ = rest.filter((f) => f.online), off = rest.filter((f) => !f.online);
  $('#online').innerHTML = on_.length ? on_.map(row).join('') : '<p class="xs mute tx" style="padding:6px 0;text-align:center">Aucun ami en ligne.</p>';
  $('#offline').innerHTML = off.map(row).join('');
  $('#suggest').innerHTML = sug.length ? '<div class="fh">Sur ton Tailscale</div>' + sug.map(row).join('') : '';
  $('#onCount').textContent = on_.length;
  const note = NOTES[state.p2p.state] || (state.p2p.listenFailed ? 'Écoute impossible : autorise Opaline dans le pare-feu Windows pour que tes amis te voient.' : '');
  $('#p2pNote').textContent = note; $('#p2pNote').hidden = !note;
  hydrateHeads($('#col'));
}

export async function refreshFriends() {
  try {
    const r = await api.friends();
    state.friends = r.friends || []; state.p2p = r.p2p || { state: 'ok', listenFailed: false };
  } catch { state.friends = []; }
  renderFriends();
}

/** Attend la fin d'installation d'un pack (événement `install://done`). */
const waitInstall = (id) => {
  let fin = null, off = () => {};
  const p = new Promise((res) => { api.listen('install://done', (e) => { if (e.packId === id) { fin = e; res(e); } }).then((u) => (off = u)); });
  return { p, fin: () => fin, off: () => off() };
};

let joining = false;
/** Rejoindre : sélectionne le pack, l'installe ou le met à jour si besoin, puis JOUER (le serveur est ajouté à la liste Multijoueur par `play`). */
export async function joinPack(id) {
  const p = pack(id);
  if (!p?.supported) return toast('Ce pack n’est pas disponible dans ton catalogue.');
  if (state.inGame || joining || state.install) return toast('Termine d’abord le jeu ou l’installation en cours.');
  joining = true;
  try {
    emit('selectPack', id);
    if (!p.installed || p.updateAvailable) {
      const w = waitInstall(id);
      await startInstall(id);
      if (!w.fin() && state.install?.packId !== id) return w.off(); // démarrage refusé (déjà signalé)
      const r = await w.p; w.off();
      if (!r.ok) return;
    }
    await playPack(pack(id) || p);
  } finally { joining = false; }
}

let invite = null, inviteTimer;
const hideInvite = () => { invite = null; $('#inviteToast').classList.remove('on'); };
function showInvite(i) {
  invite = i; $('#inviteTx').textContent = `${i.fromName} t’invite sur ${packName(i.packId)}`;
  $('#inviteToast').classList.add('on'); clearTimeout(inviteTimer); inviteTimer = setTimeout(hideInvite, 20000);
}

async function onAction(e) {
  const b = e.target.closest('[data-fa]'); if (!b) return;
  const f = state.friends.find((x) => x.uuid === b.dataset.u); if (!f) return;
  try {
    switch (b.dataset.fa) {
      case 'join': return await joinPack(f.packId);
      case 'invite': await api.friendInvite(f.uuid, state.pack); return toast('Invitation envoyée à ' + f.name);
      case 'add': await api.friendAdd(f.name); toast(f.name + ' ajouté'); break;
      case 'rm': await api.friendRemove(f.uuid); break;
    }
    await refreshFriends();
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
    try { await api.friendAdd(n); $('#addName').value = ''; form.hidden = true; $('#friendAdd').setAttribute('aria-expanded', 'false'); toast(n + ' ajouté'); await refreshFriends(); } catch (err) { toast(api.errMsg(err)); }
  });
  $('#col').addEventListener('click', onAction);
  $('#inviteJoin').addEventListener('click', () => { const i = invite; hideInvite(); if (i) joinPack(i.packId); });
  $('#inviteX').addEventListener('click', hideInvite);
  api.listen('friends://invite', showInvite);
};
