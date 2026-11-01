// Vue « Carte » : la carte du serveur (BlueMap...) dans un cadre isolé. Le cadre est d'une autre origine : aucune API Tauri ne lui est
// injectée (scripts du noyau limités au cadre principal) et les commandes refusent toute origine distante. Déchargé en quittant la vue.
import * as api from '../api.js';
import { current, on } from '../state.js';
import { $, toast } from '../util.js';

let shown = null; // id du pack dont la carte est chargée
let seq = 0;

function unload() { shown = null; seq++; const f = $('#mapFrame'); f.hidden = true; f.removeAttribute('src'); }

export async function renderMap() {
  const p = current(); if (!p?.hasMap || shown === p.id) return;
  const n = ++seq, f = $('#mapFrame'), msg = $('#mapMsg');
  $('#mapSub').textContent = `${p.name} · carte en direct du serveur`;
  f.hidden = true; f.removeAttribute('src'); msg.textContent = 'Chargement de la carte…';
  let info; try { info = await api.mapInfo(p.id); } catch (e) { if (n === seq) msg.textContent = api.errMsg(e); return; }
  if (n !== seq) return;
  if (!info.reachable) { msg.textContent = 'La carte ne répond pas : vérifie que le serveur est en ligne (et Tailscale connecté), ou ouvre-la en grand.'; return; }
  if (!info.embeddable) { msg.textContent = 'Ce site refuse d’être affiché dans Opaline : ouvre-la en grand.'; return; }
  shown = p.id; msg.textContent = 'La carte reste sur le serveur : Opaline ne lui transmet rien.'; f.src = info.url; f.hidden = false;
}

export function initMap() {
  $('#mapPop').addEventListener('click', () => { const p = current(); if (p) api.openMap(p.id).catch((e) => toast(api.errMsg(e))); });
  on('view', (v) => { if (v !== 'map') unload(); });
}
