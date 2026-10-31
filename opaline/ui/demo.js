// MODE DÉMO : données fictives pour prévisualiser l'interface dans un navigateur (hors Tauri).
// Les textures Mojang ne sont pas copiées ici : servez design/launcher-mockup/assets/mc/ sur le port 8767.

const q = new URLSearchParams(location.search);
const wait = (ms) => new Promise((r) => setTimeout(r, ms));
const subs = {};
export const listen = (n, cb) => { (subs[n] ||= new Set()).add(cb); return () => subs[n].delete(cb); };
const emit = (n, p) => (subs[n] || []).forEach((f) => f(p));
export const texUrl = (name) => `${q.get('tex') || 'http://localhost:8767/'}${name}.png`;
export const controls = { help: '' };

const iso = (min) => new Date(Date.now() - min * 60000).toISOString();
const packs = [
  { id: 'classroom-run', name: 'Classroom Run', description: 'Hardcore coopératif : vie partagée, mort collective, chaque run recommence dans la salle de classe.', mc: '26.3', loader: 'fabric', loaderVersion: '0.20.1', modsCount: 10, thumbnail: 'ender_eye', widgets: ['classroomRun', 'server', 'news'], hasServer: true, hasStats: true, supported: true, installed: true, updateAvailable: true, installedVersionHash: 'a1b2' },
  { id: 'survie', name: 'Survie entre potes', description: 'Survie vanilla améliorée, monde partagé du groupe, sans règle particulière.', mc: '26.3', loader: 'fabric', loaderVersion: '0.20.1', modsCount: 64, thumbnail: 'grass_block_side', widgets: ['server', 'news'], hasServer: true, hasStats: false, supported: true, installed: true, updateAvailable: false, installedVersionHash: 'c3d4' },
  { id: 'solo', name: 'Vanilla amélioré', description: 'Sodium, Lithium et quelques mods de confort, pour jouer en solo ou sur n’importe quel serveur.', mc: '26.3', loader: 'fabric', loaderVersion: '0.20.1', modsCount: 12, thumbnail: 'oak_log', widgets: ['server'], hasServer: false, hasStats: false, supported: true, installed: true, updateAvailable: false, installedVersionHash: 'e5f6' },
  { id: 'create', name: 'Create & Co', description: 'Automatisation et usines avec Create.', mc: '26.1', loader: 'neoforge', loaderVersion: '6.0.4', modsCount: 112, thumbnail: 'piston_side', widgets: ['server'], hasServer: true, hasStats: false, supported: false, installed: false, updateAvailable: false, installedVersionHash: null },
];
const MODS = [['Fabric API', '0.131.0', 'both'], ['Classroom Run', '0.9.2', 'both'], ['Sodium', '0.7.2', 'client'], ['Lithium', '0.18.1', 'both'], ['Iris Shaders', '1.9.3', 'client', false], ['Sodium Extra', '0.7.0', 'client'], ['Mod Menu', '15.0.0', 'client'], ['Fantasy', '0.8.1', 'server'], ['ServerReplay', '1.4.0', 'server'], ['Replay Mod', '2.7.0', 'client']]
  .map(([name, version, side, ok]) => ({ name, version, side, file: name.toLowerCase().replace(/\W+/g, '-') + '.jar', verified: ok !== false }));

let settings = { perfMode: 'balanced', memoryMb: 6144, noRemember: false, systemRamMb: 16384 };
let auth = q.has('signedout') ? { state: 'signedOut', profile: null, warning: null } : { state: 'signedIn', profile: { name: 'OddNode', uuid: '3f9a21c4-7be0-4a11-9c3d-0b6e5f51e51d' }, warning: null };

const px = (w, h, fn) => { const c = document.createElement('canvas'); c.width = w; c.height = h; const g = c.getContext('2d'); fn((col, x, y, a, b) => { g.fillStyle = col; g.fillRect(x, y, a, b); }); return c.toDataURL('image/png'); };
const skinPng = (P) => px(64, 64, (R) => {
  R(P.s, 8, 8, 8, 8); R(P.h, 8, 8, 8, 3); R('#fff', 9, 12, 2, 1); R('#fff', 13, 12, 2, 1); R(P.e, 10, 12, 1, 1); R(P.e, 13, 12, 1, 1);
  R(P.t, 20, 20, 8, 12); R(P.s, 44, 20, 4, 12); R(P.s, 36, 52, 4, 12); R(P.p, 4, 20, 4, 12); R(P.p, 20, 52, 4, 12); R(P.b, 4, 30, 4, 2); R(P.b, 20, 62, 4, 2);
});
const capePng = (c) => px(64, 32, (R) => { R(c, 1, 1, 10, 16); R('rgba(0,0,0,.25)', 1, 15, 10, 2); });
let skins = [
  ['Skin actuel', 'classic', { h: '#4a2f1a', s: '#e0ac86', t: '#e0574f', p: '#3a4a8a', b: '#2c1219', e: '#2f6fd6' }],
  ['Explorateur', 'classic', { h: '#7a4a22', s: '#d9a07a', t: '#4f9a52', p: '#6b5133', b: '#2c1219', e: '#2a8f5a' }],
  ['Mage violet', 'slim', { h: '#c0c7d0', s: '#f2c9a5', t: '#7b4bb7', p: '#4a2f7a', b: '#1d1230', e: '#a66ad6' }],
  ['Cuisinier', 'classic', { h: '#2a2a33', s: '#b57d58', t: '#f4f0ea', p: '#3a3f4a', b: '#2c1219', e: '#2a2a33' }],
].map(([name, model, P], i) => ({ id: 's' + (i + 1), name, model, active: i === 0, dataUrl: skinPng(P) }));
let capes = [['Migrator', '#b14a5a'], ['Vanilla', '#e8d9b0'], ['Mojang Studios', '#d85252'], ['Mojang Office', '#ffffff']].map(([alias, c], i) => ({ id: 'c' + i, alias, active: i === 0, url: capePng(c) }));

