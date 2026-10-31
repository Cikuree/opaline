import * as api from '../api.js';
import { state, emit } from '../state.js';
import { $, $$, esc, toast, copy, headHtml, hydrateHeads } from '../util.js';

export async function refreshAuth() {
  try { state.auth = await api.authStatus(); } catch (e) { toast(api.errMsg(e)); }
  emit('auth');
}

// ---- rendu d'un skin / d'une cape depuis la texture (PNG 64×64 ou 64×32) ----
function skinCanvas(dataUrl, slim, scale) {
  const c = document.createElement('canvas'); c.width = 16; c.height = 32; c.style.width = 16 * scale + 'px'; c.style.height = 32 * scale + 'px'; c.setAttribute('role', 'img'); c.setAttribute('aria-label', `Aperçu du skin, bras ${slim ? 'fins' : 'classiques'}`);
  const img = new Image();
  img.onload = () => {
    const g = c.getContext('2d'), a = slim ? 3 : 4, old = img.height === 32; g.imageSmoothingEnabled = false;
    const D = (sx, sy, w, h, dx, dy, flip) => { if (flip) { g.save(); g.translate(dx + w, dy); g.scale(-1, 1); g.drawImage(img, sx, sy, w, h, 0, 0, w, h); g.restore(); } else g.drawImage(img, sx, sy, w, h, dx, dy, w, h); };
    D(8, 8, 8, 8, 4, 0); D(20, 20, 8, 12, 4, 8); D(44, 20, a, 12, 4 - a, 8); D(4, 20, 4, 12, 4, 20);
    if (old) { D(44, 20, a, 12, 12, 8, true); D(4, 20, 4, 12, 8, 20, true); } else { D(36, 52, a, 12, 12, 8); D(20, 52, 4, 12, 8, 20); }
    D(40, 8, 8, 8, 4, 0);
    if (!old) { D(20, 36, 8, 12, 4, 8); D(44, 36, a, 12, 4 - a, 8); D(52, 52, a, 12, 12, 8); D(4, 36, 4, 12, 4, 20); D(4, 52, 4, 12, 8, 20); }
  };
  img.src = dataUrl; return c;
}
function capeCanvas(url) {
  const c = document.createElement('canvas'); c.width = 10; c.height = 16; c.className = 'capeimg'; c.style.cssText = 'width:20px;height:32px;border-radius:4px;border:1px solid var(--line)'; c.setAttribute('aria-hidden', 'true');
  const img = new Image(); img.onload = () => { const g = c.getContext('2d'); g.imageSmoothingEnabled = false; g.drawImage(img, 1, 1, 10, 16, 0, 0, 10, 16); }; img.src = url; return c;
}

let skins = [], capes = [], confirmDel = null, pending = null, impModel = 'classic';

function renderSkins() {
  $('#skinCards').innerHTML = skins.map((k) => {
    const n = esc(k.name);
    return `<article class="card sk" data-id="${esc(k.id)}">${k.active ? '<span class="pill sk-b">Actif</span>' : ''}
     <div class="skp" data-skin="${esc(k.id)}"></div>
     <input class="skn" value="${n}" aria-label="Nom du skin ${n}" maxlength="24">
     <span class="tag">Bras ${k.model === 'slim' ? 'fins' : 'classiques'}</span>
     ${confirmDel === k.id ? `<p class="xs" role="alert">Supprimer « ${n} » ?</p><div class="acts2"><button class="btn sm" data-a="no">Annuler</button><button class="btn sm danger" data-a="yes">Confirmer</button></div>`
      : `<div class="acts2"><button class="btn sm primary" data-a="apply" ${k.active ? 'disabled' : ''}>Appliquer</button><button class="btn sm" data-a="ren">Renommer</button><button class="btn sm" data-a="del" ${k.active ? 'disabled' : ''} aria-label="Supprimer ${n}">Supprimer</button></div>`}</article>`;
  }).join('');
  skins.forEach((k) => $(`[data-skin="${CSS.escape(k.id)}"]`)?.append(skinCanvas(k.dataUrl, k.model === 'slim', 4)));
  const a = skins.find((k) => k.active);
  $('#fullSkin').replaceChildren(...(a ? [skinCanvas(a.dataUrl, a.model === 'slim', 7)] : []));
}
function renderCapes() {
  const none = !capes.some((c) => c.active);
  $('#capes').innerHTML = `<label class="opt"><input type="radio" name="cape" value="" ${none ? 'checked' : ''}><span style="width:20px;height:32px" aria-hidden="true"></span><span>Aucune</span></label>`
    + capes.map((c) => `<label class="opt"><input type="radio" name="cape" value="${esc(c.id)}" ${c.active ? 'checked' : ''}><span data-cape="${esc(c.id)}"></span><span>${esc(c.alias)}</span></label>`).join('');
  capes.forEach((c) => $(`[data-cape="${CSS.escape(c.id)}"]`)?.append(capeCanvas(c.url)));
}
async function loadWardrobe() {
  try { skins = await api.skinsList(); } catch (e) { skins = []; toast(api.errMsg(e)); }
  try { capes = await api.capesList(); } catch { capes = []; }
  renderSkins(); renderCapes();
}

