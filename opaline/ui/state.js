// État partagé et mini bus d'événements entre les vues.
export const state = {
  auth: { state: 'signedOut', profile: null, warning: null },
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
export const current = () => state.packs.find((p) => p.id === state.pack) || null;
export const hasWidget = (p, w) => !!p && p.supported && (p.widgets || []).includes(w);

const bus = new EventTarget();
export const emit = (n, d) => bus.dispatchEvent(new CustomEvent(n, { detail: d }));
export const on = (n, f) => bus.addEventListener(n, (e) => f(e.detail));