const FRIENDS = [['LunaFox', 3, true, true, null], ['Creeper_Kid', 2, true, true, null], ['Maëlle', 1, true, false, null], ['PixelPaul', 4, false, false, 1200], ['Zéphyr', 5, false, false, 4300]]
  .map(([name, k, online, inGame, ago]) => ({ name, uuid: 'uuid-' + k, online, inGame, lastSeen: ago ? iso(ago) : null, k }));

const SPL = ['PICKAXE', 'IRON_INGOT', 'DIAMOND', 'NETHER_PORTAL', 'NETHER_WART', 'BLAZE_ROD', 'ENDER_EYE', 'ENTER_END', 'KILL_DRAGON'];
const DEATHS = [
  { name: 'Creeper_Kid', k: 2, cause: 'minecraft:explosion', attacker: 'minecraft:creeper', pos: [-312, 64, 148], hp: [20, 20, 20, 19, 19, 19, 12, 12, 4, 0] },
  { name: 'LunaFox', k: 3, cause: 'minecraft:lava', attacker: null, pos: [88, 11, -40], hp: [20, 20, 20, 20, 18, 18, 15, 10, 2, 0] },
  { name: 'Maëlle', k: 1, cause: 'minecraft:arrow', attacker: 'minecraft:skeleton', pos: [-17, 71, 203], hp: [20, 20, 16, 16, 12, 12, 10, 7, 3, 0] },
  { name: 'OddNode', k: 0, cause: 'minecraft:fall', attacker: null, pos: [240, 118, 9], hp: [20, 20, 20, 20, 20, 20, 20, 20, 6, 0] },
  { name: 'Creeper_Kid', k: 2, cause: 'minecraft:fireball', attacker: 'minecraft:blaze', pos: [31, 74, -88], hp: [20, 19, 17, 14, 13, 10, 8, 5, 3, 0], dim: 'minecraft:the_nether' },
];
const RUNS = [[12 * 3600 + 34 * 60 + 56, 19, 'ENDER_EYE'], [3 * 3600 + 2 * 60 + 11, 4, 'IRON_INGOT'], [2 * 3600 + 48 * 60 + 40, 3, 'NETHER_PORTAL'], [1 * 3600 + 55 * 60 + 7, 2, 'DIAMOND'], [4 * 3600 + 10 * 60 + 30, 6, 'BLAZE_ROD'], [59 * 60 + 14, 1, 'PICKAXE']]
  .map(([s, d, b], i) => ({ run: 20 - i, startedAt: iso(60 * 24 * (i + 1)), durationMs: s * 1000, deaths: d, bestSplit: b, finished: false }));
const detail = (n) => {
  const list = DEATHS.filter((_, i) => (i + (20 - n)) % 3 !== 2 || n === 20).slice(0, n === 20 ? 3 : 2);
  const mine = n === 20 ? DEATHS.slice(0, 3) : list;
  return { run: n, startedAt: iso(60 * 24), deaths: mine.map((d, i) => ({
    at: iso(30 - i * 9), player: 'uuid-' + d.k, name: d.name, cause: d.cause, message: d.name + ' died', attacker: d.attacker, fatalBy: d.name,
    pos: d.pos, dimension: d.dim || 'classroomrun:run_' + n + '_overworld',
    health: [{ name: d.name, samples: d.hp.map((h, j) => [-10000 + j * 1111, h]) },
      { name: 'Maëlle', samples: d.hp.map((h, j) => [-10000 + j * 1111, Math.min(20, h)]) }],
  })) };
};

let started = null, attemptN = 0, cancelled = false;
const stats = () => ({ runNumber: 20, totalTimeMs: 12 * 3600e3 + 34 * 60e3 + 56e3 + (started ? Date.now() - started : 0), running: true, deaths: 19, currentSplit: 6, currentSplitName: 'ENDER_EYE', reachedSplits: SPL.slice(0, 6), bestSplit: 'ENDER_EYE', players: ['LunaFox', 'Creeper_Kid', 'Maëlle'] });
started = Date.now();

