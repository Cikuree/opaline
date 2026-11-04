import * as api from '../api.js';
import { state, current, hasWidget, statsMissing, emit, on, who } from '../state.js';
import { $, esc, toast, tex, hms, ago, headHtml, hydrateHeads } from '../util.js';
import { SPLITS, splitByKey, HELP, STAGE_ORDER, loaderName } from '../data.js';
import { openMenu } from './packmenu.js';
import { splitPacks, AVAIL_FOLD } from '../packs.js';
import { renderPlaytime } from './playtime.js';
import { causeInfo, loadRecent } from './history.js';

const GR = ['linear-gradient(135deg,#e0574f,#f07a5a)', 'linear-gradient(135deg,#7b4bb7,#a66ad6)', 'linear-gradient(135deg,#f6dcc8,#f3b9b4)'];
const STATUS = { upd: 'Mise à jour dispo', ok: 'Installé', no: 'Non installé', unsupported: 'Non pris en charge' };
export const statusOf = (p) => (!p.supported ? 'unsupported' : !p.installed ? 'no' : p.updateAvailable ? 'upd' : 'ok');

const listed = new Set(); // packs dont le serveur figure dans la liste Multijoueur (retour de `play`)
const servers = new Map(); // id -> ServerView | null (inconnu)
let recent = [];
let statsAt = 0;
let filter = '';
const changelogs = new Map();

/** Pourcentage global d'installation (6 étapes). */
export const installPct = (i) => { const k = Math.max(0, STAGE_ORDER.indexOf(i.stage)); return ((k + (i.total ? i.done / i.total : 0)) / STAGE_ORDER.length) * 100; };

export async function startInstall(id) {
  if (state.inGame) return toast('Ferme le jeu avant d’installer ou de mettre à jour.');
  state.install = { packId: id, stage: 'java', done: 0, total: 1 };
  emit('install');
  try { await api.packInstall(id); } catch (e) { state.install = null; emit('install'); toast(api.errMsg(e)); }
}

export async function refreshServers() {
  await Promise.all(state.packs.filter((p) => hasWidget(p, 'server')).map(async (p) => {
    try { servers.set(p.id, await api.serverStatus(p.id)); } catch { servers.set(p.id, null); }
  }));
  renderHome();
}
export async function refreshStats() {
  const p = current(); state.stats = null;
  if (hasWidget(p, 'classroomRun')) {
    try { state.stats = await api.runStats(p.id); statsAt = Date.now(); } catch { /* serveur injoignable */ }
    try { recent = await loadRecent(p.id); } catch { recent = []; }
  } else recent = [];
  renderHome();
}

function areas(cr, srv, news) {
  const n = ['"hero"', srv && '"server"', cr ? '"splits" "stat" "deaths"' : '"stat"', news && '"news"'].filter(Boolean).join(' ');
  const w = [srv ? '"hero server"' : '"hero hero"', cr ? '"splits splits" "stat deaths"' : '"stat stat"', news && '"news news"'].filter(Boolean).join(' ');
  return [n, w];
}

