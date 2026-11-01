import * as api from './api.js';
import { state, current, hasWidget, emit, on, usable } from './state.js';
import { $, esc, toast, copy, setProgress, LS, refreshTextures } from './util.js';
import { STAGES, STAGE_ORDER } from './data.js';
import { initChrome, show, currentView } from './views/chrome.js';
import { initHome, renderHome, refreshServers, refreshStats, installPct, startInstall, refreshPlaytime } from './views/home.js';
import { initAccount, renderAccount, renderLogin, onAuthDone, refreshAuth } from './views/account.js';
import { initSettings, renderSettings } from './views/settings.js';
import { initFiles, renderFiles, syncBusy } from './views/files.js';
import { initHistory, renderHistory, openDeath } from './views/history.js';
import { initNews, renderNews } from './views/news.js';
import { initFriends, renderMe, refreshFriends } from './views/friends.js';

let crashPack = null, crashReport = '';
const signed = usable; // connecté, ou mode officiel choisi

async function loadPacks() {
  try { state.packs = await api.packsList(); } catch (e) { state.packs = []; toast(api.errMsg(e)); }
  const saved = LS.get('opaline.pack');
  if (!state.packs.some((p) => p.id === state.pack)) state.pack = (state.packs.find((p) => p.id === saved) || state.packs.find((p) => p.supported) || state.packs[0] || {}).id || null;
  emit('packs');
}
async function loadTextures() { try { state.texReady = await api.loadTextures(); } catch { state.texReady = false; } }
const reloadTextures = async () => { await loadTextures(); refreshTextures(); };

function selectPack(id) {
  state.pack = id; LS.set('opaline.pack', id); state.stats = null; emit('packs');
  refreshStats(); refreshServers();
  if (currentView() === 'history' && !hasWidget(current(), 'classroomRun')) show('home');
}

const VIEWS = { home: renderHome, account: renderAccount, settings: renderSettings, files: renderFiles, history: renderHistory, news: renderNews };

async function applyAuth() {
  renderLogin(); renderMe();
  if (!signed()) { show('login', false); return; }
  if (currentView() === 'login') show('home');
  refreshFriends(); await loadPacks(); refreshServers(); refreshStats(); refreshPlaytime();
}

function progressUi() {
  const i = state.install;
  if (!i) return setProgress(false);
  const p = state.packs.find((x) => x.id === i.packId);
  setProgress(true, `${p?.installed ? 'Mise à jour' : 'Installation'} de ${p?.name || 'pack'}`, `${STAGES[i.stage] || i.stage} · étape ${Math.max(0, STAGE_ORDER.indexOf(i.stage)) + 1}/${STAGE_ORDER.length}`, installPct(i));
}

function wireEvents() {
  api.listen('auth://done', onAuthDone);
  api.listen('textures://ready', reloadTextures);
  api.listen('install://progress', (p) => { state.install = { ...p }; progressUi(); emit('install'); });
  api.listen('install://done', async (p) => {
    state.install = null; progressUi(); emit('install');
    if (p.ok) { toast('Pack prêt'); await reloadTextures(); await loadPacks(); refreshServers(); refreshStats(); } else toast(p.error || 'L’installation a échoué.');
  });
  api.listen('game://exited', (p) => {
    state.inGame = false; emit('game'); refreshPlaytime();
    if (p.code !== 0 && (p.logTail || p.crash)) {
      crashPack = p.packId;
      $('#crashT').textContent = `Le jeu s’est arrêté${p.code != null ? ` (code ${p.code})` : ''}`;
      const f = p.crash?.findings || [];
      $('#crashList').innerHTML = f.length ? f.map((x) => `<li>${esc(x.text)}</li>`).join('') : '<li class="mute">Cause non identifiée : consulte la fin du journal ci-dessous.</li>';
      crashReport = p.crash?.report || p.logTail || '';
      $('#crashLog').textContent = p.logTail || ''; $('#crash').hidden = false; show('home');
      toast('Le jeu s’est arrêté de façon inattendue');
    }
  });
  $('#crashCopy').addEventListener('click', () => copy(crashReport, 'Rapport copié'));
  $('#crashLogs').addEventListener('click', () => api.openFolder(crashPack, 'logs').catch((e) => toast(api.errMsg(e))));
  $('#crashRepair').addEventListener('click', () => { $('#crash').hidden = true; startInstall(crashPack); });
  on('install', () => { progressUi(); syncBusy(); });
  on('game', syncBusy);
  on('selectPack', selectPack);
  on('auth', applyAuth);
  on('openDeath', async (d) => { await openDeath(d); show('history'); });
  on('view', (v) => VIEWS[v]?.());
  on('packs', () => {
    $('#rail [data-view=history]').hidden = !hasWidget(current(), 'classroomRun');
    if (currentView() === 'files') renderFiles();
    if (currentView() === 'news') renderNews();
  });
  document.addEventListener('click', (e) => { const b = e.target.closest('#filePacks [data-k]'); if (b) selectPack(b.dataset.k); });
  setInterval(() => { if (document.visibilityState === 'visible' && signed()) { refreshServers(); refreshStats(); refreshFriends(); } }, 15000);
}

async function boot() {
  initChrome(signed); initHome(); initAccount(); initSettings(); initFiles(); initHistory(); initNews(); initFriends();
  wireEvents();
  await Promise.all([
    api.appInfo().then((i) => { state.info = i; $('#appVer').textContent = 'Opaline ' + i.version; }).catch(() => {}),
    api.settingsGet().then((s) => (state.settings = s)).catch((e) => toast(api.errMsg(e))),
    api.authStatus().then((a) => (state.auth = a)).catch((e) => toast(api.errMsg(e))),
    loadTextures(),
  ]);
  renderNews(); // pastille de mise à jour
  await applyAuth();
}
boot();
