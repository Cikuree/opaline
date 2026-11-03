import * as api from '../api.js';
import { current, hasWidget } from '../state.js';
import { $, $$, esc, tex, hms, headHtml, hydrateHeads, toast } from '../util.js';
import { CAUSES, splitByKey } from '../data.js';

const pretty = (id) => { const t = String(id).split(':').pop().replace(/_/g, ' '); return t.charAt(0).toUpperCase() + t.slice(1); };
export function causeInfo(d) {
  const [label, t] = CAUSES[String(d.cause || '').split(':').pop()] || [d.message || pretty(d.cause || 'generic'), 'totem_of_undying'];
  const att = d.attacker ? pretty(d.attacker) : null;
  return { how: att ? `${label} (${att})` : label, label, att, tex: t };
}
const dimLabel = (d) => (/nether/.test(d) ? 'Nether' : /(^|[:_])end/.test(d) ? 'End' : 'Overworld');

/** Les 4 dernières morts (détail des 2 dernières runs), de la plus récente à la plus ancienne. */
export async function loadRecent(id) {
  const idx = await api.runHistory(id, null);
  const runs = (idx?.runs || []).slice(0, 2);
  const det = await Promise.all(runs.map((r) => api.runHistory(id, r.run).catch(() => null)));
  return det.flatMap((d) => (d?.deaths || []).map((x) => ({ ...x, run: d.run })))
    .sort((a, b) => Date.parse(b.at) - Date.parse(a.at));
}

let runs = [], sel = null, deaths = [], di = 0;

function graph(d) {
  const x = (t) => 30 + (Math.max(-10000, t) + 10000) / 10000 * 350, y = (h) => 150 - h * 6.5;
  const series = d.health || [], victim = series.find((s) => s.name === d.name) || series[0];
  const line = (s, main) => `<polyline points="${s.samples.map(([t, h]) => `${x(Number(t)).toFixed(1)},${y(Number(h)).toFixed(1)}`).join(' ')}" fill="none" stroke="${main ? '#f07a5a' : '#fff1e0'}" stroke-opacity="${main ? 1 : .4}" stroke-width="${main ? 3 : 1.6}" stroke-linejoin="round" stroke-linecap="round"/>`;
  const first = victim?.samples[0]?.[1] ?? 20;
  return `<div class="graph"><div class="xs mute" style="margin-bottom:4px">Vie sur les 10 dernières secondes (points de vie)${series.length > 1 ? ' · en clair : les autres joueurs' : ''}</div>
   <svg viewBox="0 0 400 170" width="100%" role="img" aria-label="Graphe : la vie de ${esc(d.name)} passe de ${Number(first)} à 0 en 10 secondes">
    <g stroke="#6a3340" stroke-width="1"><line x1="30" y1="20" x2="390" y2="20"/><line x1="30" y1="85" x2="390" y2="85"/><line x1="30" y1="150" x2="390" y2="150"/></g>
    <g fill="#c9a0a6" font-size="10"><text x="4" y="24">20</text><text x="4" y="89">10</text><text x="10" y="154">0</text><text x="30" y="166">−10 s</text><text x="352" y="166">Mort</text></g>
    ${series.filter((s) => s !== victim).map((s) => line(s, false)).join('')}${victim ? line(victim, true) : ''}
    <circle cx="380" cy="150" r="6" fill="#fff1e0"/></svg></div>`;
}