const runInstall = async (id) => {
  const stages = ['java', 'minecraft', 'assets', 'loader', 'mods', 'textures'];
  for (const stage of stages) for (let d = 0; d <= 4; d++) { emit('install://progress', { packId: id, stage, done: d * 25, total: 100 }); await wait(220); }
  const p = packs.find((x) => x.id === id); p.installed = true; p.updateAvailable = false;
  emit('install://done', { packId: id, ok: true, error: null });
};

export async function invoke(cmd, a = {}) {
  await wait(60);
  switch (cmd) {
    case 'app_info': return { version: '0.1.0', update: q.has('update') ? { version: '0.1.1', url: 'https://github.com/Cikuree/opaline/releases/latest' } : null };
    case 'auth_status': return auth;
    case 'auth_start': { const attempt = ++attemptN; cancelled = false; setTimeout(() => { if (cancelled) return; auth = { state: 'signedIn', profile: { name: 'OddNode', uuid: '3f9a21c4-7be0-4a11-9c3d-0b6e5f51e51d' }, warning: null }; emit('auth://done', { attempt, ok: true, profile: auth.profile, error: null }); }, 6000); return a.mode === 'device' ? { attempt, mode: 'device', userCode: 'KQ7M4XNP', verificationUri: 'https://www.microsoft.com/link', expiresIn: 900 } : { attempt, mode: 'browser', authorizeUrl: 'https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize' }; }
    case 'auth_cancel': cancelled = true; return null;
    case 'auth_sign_out': auth = { state: 'signedOut', profile: null, warning: null }; return { warning: null };
    case 'packs_list': return structuredClone(packs);
    case 'pack_install': case 'pack_repair': runInstall(a.id); return null;
    case 'pack_changelog': return a.id === 'classroom-run' ? { added: ['Cloth Config 21.0.1'], removed: ['Indium 1.0.37'], updated: [{ name: 'Classroom Run', from: '0.9.2', to: '0.9.3' }, { name: 'Sodium', from: '0.7.1', to: '0.7.2' }] } : { added: [], removed: [], updated: [] };
    case 'pack_mods': return a.id === 'classroom-run' ? MODS : packs.find((p) => p.id === a.id)?.installed ? MODS.slice(0, 5).map((m) => ({ ...m, verified: true })) : [];
    case 'play': if (!packs.find((p) => p.id === a.id)?.installed) throw 'Le pack n’est pas installé.'; setTimeout(() => emit('game://exited', q.has('crash') ? { packId: a.id, code: 1, logTail: '[12:01:03] [main/ERROR]: Échec du chargement du mod « sodium ».\njava.lang.IllegalStateException: démo de plantage précoce' } : { packId: a.id, code: 0, logTail: null }), 5000); return { started: true, reason: null, serverListed: packs.find((p) => p.id === a.id)?.hasServer !== false };
    case 'server_status': {
      if (a.id === 'solo') return null; // pack sans serveur
      const ts = a.id !== 'survie'; // « survie » = serveur public hébergé : ping direct, sans Tailscale
      const help = ts ? controls.help || null : controls.help ? 'serverOffline' : null;
      if (help) return { online: false, latencyMs: null, players: { online: 0, max: 0, sample: [] }, version: null, motdOk: false, help, viaTailscale: ts };
      return a.id === 'classroom-run' ? { online: true, latencyMs: 23, players: { online: 3, max: 8, sample: [] }, version: '26.3', motdOk: true, help: null, viaTailscale: true } : { online: true, latencyMs: 31, players: { online: 2, max: 10, sample: [] }, version: '26.3', motdOk: true, help: null, viaTailscale: ts };
    }
    case 'run_stats': return a.id === 'classroom-run' ? stats() : null;
    case 'run_history': return a.id !== 'classroom-run' ? null : a.run == null ? { runs: RUNS } : detail(a.run);
    case 'settings_get': return settings;
    case 'settings_set': settings = { ...settings, ...a }; return settings;
    case 'open_folder': return null;
    case 'textures_dir': return { dir: q.has('notex') ? null : 'demo' };
    case 'skins_list': return structuredClone(skins);
    case 'skin_import': { const s = { id: 's' + Date.now(), name: a.name, model: a.model, active: false, dataUrl: 'data:image/png;base64,' + a.pngBase64 }; skins.push(s); return s; }
    case 'skin_apply': skins.forEach((s) => (s.active = s.id === a.id)); return null;
    case 'skin_rename': skins.find((s) => s.id === a.id).name = a.name; return null;
    case 'skin_delete': skins = skins.filter((s) => s.id !== a.id); return null;
    case 'capes_list': return structuredClone(capes);
    case 'cape_set': capes.forEach((c) => (c.active = c.id === a.id)); return null;
    case 'friends': return FRIENDS;
    case 'player_head': throw 'Tête indisponible en démo.';
    default: throw 'Commande inconnue : ' + cmd;
  }
}