export async function renderAccount() {
  const pr = state.auth.profile; if (!pr) return;
  $('#accName').textContent = pr.name; $('#accUuid').textContent = pr.uuid;
  $('#accAvatar').innerHTML = headHtml(pr.name, pr.uuid, 56); hydrateHeads($('#accAvatar'));
  $('#sessionList').innerHTML = `<li class="row"><span class="mute">Compte Microsoft</span><span class="ok">Connecté</span></li>
   <li class="row"><span class="mute">Connexion mémorisée</span><span>${state.settings?.noRemember ? 'Non (sur ce PC)' : 'Oui'}</span></li>
   ${state.auth.warning ? `<li class="row"><span class="mute">Avertissement</span><span style="color:var(--warn);font-weight:700">${esc(state.auth.warning)}</span></li>` : ''}`;
  await loadWardrobe();
}

function skinFile(f) {
  if (!f) return;
  if (f.type !== 'image/png') { $('#dropMsg').textContent = 'Format refusé : PNG uniquement.'; return toast('Fichier refusé : PNG requis'); }
  const u = URL.createObjectURL(f), im = new Image();
  im.onload = () => {
    const ok = im.width === 64 && (im.height === 64 || im.height === 32);
    $('#dropMsg').textContent = ok ? `${f.name} · ${im.width}×${im.height} valide` : `${f.name} : ${im.width}×${im.height}, 64×64 requis`;
    pending = ok ? f : null; toast(ok ? 'Skin prêt à importer' : 'Dimensions invalides'); URL.revokeObjectURL(u);
  };
  im.src = u;
}
const b64 = (f) => new Promise((ok, ko) => { const r = new FileReader(); r.onload = () => ok(String(r.result).split(',')[1]); r.onerror = ko; r.readAsDataURL(f); });

// ---- connexion ----
let expTimer = null, attempt = null;
const loginMsg = (t) => { const m = $('#loginMsg'); m.hidden = !t; m.textContent = t || ''; };
export function renderLogin() {
  const nc = state.auth.state === 'notConfigured';
  $('#btnLogin').disabled = nc;
  loginMsg(nc ? 'La connexion Microsoft n’est pas configurée dans cette version d’Opaline.' : '');
}
function stopLogin() { clearInterval(expTimer); $('#codeBox').hidden = true; $('#btnLogin').hidden = false; }
export function onAuthDone(p) {
  if (attempt === null || p.attempt !== attempt) return; // tentative périmée ou annulée
  attempt = null; stopLogin();
  if (!p.ok) { loginMsg(p.error || 'La connexion a échoué.'); return; }
  refreshAuth(); toast('Connecté en tant que ' + (p.profile?.name || ''));
}