function blob() {
  const b = (r, a, n, ph, ox, oy) => { let d = ''; for (let k = 0; k <= 120; k++) { const t = k / 120 * Math.PI * 2, R = r + a * Math.sin(n * t + ph) + a * .55 * Math.sin((n + 1) * t + ph * 1.7) + a * .4 * Math.sin(2 * t + ph * .6); d += (k ? 'L' : 'M') + (150 + ox + R * Math.cos(t)).toFixed(1) + ' ' + (150 + oy + R * Math.sin(t)).toFixed(1); } return d + 'Z'; };
  const L = (cls, dl, d, fill, op) => `<g class="spin ${cls}"><g class="breath" style="animation-delay:${dl}s"><path d="${d}" fill="${fill}" opacity="${op}"/></g></g>`;
  $('#statSvg').innerHTML = `<defs>
   <linearGradient id="gv" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#7b4bb7"/><stop offset="1" stop-color="#a66ad6"/></linearGradient>
   <linearGradient id="go" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#e8774f"/><stop offset="1" stop-color="#f2a65a"/></linearGradient>
   <linearGradient id="gc" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#f6dcc8"/><stop offset="1" stop-color="#f3b9c8"/></linearGradient></defs>
   <g aria-hidden="true">${L('b', -3, b(98, 6.5, 5, 2.4, 2, 9), 'url(#gc)', .85)}${L('d', -2, b(104, 7, 5, 0, -7, 4), 'url(#gv)', .85)}${L('', -5, b(101, 7, 6, 1.3, 8, -4), 'url(#go)', .8)}${L('c', -7, b(107, 6, 7, .8, -3, -7), '#fff', .14)}
    <circle cx="150" cy="150" r="84" fill="#1d0b11"/>
    <g text-anchor="middle"><text x="150" y="138" font-size="12" font-weight="600" fill="#f3cfd2">Mon temps de jeu</text><text id="statVal" x="150" y="172" font-size="34" font-weight="800" fill="#fff" letter-spacing="-1">—</text></g></g>`;
}
// Heures et minutes (« 1 h 05 ») ; moins d'une heure : minutes seules
const fmtHours = (ms) => { const m = Math.floor(ms / 60e3); return m >= 60 ? Math.floor(m / 60).toLocaleString('fr-FR').replace(/\s/g, ' ') + ' h ' + String(m % 60).padStart(2, '0') : m + ' min'; };

let showAvail = false; // liste « À installer » dépliée

function renderPacks() {
  const { mine, avail } = splitPacks(state.packs, filter);
  $('#packs').innerHTML = mine.map((p) => {
    const i = state.packs.indexOf(p), st = statusOf(p), s = servers.get(p.id), up = s?.online;
    const srv = s ? `<span><span class="dot ${up ? '' : 'off'}"></span> ${up ? 'En ligne' : 'Hors ligne'}</span>` : '';
    return `<button class="pk" role="radio" aria-checked="${p.id === state.pack}" data-k="${esc(p.id)}"><span class="th" style="background:${GR[i % 3]}">${tex(p.thumbnail, 32)}</span><span class="tx2"><b>${esc(p.name)}</b><span class="meta">${esc(loaderName(p.loader))} ${esc(p.mc)} · ${Number(p.modsCount)} mods</span><span class="st"><span class="pill ${st === 'upd' ? 's-upd' : st === 'ok' ? '' : 's-no'}">${STATUS[st]}</span>${srv}</span></span></button>`;
  }).join('');
  $('#mineT').hidden = !mine.length;
  $('#availBox').hidden = !avail.length;
  const folded = avail.length > AVAIL_FOLD && !showAvail;
  $('#packsAvail').innerHTML = folded ? '' : avail.map((p) => {
    const i = state.packs.indexOf(p), off = !p.supported;
    // Carte = sélection du pack ; « Installer » = vrai bouton frère (pas imbriqué), seule action qui lance l'installation.
    return `<div class="pkw ${off ? 'xp-gray' : ''}"><button class="pk" role="radio" aria-checked="${p.id === state.pack}" data-k="${esc(p.id)}"><span class="th" style="background:${GR[i % 3]}">${tex(p.thumbnail, 32)}</span><span class="tx2"><b>${esc(p.name)}</b><span class="meta">${esc(loaderName(p.loader))} ${esc(p.mc)} · ${Number(p.modsCount)} mods</span>${off ? '<span class="st"><span class="pill s-no">Non pris en charge</span></span>' : ''}</span></button><button class="btn sm ${off ? '' : 'primary'} pki" data-i="${esc(p.id)}" aria-label="${off ? 'Indisponible : ' : 'Installer '}${esc(p.name)}" ${off || state.install ? 'disabled' : ''}>${off ? 'Indisponible' : 'Installer'}</button></div>`;
  }).join('');
  const more = $('#availMore');
  more.hidden = avail.length <= AVAIL_FOLD;
  more.textContent = showAvail ? 'Replier' : `Voir les ${avail.length} packs disponibles`;
}

