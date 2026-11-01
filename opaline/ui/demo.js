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
  { id: 'classroom-run', name: 'Classroom Run', description: 'Hardcore coopératif : vie partagée, mort collective, chaque run recommence dans la salle de classe.', mc: '26.3', loader: 'fabric', loaderVersion: '0.20.1', modsCount: 10, thumbnail: 'ender_eye', widgets: ['classroomRun', 'server', 'news'], hasServer: true, hasStats: true, hasMap: true, supported: true, installed: true, updateAvailable: true, installedVersionHash: 'a1b2' },
  { id: 'survie', name: 'Survie entre potes', description: 'Survie vanilla améliorée, monde partagé du groupe, sans règle particulière.', mc: '26.3', loader: 'fabric', loaderVersion: '0.20.1', modsCount: 64, thumbnail: 'grass_block_side', widgets: ['server', 'news'], hasServer: true, hasStats: false, supported: true, installed: true, updateAvailable: false, installedVersionHash: 'c3d4' },
  { id: 'solo', name: 'Vanilla amélioré', description: 'Sodium, Lithium et quelques mods de confort, pour jouer en solo ou sur n’importe quel serveur.', mc: '26.3', loader: 'fabric', loaderVersion: '0.20.1', modsCount: 12, thumbnail: 'oak_log', widgets: ['server'], hasServer: false, hasStats: false, supported: true, installed: true, updateAvailable: false, installedVersionHash: 'e5f6' },
  { id: 'cobble', name: 'Cobblemon entre potes', description: 'Capture et combats de Pokémon avec Cobblemon.', mc: '26.3', loader: 'fabric', loaderVersion: '0.20.1', modsCount: 180, thumbnail: 'ender_pearl', widgets: ['server', 'news'], hasServer: true, hasStats: false, supported: true, installed: false, updateAvailable: false, installedVersionHash: null },
  { id: 'create', name: 'Create & Co', description: 'Automatisation et usines avec Create.', mc: '26.1', loader: 'neoforge', loaderVersion: '6.0.4', modsCount: 112, thumbnail: 'piston_side', widgets: ['server'], hasServer: true, hasStats: false, supported: false, installed: false, updateAvailable: false, installedVersionHash: null },
];
const MODS = [['Fabric API', '0.131.0', 'both'], ['Classroom Run', '0.9.2', 'both'], ['Sodium', '0.7.2', 'client'], ['Lithium', '0.18.1', 'both'], ['Iris Shaders', '1.9.3', 'client', false], ['Sodium Extra', '0.7.0', 'client'], ['Mod Menu', '15.0.0', 'client'], ['Fantasy', '0.8.1', 'server'], ['ServerReplay', '1.4.0', 'server'], ['Replay Mod', '2.7.0', 'client'], ['Cloth Config', '21.0.1', 'both']]
  .map(([name, version, side, ok]) => ({ name, version, side, file: name.toLowerCase().replace(/\W+/g, '-') + '.jar', verified: ok !== false }));

let settings = { perfMode: 'balanced', memoryMb: 6144, noRemember: false, launchVia: 'opaline', joinServer: true, notifyServer: true, discordPresence: true, trayOnGame: true, autoTuned: false, systemRamMb: 16384 };
// ?signedout : écran de connexion ; ?official : mode officiel avec pseudo ; ?notapproved : la connexion échoue comme avec une app Azure non approuvée.
const UUID = '3f9a21c4-7be0-4a11-9c3d-0b6e5f51e51d';
let identity = q.has('official') ? { name: 'OddNode', uuid: UUID } : null, pending = false;
let signedIn = !(q.has('signedout') || q.has('official'));
const authView = () => ({ state: signedIn ? 'signedIn' : 'signedOut', profile: signedIn ? { name: 'OddNode', uuid: UUID } : null, warning: null, mode: signedIn ? 'direct' : 'official', mojangPending: !signedIn && pending, identity: signedIn ? null : identity });

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

// Joueurs vus sur le serveur du pack.
let FRIENDS = [['LunaFox', 3, true, true, null, true], ['Creeper_Kid', 2, true, true, null, true], ['Maelle', 1, true, false, null, false], ['PixelPaul', 4, false, false, 1200, true], ['Zéphyr', 5, false, false, 4300, true]]
  .map(([name, k, online, inGame, ago, friend]) => ({ name, uuid: 'uuid-' + k, online, inGame, lastSeen: ago ? iso(ago) : null, k, friend }));