function renderDetail() {
  const box = $('#dthDetail');
  if (!sel) { box.innerHTML = '<h2>Détail de la mort</h2><p class="mute small" style="margin-top:8px">Choisis une run dans la liste.</p>'; return; }
  if (!deaths.length) { box.innerHTML = `<h2>Run #${Number(sel)}</h2><p class="mute small" style="margin-top:8px">Aucune mort dans cette run.</p>`; return; }
  const d = deaths[di], c = causeInfo(d), pos = d.pos ? `X ${Number(d.pos[0])}, Y ${Number(d.pos[1])}, Z ${Number(d.pos[2])}` : '—', dim = d.dimension ? dimLabel(d.dimension) : '';
  box.innerHTML = `<h2>Détail de la mort · Run #${Number(sel)}</h2>
   <ul class="list" style="gap:8px;margin:12px 0;font-size:14px">
    <li class="row"><span class="mute">Joueur</span><b style="display:flex;align-items:center;gap:8px"><span class="avatar" style="width:28px;height:28px">${headHtml(d.name, d.player, 28)}</span>${esc(d.name)}</b></li>
    <li class="row"><span class="mute">Cause</span><b>${esc(c.label)}</b></li>
    <li class="row"><span class="mute">Attaquant</span><b style="display:flex;align-items:center;gap:8px">${tex(c.tex, 32)}${esc(c.att || '—')}</b></li>
    <li class="row"><span class="mute">Position</span><b>${dim && dim !== 'Overworld' ? dim + ' · ' : ''}${pos}</b></li></ul>${graph(d)}`;
  hydrateHeads(box);
}

// Une seule mort compte par run (l'équipe meurt ensemble) : par défaut la dernière, celle qui a fini la run.
async function select(n, idx = -1) {
  sel = n; di = idx; deaths = [];
  $$('#runs tr').forEach((r) => r.classList.toggle('sel', +r.dataset.r === n));
  try { deaths = (await api.runHistory(current().id, n))?.deaths || []; } catch (e) { toast(api.errMsg(e)); }
  if (di < 0 || di >= deaths.length) di = Math.max(0, deaths.length - 1);
  renderDetail();
}

export async function renderHistory() {
  const p = current(); if (!hasWidget(p, 'classroomRun')) return;
  try { runs = (await api.runHistory(p.id, null))?.runs || []; } catch (e) { runs = []; toast(api.errMsg(e)); }
  $('#histSum').textContent = `${runs.length} run${runs.length > 1 ? 's' : ''} · ${hms(runs.reduce((a, r) => a + r.durationMs, 0))} jouées`;
  $('#runsEmpty').hidden = runs.length > 0;
  // Morts par joueur sur toutes les runs de l'historique (« victims » : un nom par mort)
  const per = new Map(); runs.forEach((r) => (r.victims || []).forEach((n) => per.set(n, (per.get(n) || 0) + 1)));
  const top = [...per].sort((a, b) => b[1] - a[1]);
  $('#histStats').hidden = !top.length;
  $('#histStats').innerHTML = top.map(([n, c], i) => `<div class="hst${i === 0 ? ' hdeath' : ''}"><span class="hn">${esc(n)}</span><b>${c} <small>mort${c > 1 ? 's' : ''}</small></b></div>`).join('');
  $('#runs').innerHTML = runs.map((r) => { const b = r.bestSplit ? splitByKey(r.bestSplit) : null; return `<tr tabindex="0" data-r="${Number(r.run)}"><td>#${Number(r.run)}</td><td>${hms(r.durationMs)}</td><td>${Number(r.deaths)}</td><td><span style="display:flex;align-items:center;gap:8px">${b ? tex(b.tex, 16) + esc(b.label) : '—'}</span></td></tr>`; }).join('');
  if (!runs.length) { sel = null; renderDetail(); } else if (!runs.some((r) => r.run === sel)) await select(runs[0].run); else await select(sel, di);
}

/** Ouvre le détail d'une mort (depuis l'accueil) ; appelé après renderHistory. */
export async function openDeath(d) {
  await renderHistory();
  await select(d.run);
  const k = deaths.findIndex((x) => x.at === d.at && x.player === d.player);
  if (k >= 0 && k !== di) { di = k; renderDetail(); }
}

export function initHistory() {
  $('#runs').addEventListener('click', (e) => { const r = e.target.closest('tr'); if (r) select(+r.dataset.r); });
  $('#runs').addEventListener('keydown', (e) => { if (e.key === 'Enter' || e.key === ' ') { const r = e.target.closest('tr'); if (r) { e.preventDefault(); select(+r.dataset.r); } } });
}