export function initAccount() {
  $('#btnLogin').addEventListener('click', async () => {
    loginMsg('');
    try {
      const r = await api.authStart(); attempt = r.attempt; let left = r.expiresIn;
      $('#btnLogin').hidden = true; $('#codeBox').hidden = false;
      $('#codeVal').textContent = r.userCode; $('#codeVal').setAttribute('aria-label', 'Code ' + [...r.userCode].join(', '));
      $('#codeUri').textContent = r.verificationUri.replace(/^https?:\/\/(www\.)?/, ''); $('#codeBox').dataset.uri = r.verificationUri; $('#codeBox').dataset.code = r.userCode;
      const tick = () => { $('#codeWait').textContent = `En attente de validation… le code expire dans ${Math.max(1, Math.ceil(left / 60))} min.`; left -= 15; };
      clearInterval(expTimer); tick(); expTimer = setInterval(tick, 15000); $('#copyCode').focus();
    } catch (e) { loginMsg(api.errMsg(e)); }
  });
  $('#copyCode').addEventListener('click', () => copy($('#codeBox').dataset.code, 'Code copié'));
  $('#openUri').addEventListener('click', () => api.openUrl($('#codeBox').dataset.uri));
  $('#cancelLogin').addEventListener('click', async () => { attempt = null; try { await api.authCancel(); } catch { /* déjà terminé */ } stopLogin(); $('#btnLogin').focus(); });
  $('#btnLogout').addEventListener('click', async () => {
    try { const r = await api.authSignOut(); if (r?.warning) toast(r.warning); else toast('Déconnecté'); } catch (e) { toast(api.errMsg(e)); }
    await refreshAuth();
  });

  $('#capes').addEventListener('change', async (e) => {
    const id = e.target.value || null;
    try { await api.capeSet(id); toast(id ? 'Cape appliquée' : 'Cape retirée'); } catch (er) { toast(api.errMsg(er)); }
    loadWardrobe();
  });
  $('#skinCards').addEventListener('click', async (e) => {
    const card = e.target.closest('[data-id]'), ab = e.target.closest('[data-a]'); if (!card || !ab) return;
    const k = skins.find((x) => x.id === card.dataset.id), a = ab.dataset.a;
    try {
      if (a === 'apply') { await api.skinApply(k.id); toast('Skin appliqué'); await loadWardrobe(); }
      else if (a === 'ren') { const i = $('.skn', card); i.focus(); i.select(); }
      else if (a === 'del') { confirmDel = k.id; renderSkins(); $('#skinCards [data-a=no]').focus(); }
      else if (a === 'no') { confirmDel = null; renderSkins(); }
      else if (a === 'yes') { await api.skinDelete(k.id); confirmDel = null; toast('Skin supprimé'); await loadWardrobe(); }
    } catch (er) { toast(api.errMsg(er)); }
  });
  $('#skinCards').addEventListener('change', async (e) => {
    const card = e.target.closest('[data-id]'); if (!card || !e.target.classList.contains('skn')) return;
    const k = skins.find((x) => x.id === card.dataset.id), name = e.target.value.trim() || k.name;
    try { await api.skinRename(k.id, name); toast('Skin renommé'); } catch (er) { toast(api.errMsg(er)); }
    loadWardrobe();
  });
  $('#model').addEventListener('click', (e) => { const b = e.target.closest('[data-m]'); if (!b) return; impModel = b.dataset.m; $$('#model button').forEach((x) => x.setAttribute('aria-checked', x === b)); });
  $('#skinFile').addEventListener('change', (e) => skinFile(e.target.files[0]));
  const dz = $('#drop');
  ['dragenter', 'dragover'].forEach((n) => dz.addEventListener(n, (e) => { e.preventDefault(); dz.classList.add('over'); }));
  ['dragleave', 'drop'].forEach((n) => dz.addEventListener(n, (e) => { e.preventDefault(); dz.classList.remove('over'); }));
  dz.addEventListener('drop', (e) => skinFile(e.dataTransfer.files[0]));
  $('#importSkin').addEventListener('click', async () => {
    if (!pending) return toast('Choisis d’abord un PNG 64×64 valide');
    try {
      await api.skinImport(pending.name.replace(/\.png$/i, '').slice(0, 24) || 'Skin importé', impModel, await b64(pending));
      pending = null; $('#dropMsg').textContent = 'ou clique pour parcourir'; $('#skinFile').value = ''; toast('Skin importé dans la garde-robe'); loadWardrobe();
    } catch (er) { toast(api.errMsg(er)); }
  });
}