const SPL = ['PICKAXE', 'IRON_INGOT', 'DIAMOND', 'NETHER_PORTAL', 'NETHER_WART', 'BLAZE_ROD', 'ENDER_EYE', 'ENTER_END', 'KILL_DRAGON'];
const DEATHS = [
  { name: 'Creeper_Kid', k: 2, cause: 'minecraft:explosion', attacker: 'minecraft:creeper', pos: [-312, 64, 148], hp: [20, 20, 20, 19, 19, 19, 12, 12, 4, 0] },
  { name: 'LunaFox', k: 3, cause: 'minecraft:lava', attacker: null, pos: [88, 11, -40], hp: [20, 20, 20, 20, 18, 18, 15, 10, 2, 0] },
  { name: 'Maelle', k: 1, cause: 'minecraft:arrow', attacker: 'minecraft:skeleton', pos: [-17, 71, 203], hp: [20, 20, 16, 16, 12, 12, 10, 7, 3, 0] },
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
      { name: 'Maelle', samples: d.hp.map((h, j) => [-10000 + j * 1111, Math.min(20, h)]) }],
  })) };
};

let backups = [[60 * 20, 412e6], [60 * 24 * 3, 398e6], [60 * 24 * 9, 377e6]].map(([m, size], i) => ({ id: '2026110' + (3 - i) + '-1803' + i + '0', createdAt: iso(m), sizeBytes: size }));
let extras = [{ file: 'zoomify-2.14.jar', title: null, auto: false, requiredBy: [], sizeBytes: 812000, warning: null }];
const MR = [['YL57xq9U', 'Iris Shaders', 'Shaders modernes pour Minecraft, compatibles avec Sodium.', 18400000, ['AANobbMI']], ['AANobbMI', 'Sodium', 'Moteur de rendu moderne qui accélère le jeu.', 52100000, []], ['gvQqBUqZ', 'Lithium', 'Optimisations du serveur de jeu intégré.', 41000000, []]];
const MRT = Object.fromEntries(MR.map((m) => [m[0], m[1]]));
let started = null, attemptN = 0, cancelled = false;
const stats = () => ({ runNumber: 20, totalTimeMs: 12 * 3600e3 + 34 * 60e3 + 56e3 + (started ? Date.now() - started : 0), running: true, deaths: 19, currentSplit: 6, currentSplitName: 'ENDER_EYE', reachedSplits: SPL.slice(0, 6), bestSplit: 'ENDER_EYE', players: ['LunaFox', 'Creeper_Kid', 'Maelle'] });
started = Date.now();

const MANY = ['a', 'b', 'c', 'd'].map((k) => ({ id: 'extra-' + k, name: 'Pack ' + k.toUpperCase(), description: '', mc: '26.3', loader: 'fabric', loaderVersion: '0.20.1', modsCount: 20, thumbnail: 'oak_log', widgets: ['server'], hasServer: false, hasStats: false, supported: true, installed: false, updateAvailable: false, installedVersionHash: null }));
if (q.has('many')) packs.push(...MANY);
const runInstall = async (id) => {
  const stages = ['java', 'minecraft', 'assets', 'loader', 'mods', 'textures'];
  for (const stage of stages) for (let d = 0; d <= 4; d++) { emit('install://progress', { packId: id, stage, done: d * 25, total: 100 }); await wait(220); }
  const p = packs.find((x) => x.id === id); p.installed = true; p.updateAvailable = false;
  emit('install://done', { packId: id, ok: true, error: null });
};

