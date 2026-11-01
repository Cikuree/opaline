import * as api from '../api.js';
import { state } from '../state.js';
import { $, esc, tex, toast, setProgress } from '../util.js';
import { OPALINE_NOTES } from '../data.js';
import { changelogHtml } from './home.js';

const dismissed = new Set();

export async function renderNews() {
  const u = state.info.update, show = !!u && !dismissed.has(u.version);
  $('#banner').hidden = !show; $('#newsBadge').hidden = !u;
  $('#newsBtn').setAttribute('aria-label', u ? 'Actualités et mises à jour (1 nouvelle)' : 'Actualités et mises à jour');
  if (u) { $('#bannerT').textContent = `Nouvelle version du launcher : ${u.version} est disponible.`; $('#updLauncher').dataset.url = u.url; $('#updInstall').hidden = !u.auto; }
  const notes = OPALINE_NOTES.map((n) => `<div class="note"><h3>${n.v} ${n.fresh ? '<span class="pill">Nouveau</span>' : ''}</h3><ul>${n.items.map((i) => `<li>${esc(i)}</li>`).join('')}</ul></div>`).join('');
  const logo = '<svg class="logo-svg" width="32" height="32" viewBox="0 0 64 64" aria-hidden="true"><use href="#logo-o"/></svg>';
  const cards = [`<article class="card"><h2 class="h2i">${logo}Opaline ${esc(state.info.version)}</h2>${notes}</article>`];
  for (const p of state.packs.filter((x) => x.supported)) {
    let body = '<div class="note"><p class="small mute">Aucune nouveauté à installer.</p></div>';
    if (p.installed && p.updateAvailable) { try { body = `<div class="note"><h3>Mise à jour disponible <span class="pill">Nouveau</span></h3>${changelogHtml(await api.packChangelog(p.id))}</div>`; } catch { /* sans changelog */ } }
    cards.push(`<article class="card"><h2 class="h2i">${tex(p.thumbnail, 32)}${esc(p.name)}</h2><p class="small mute" style="margin-top:6px">${esc(p.description)}</p>${body}</article>`);
  }
  $('#newsCards').innerHTML = cards.join('');
}

export function initNews() {
  const mo = (n) => (n / 1048576).toFixed(1).replace('.', ',');
  api.listen('update://progress', ({ done, total }) => setProgress(true, 'Mise à jour d’Opaline', total ? `Téléchargement · ${mo(done)} / ${mo(total)} Mo` : `Téléchargement · ${mo(done)} Mo`, total ? (done / total) * 100 : 0));
  api.listen('update://done', ({ ok, error }) => { if (ok) return; setProgress(false); $('#updInstall').disabled = false; toast((error || 'La mise à jour a échoué.') + ' Utilisez le lien « Télécharger » en secours.'); });
  $('#updInstall').addEventListener('click', async (e) => {
    const b = e.currentTarget; b.disabled = true;
    try { await api.appUpdateInstall(); setProgress(true, 'Mise à jour d’Opaline', 'Téléchargement…', 0); } catch (err) { b.disabled = false; toast(api.errMsg(err)); }
  });
  $('#updLauncher').addEventListener('click', (e) => { api.openUrl(e.currentTarget.dataset.url); toast('Page de téléchargement ouverte'); });
  $('#dismiss').addEventListener('click', () => { if (state.info.update) dismissed.add(state.info.update.version); $('#banner').hidden = true; toast('Bannière masquée'); $('#newsBtn').focus(); });
}