export const fmtPlay = (s) => (s >= 3600 ? `${Math.floor(s / 3600)} h ${String(Math.floor(s / 60) % 60).padStart(2, '0')}` : `${Math.max(1, Math.round(s / 60))} min`);
/** Temps de jeu mesuré par Opaline ; mode officiel : non mesuré (le Launcher Minecraft possède le processus). */
function playtimeText(id) {
  if (state.auth.mode !== 'direct') return 'Temps de jeu non mesuré avec le Launcher Minecraft.';
  const t = state.playtime[id];
  return t ? `Joué ${fmtPlay(t.totalSecs)} · dernière session ${ago(new Date(t.lastPlayed * 1000).toISOString())}` : 'Aucune session mesurée pour l’instant.';
}
export async function refreshPlaytime() { try { state.playtime = await api.playtime(); } catch { /* inchangé */ } renderHome(); renderPlaytime(); }

function renderHero(p, st, cr) {
  const s = state.stats, inst = state.install?.packId === p.id ? state.install : null;
  $('#heroT').textContent = cr && s ? `Run #${s.runNumber}` : p.name;
  $('#runState').innerHTML = cr ? (s ? `<span class="dot ${s.running ? '' : 'idle'}"></span>${s.running ? 'En cours' : 'En pause'}` : '<span class="dot idle"></span>En attente du serveur') : `<span class="dot ${st === 'ok' || st === 'upd' ? '' : 'idle'}"></span>${STATUS[st]}`;
  const best = s?.bestSplit ? splitByKey(s.bestSplit) : null;
  $('#heroStats').innerHTML = cr
    ? `<div><b id="chrono">${s ? fmtHours(s.totalTimeMs) : '—'}</b><span>Chrono total</span></div><div><b>${s ? Number(s.deaths) : '—'}</b><span>Morts cumulées</span></div><div><b style="font-size:clamp(14px,4cqi,18px);line-height:1.4">${best ? esc(best.label) : '—'}</b><span>Meilleur split</span></div>`
    : `<div><b>${Number(p.modsCount)}</b><span>Mods</span></div><div><b>${esc(p.mc)}</b><span>Minecraft</span></div><div><b style="font-size:18px;line-height:1.4">${esc(loaderName(p.loader))}</b><span>Chargeur</span></div>`;
  const art = cr
    ? tex('ender_eye', 96, 'dragon sm') + tex('blaze_rod', 48, 'eye') + tex('diamond', 48, 'dia') + tex('ender_pearl', 32, 'p3') + tex('end_stone', 32, 'p4')
    : tex(p.thumbnail, 80, 'dragon sm') + tex('diamond', 48, 'eye') + tex('diamond_pickaxe', 48, 'dia');
  // Réécrire le même contenu relancerait l'animation de flottement (saut visible à chaque rafraîchissement).
  if ($('#heroArt').dataset.art !== art) { $('#heroArt').innerHTML = art; $('#heroArt').dataset.art = art; }
  $('#heroPlay').textContent = st === 'no' || st === 'unsupported' ? p.description || '' : playtimeText(p.id);
  const play = $('#btnPlay'), busy = !!state.install;
  $('.bl', play).textContent = st === 'no' ? 'INSTALLER' : st === 'unsupported' ? 'INDISPONIBLE' : state.inGame ? 'EN JEU' : 'JOUER';
  play.firstElementChild.firstElementChild.setAttribute('href', st === 'no' ? '#i-down' : '#i-play');
  play.disabled = st === 'unsupported' || state.inGame || busy;
  const ub = $('#btnUpd'), ui = ub.firstElementChild, ul = $('.bl', ub), ic = ui.firstElementChild;
  ub.hidden = st === 'no' || st === 'unsupported'; ui.classList.remove('spin-i');
  if (inst) { ub.disabled = true; ic.setAttribute('href', '#i-refresh'); ui.classList.add('spin-i'); ul.textContent = Math.round(installPct(inst)) + ' %'; }
  else if (st === 'upd') { ub.disabled = busy || state.inGame; ic.setAttribute('href', '#i-down'); ul.textContent = 'Mettre à jour'; }
  else { ub.disabled = true; ic.setAttribute('href', '#i-check'); ul.textContent = 'À jour'; }
}