export async function invoke(cmd, a = {}) {
  await wait(60);
  switch (cmd) {
    case 'app_info': return { version: '0.1.0', update: q.has('update') ? { version: '0.1.1', url: 'https://github.com/Cikuree/opaline/releases/latest', auto: !q.has('manual') } : null };
    case 'app_update_install': (async () => {
      if (q.has('updfail')) { await wait(800); emit('update://done', { ok: false, error: 'hash invalide pour l’installeur, fichier supprimé' }); return; }
      for (let d = 0; d <= 10; d++) { emit('update://progress', { done: d * 8e6, total: 80e6 }); await wait(250); }
      console.info('[démo] installeur lancé : /P /R /UPDATE, Opaline quitte');
    })(); return null;
    case 'auth_status': return authView();
    case 'auth_start': { const attempt = ++attemptN; cancelled = false; setTimeout(() => { if (cancelled) return; if (q.has('notapproved')) { pending = true; emit('auth://done', { attempt, ok: false, profile: null, error: 'Application Azure non encore approuvée par Mojang (403 sur login_with_xbox).', mojangPending: true }); return; } signedIn = true; emit('auth://done', { attempt, ok: true, profile: authView().profile, error: null, mojangPending: false }); }, q.has('notapproved') ? 1500 : 6000); return a.mode === 'device' ? { attempt, mode: 'device', userCode: 'KQ7M4XNP', verificationUri: 'https://www.microsoft.com/link', expiresIn: 900 } : { attempt, mode: 'browser', authorizeUrl: 'https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize' }; }
    case 'auth_cancel': cancelled = true; return null;
    case 'auth_sign_out': signedIn = false; return { warning: null };
    case 'identity_set': {
      const n = String(a.name || '').trim();
      if (!n) { identity = null; return null; }
      if (!/^[A-Za-z0-9_]{3,16}$/.test(n)) throw 'Pseudo invalide : 3 à 16 caractères, lettres, chiffres ou _';
      if (n.toLowerCase() === 'inconnu') throw 'Aucun joueur Minecraft nommé « ' + n + ' »';
      return (identity = { name: n, uuid: UUID });
    }
    case 'identity_skin': return identity ? { dataUrl: skins[0].dataUrl, model: 'classic' } : null;
    case 'packs_list': return structuredClone(packs);
    case 'pack_install': case 'pack_repair': runInstall(a.id); return null;
    case 'pack_uninstall': { const p = packs.find((x) => x.id === a.id); if (!p?.installed) throw 'ce pack n’est pas installé'; if (q.has('worldopen')) throw 'Un monde est ouvert : ferme Minecraft puis réessaie'; p.installed = false; p.updateAvailable = false; return null; }
    case 'pack_changelog': return a.id === 'classroom-run' ? { added: ['Cloth Config 21.0.1'], removed: ['Indium 1.0.37'], updated: [{ name: 'Classroom Run', from: '0.9.2', to: '0.9.3' }, { name: 'Sodium', from: '0.7.1', to: '0.7.2' }] } : { added: [], removed: [], updated: [] };
    case 'pack_mods': return a.id === 'classroom-run' ? MODS : packs.find((p) => p.id === a.id)?.installed ? MODS.slice(0, 5).map((m) => ({ ...m, verified: true })) : [];
    case 'play': if (!packs.find((p) => p.id === a.id)?.installed) throw 'Le pack n’est pas installé.'; if (!signedIn || settings.launchVia === 'official') return { started: true, reason: null, serverListed: true, via: 'official', message: 'Profil prêt dans le Launcher Minecraft : clique sur Jouer' }; setTimeout(() => emit('game://exited', q.has('crash') ? { packId: a.id, code: 1, logTail: "[12:01:03] [main/ERROR]: Incompatible mod set!\n - Mod 'Legendary Monuments' (legendarymonuments) 8.1 requires any version of lithostitched, which is missing!", crash: { crashFile: 'crash-2026-11-01_12.01.03-client.txt', findings: [{ kind: 'missingMod', text: 'Le mod « Legendary Monuments » requiert « lithostitched », absent.' }, { kind: 'memory', text: 'Mémoire insuffisante : augmente la mémoire allouée dans les réglages d’Opaline (ou ferme d’autres programmes).' }], report: 'Opaline : plantage du pack classroom-run (code 1)\n- Le mod « Legendary Monuments » requiert « lithostitched », absent.' } } : { packId: a.id, code: 0, logTail: null }), 5000); return { started: true, reason: null, serverListed: packs.find((p) => p.id === a.id)?.hasServer !== false, via: 'direct', joining: settings.joinServer && packs.find((p) => p.id === a.id)?.hasServer !== false };
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
    case 'settings_set': settings = { ...settings, ...a.patch, ...(a.patch.perfMode || a.patch.memoryMb ? { autoTuned: false } : {}) }; return settings;
    case 'playtime': return { 'classroom-run': { totalSecs: 41 * 3600 + 25 * 60, lastPlayed: Math.floor(Date.now() / 1000) - 2 * 3600 }, survie: { totalSecs: 1500, lastPlayed: Math.floor(Date.now() / 1000) - 86400 * 4 } };
    case 'open_folder': return null;
    case 'map_info': { const p = packs.find((x) => x.id === a.id); if (!p?.hasMap) throw 'ce serveur n’a pas de carte'; await wait(300); return { name: p.name, url: 'https://bluecolored.de/bluemap/', reachable: !q.has('mapdown'), embeddable: !q.has('mapblock') }; }
    case 'open_map': { const p = packs.find((x) => x.id === a.id); if (!p?.hasMap) throw 'ce serveur n’a pas de carte'; console.info('[démo] fenêtre « Carte – ' + p.name + ' » (WebviewWindow externe, sans capability)'); return null; }
    case 'extra_mods_list': return packs.find((p) => p.id === a.id)?.installed ? structuredClone(extras) : [];
    case 'extra_mod_import': case 'extra_mod_add': { const n = a.name || String(a.path).split(/[\\/]/).pop(); if (!/\.jar$/i.test(n)) throw 'seuls les fichiers .jar au nom simple sont acceptés'; if (extras.some((x) => x.file === n)) throw '« ' + n + ' » est déjà dans Mes mods'; extras.push({ file: n, title: null, auto: false, requiredBy: [], sizeBytes: 123456, warning: /sodium/i.test(n) ? 'Même identifiant (« sodium ») que le mod « Sodium » du pack : risque de conflit.' : null }); return n; }
    case 'extra_mod_remove': { const e = extras.find((x) => x.file === a.file); if (!e) throw 'ce mod n’est pas dans Mes mods'; extras = extras.filter((x) => x.file !== a.file); const gone = [a.file]; extras.filter((x) => x.auto && x.requiredBy.includes(e.title)).forEach((x) => { x.requiredBy = x.requiredBy.filter((r) => r !== e.title); if (!x.requiredBy.length) { extras = extras.filter((y) => y !== x); gone.push(x.file); } }); return gone; }
    case 'modrinth_search': return MR.filter((m) => m[1].toLowerCase().includes(String(a.query).toLowerCase())).map(([projectId, title, description, downloads]) => ({ projectId, title, description, downloads, iconUrl: null }));
    case 'modrinth_add': {
      const m = MR.find((x) => x[0] === a.projectId); if (!m) throw 'Projet introuvable';
      if (extras.some((x) => x.title === m[1])) throw '« ' + m[1] + ' » est déjà dans Mes mods';
      const deps = [], provided = [];
      for (const d of m[4]) { const t = MRT[d]; if (a.id === 'classroom-run') provided.push(t); else if (!extras.some((x) => x.title === t)) { extras.push({ file: t.toLowerCase() + '-1.0.jar', title: t, auto: true, requiredBy: [m[1]], sizeBytes: 900000, warning: null }); deps.push({ title: t, file: t.toLowerCase() + '-1.0.jar' }); } }
      extras.push({ file: m[1].toLowerCase().replace(/\W+/g, '-') + '-1.0.jar', title: m[1], auto: false, requiredBy: [], sizeBytes: 1500000, warning: null });
      return { added: { title: m[1], file: m[1].toLowerCase() + '.jar', version: '1.0' }, deps, provided };
    }
    case 'backups_list': return a.id === 'classroom-run' ? structuredClone(backups) : [];
    case 'backup_restore': if (!backups.some((b) => b.id === a.backupId)) throw 'Sauvegarde introuvable'; backups.unshift({ id: '20261104-120000', createdAt: iso(0), sizeBytes: 415e6 }); backups = backups.slice(0, 3); return null;
    case 'textures_dir': return { dir: q.has('notex') ? null : 'demo' };
    case 'skins_list': return structuredClone(skins);
    case 'skin_import': { const s = { id: 's' + Date.now(), name: a.name, model: a.model, active: false, dataUrl: 'data:image/png;base64,' + a.pngBase64 }; skins.push(s); return s; }
    case 'skin_apply': skins.forEach((s) => (s.active = s.id === a.id)); return null;
    case 'skin_rename': skins.find((s) => s.id === a.id).name = a.name; return null;
    case 'skin_delete': skins = skins.filter((s) => s.id !== a.id); return null;
    case 'capes_list': return structuredClone(capes);
    case 'cape_set': capes.forEach((c) => (c.active = c.id === a.id)); return null;
    case 'friends': return structuredClone(FRIENDS);
    case 'player_head': throw 'Tête indisponible en démo.';
    default: throw 'Commande inconnue : ' + cmd;
  }
}
