import { LS } from './util.js';

// État partagé et mini bus d'événements entre les vues.
export const state = {
  // mode : « direct » (session Opaline) ou « official » (Launcher Minecraft) ; identity : pseudo saisi en mode officiel.
  auth: { state: 'signedOut', profile: null, warning: null, mode: 'official', mojangPending: false, identity: null },
  skip: LS.get('opaline.official') === '1', // « Jouer via le Launcher Minecraft » choisi : l'écran de connexion est passé
  settings: null,
  info: { version: '', update: null },
  packs: [],
  pack: null, // id du pack sélectionné
  texReady: false,
  server: null,
  stats: null,
  inGame: false,
  install: null, // {packId, stage, done, total}
  friends: [],
};
export const official = () => state.auth.state !== 'signedIn';
/** Joueur affiché : compte connecté, sinon pseudo saisi (mode officiel), sinon rien. */
export const who = () => state.auth.profile || state.auth.identity || null;
/** L'application est utilisable : connecté, ou mode officiel choisi. */
export const usable = () => !official() || state.skip;
export const current = () => state.packs.find((p) => p.id === state.pack) || null;
// Sans serveur : ni carte serveur ni widgets de run ; sans endpoint de stats (statsPort 0) : pas de widgets de run.
export const hasWidget = (p, w) => !!p && p.supported && (p.widgets || []).includes(w)
  && (w !== 'server' || p.hasServer !== false) && (w !== 'classroomRun' || (p.hasServer !== false && p.hasStats !== false));
export const statsMissing = (p) => !!p && p.supported && p.hasServer !== false && p.hasStats === false && (p.widgets || []).includes('classroomRun');

const bus = new EventTarget();
export const emit = (n, d) => bus.dispatchEvent(new CustomEvent(n, { detail: d }));
export const on = (n, f) => bus.addEventListener(n, (e) => f(e.detail));