function renderServer(p) {
  const s = servers.get(p.id), on = !!s?.online;
  $('#srvName').textContent = p.name;
  $('#srvPill').innerHTML = s ? (on ? '<span class="dot"></span>En ligne' : '<span class="dot off"></span>Hors ligne') : '<span class="dot idle"></span>Inconnu';
  $('#lat').textContent = on && s.latencyMs != null ? s.latencyMs + ' ms' : '—';
  $('#kvPl').textContent = on ? `${s.players.online}/${s.players.max}` : '—';
  $('#kvVer').textContent = (on && s.version) || p.mc;
  const help = s?.help ? HELP[s.help] : null, bad = !!help || (s && !on);
  const tsDown = s?.help === 'tailscaleNotInstalled' || s?.help === 'tailscaleDisconnected';
  const ts = s?.viaTailscale !== false; // serveur public : aucune mention de Tailscale
  $('#tsBox').hidden = !ts && !bad;
  $('#tsT').textContent = help ? help[0] : s && !on ? 'Serveur injoignable' : 'Tailscale connecté';
  $('#tsH').textContent = help ? help[1] : s && !on ? 'Le serveur ne répond pas : réessaie dans un instant.' : '';
  $('#tsH').hidden = !bad;
  $('#tsBox').classList.toggle('bad', bad);
  $('#tsBox > svg').style.color = bad ? 'var(--bad)' : 'var(--ok)';
  $('#tsBox > svg use').setAttribute('href', tsDown || !bad ? '#i-wifi' : '#i-x');
  $('#srvMsg').hidden = !s || on || !!help;
  $('#srvMsg').textContent = ts ? 'Serveur injoignable : vérifie que Tailscale est connecté, sinon demande l’accès à l’hôte.' : 'Serveur hors ligne ou injoignable.';
  if (s && motdBad(s)) { $('#srvMsg').hidden = false; $('#srvMsg').textContent = 'Le serveur répond mais ne correspond pas à ce pack.'; }
}
const motdBad = (s) => s.online && !s.motdOk && !s.help;

/** Temps de jeu personnel sur ce pack (mesuré par Opaline), indépendant du chrono de la run. */
function renderPlay(p) {
  const secs = state.playtime?.[p.id]?.totalSecs, v = secs ? fmtHours(secs * 1000) : '—';
  $('#statVal').textContent = v;
  $('#statVal').setAttribute('font-size', v.length > 8 ? 26 : 34); // « 1 234 h 05 » tient dans le disque
  $('#statSvg').setAttribute('aria-label', secs ? `Mon temps de jeu : ${v}` : 'Aucun temps de jeu mesuré');
}

function renderSplits(s) {
  const done = new Set(s?.reachedSplits || []), cur = s ? s.currentSplit : -1;
  $('#splCount').textContent = s ? `${done.size} sur ${SPLITS.length} atteints` : '';
  $('#splits').innerHTML = SPLITS.map((x, i) => { const c = done.has(x.k) ? 'done' : i === cur ? 'cur' : 'next'; return `<li class="split ${c}" ${c === 'cur' ? 'aria-current="step"' : ''}>${tex(x.tex, 48)}<b>${x.label}</b><span>${c === 'cur' ? 'en cours' : c === 'done' ? 'atteint' : '—'}</span></li>`; }).join('');
}

function renderDeaths() {
  const d = recent.slice(0, 4);
  $('#deaths').innerHTML = d.length ? d.map((x, i) => { const c = causeInfo(x); return `<li><button data-d="${i}" aria-label="Détail : ${esc(x.name)}, ${esc(c.how)}, run ${Number(x.run)}"><span class="avatar" style="width:36px;height:36px">${headHtml(x.name, x.player, 36)}</span><span class="t"><b>${esc(x.name)}</b><span class="mute">${esc(c.how)}</span></span>${tex(c.tex, 32)}<span class="run">Run #${Number(x.run)}</span></button></li>`; }).join('') : '<li class="mute small">Aucune mort enregistrée.</li>';
}

async function renderNews(p) {
  const box = $('#packNews'), key = p.id + ':' + p.installedVersionHash;
  if (!p.installed || !p.updateAvailable) { box.innerHTML = '<div class="note"><p class="small mute">Aucune nouveauté à installer : le pack est à jour.</p></div>'; return; }
  if (!changelogs.has(key)) changelogs.set(key, api.packChangelog(p.id).catch(() => null));
  const c = await changelogs.get(key);
  if (current()?.id !== p.id) return;
  box.innerHTML = c ? `<div class="note"><h3>Mise à jour disponible <span class="pill">Nouveau</span></h3>${changelogHtml(c)}</div>` : '<div class="note"><p class="small mute">Changelog indisponible.</p></div>';
}
export function changelogHtml(c) {
  const g = (t, k, sym, a) => a && a.length ? `<h3 style="margin-top:10px">${t} (${a.length})</h3><ul class="chg">${a.map((x) => `<li><span class="chs ${k}" aria-hidden="true">${sym}</span>${esc(x)}</li>`).join('')}</ul>` : '';
  const body = g('Ajoutés', 'add', '+', c.added) + g('Retirés', 'rem', '−', c.removed) + g('Mis à jour', 'upd', '↑', (c.updated || []).map((u) => `${u.name} ${u.from} → ${u.to}`));
  return body || '<p class="small mute">Aucune différence de mods.</p>';
}

export function renderHome() {
  const p = current(); if (!p) return;
  const st = statusOf(p), cr = hasWidget(p, 'classroomRun'), srv = hasWidget(p, 'server'), news = hasWidget(p, 'news');
  const name = who()?.name || '';
  $('#greet').textContent = new Date().getHours() >= 18 || new Date().getHours() < 5 ? 'Bonsoir,' : 'Bonjour,';
  $('#homeName').textContent = name;
  const s = state.stats, sv = servers.get(p.id);
  $('#homeSub').textContent = !p.supported ? `Non pris en charge par cette version d’Opaline (chargeur ${loaderName(p.loader)}).`
    : statsMissing(p) ? 'Statistiques indisponibles sur ce serveur.'
    : p.hasServer === false ? p.name
    : cr ? (s ? `Run #${s.runNumber} ${s.running ? 'en cours' : 'en pause'}.${sv?.online ? ' Le serveur t’attend.' : ''}` : 'Run en attente du serveur.')
    : p.name + ' · ' + (sv ? (sv.online ? `${sv.players.online} joueur${sv.players.online > 1 ? 's' : ''} connecté${sv.players.online > 1 ? 's' : ''}` : 'serveur injoignable') : 'serveur inconnu');
  renderPacks();
  const [an, aw] = areas(cr, srv, news);
  $('#home').style.setProperty('--areas-n', an); $('#home').style.setProperty('--areas-w', aw);
  $('#cardServer').hidden = !srv; ['cardSplits', 'cardDeaths'].forEach((i) => ($('#' + i).hidden = !cr)); $('#cardNews').hidden = !news;
  renderHero(p, st, cr);
  if (srv) renderServer(p);
  $('#srvList').hidden = !listed.has(p.id);
  renderPlay(p);
  if (cr) { renderSplits(s); renderDeaths(); hydrateHeads($('#deaths')); }
  if (news) renderNews(p);
  $('#btnPlay').dataset.st = st;
}

export function initHome() {
  blob();
  $('#home').addEventListener('click', (e) => { const b = e.target.closest('[data-d]'); if (b) emit('openDeath', recent[+b.dataset.d]); });
  $('#packs').addEventListener('click', (e) => { const b = e.target.closest('[data-k]'); if (b) emit('selectPack', b.dataset.k); });
  // Clic droit ou touche Menu / Maj+F10 sur un pack installé : menu au curseur (ou sous la carte au clavier) ; menu natif coupé sur ces cartes seulement.
  $('#packs').addEventListener('contextmenu', (e) => {
    const b = e.target.closest('.pk[data-k]'); if (!b) return;
    e.preventDefault(); const r = b.getBoundingClientRect(), kb = !e.clientX && !e.clientY;
    openMenu(b.dataset.k, kb ? r.left + 20 : e.clientX, kb ? r.bottom : e.clientY, b);
  });
  $('#packsAvail').addEventListener('click', (e) => {
    const i = e.target.closest('[data-i]'), k = e.target.closest('[data-k]');
    if (i) { if (!i.disabled) startInstall(i.dataset.i); } else if (k) emit('selectPack', k.dataset.k);
  });
  $('#availMore').addEventListener('click', () => { showAvail = !showAvail; renderPacks(); });
  $('#packSearch').addEventListener('input', (e) => { filter = e.target.value.trim().toLowerCase(); renderPacks(); });
  $('#btnUpd').addEventListener('click', () => { const p = current(); if (p) startInstall(p.id); });
  $('#btnPlay').addEventListener('click', () => { const p = current(); if (p) doPlay(p); });
  // Launcher Minecraft ouvert (mode officiel) : confirmation avant de le fermer ; jamais proposé si une partie tourne (le backend refuse).
  $('#lcNo').addEventListener('click', () => { $('#launcherDlg').hidden = true; $('#btnPlay').focus(); });
  $('#lcOk').addEventListener('click', async () => {
    const p = current(); $('#launcherDlg').hidden = true; if (!p) return;
    toast('Fermeture du Launcher Minecraft…');
    try { await api.officialCloseLauncher(); } catch (e) { return toast(api.errMsg(e)); }
    doPlay(p, true);
  });
  $('#launcherDlg').addEventListener('keydown', (e) => { if (e.key === 'Escape') $('#lcNo').click(); });
  // Mise à jour disponible : JOUER met d'abord le pack à jour (mod du serveur compris), puis lance le jeu
  let playAfter = null;
  on('installed', (id) => { const q = state.packs.find((x) => x.id === id); if (id === playAfter && q) { playAfter = null; doPlay(q); } });
  async function doPlay(p, retried = false) {
    if ($('#btnPlay').dataset.st === 'no') return startInstall(p.id);
    if ($('#btnPlay').dataset.st === 'upd') { playAfter = p.id; toast('Mise à jour du pack avant de jouer…'); return startInstall(p.id); }
    try {
      const r = await api.play(p.id);
      if (!r.started && r.reason === 'officialLauncherOpen' && !retried) { $('#launcherDlg').hidden = false; $('#lcNo').focus(); return; }
      if (!r.started) return toast(r.reason === 'officialLauncherOpen' ? 'Le Launcher Minecraft ne s’est pas fermé : ferme-le toi-même.' : r.reason || 'Le jeu n’a pas pu démarrer.');
      if (r.serverListed) listed.add(p.id); else listed.delete(p.id);
      if (r.via === 'official') return toast(r.message || 'Profil prêt dans le Launcher Minecraft : clique sur Jouer'); // pas de processus à suivre : jamais « en jeu »
      state.inGame = true; emit('game'); renderHome(); toast('Lancement de ' + p.name + (r.joining ? ' : connexion au serveur…' : '…'));
    } catch (e) { toast(api.errMsg(e)); }
  }
  $('#crashClose').addEventListener('click', () => ($('#crash').hidden = true));
  $('#demoTs')?.addEventListener('change', (e) => { api.demoControls.help = e.target.value; refreshServers(); });
  $('#demoTsL').hidden = !api.demoControls;
  on('install', renderHome); on('game', renderHome); on('packs', renderHome);
  setInterval(() => { // chrono extrapolé entre deux relevés
    const s = state.stats, el = $('#chrono'); if (s?.running && el) el.textContent = fmtHours(s.totalTimeMs + Date.now() - statsAt);
  }, 1000);
}
