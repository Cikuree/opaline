// Couche d'accès unique : seule cette couche (et demo.js) connaît Tauri.
// Tauri 2 avec withGlobalTauri: true : window.__TAURI__.{core.invoke, core.convertFileSrc, event.listen, window.getCurrentWindow}.
import * as demo from './demo.js';

const T = window.__TAURI__;
export const isTauri = !!(T && T.core && T.core.invoke);

const call = (cmd, args) => (isTauri ? T.core.invoke(cmd, args) : demo.invoke(cmd, args));

/** S'abonne à un événement du backend ; renvoie une promesse de fonction de désabonnement. */
export const listen = (name, cb) =>
  isTauri ? T.event.listen(name, (e) => cb(e.payload)) : Promise.resolve(demo.listen(name, cb));

export const errMsg = (e) => (typeof e === 'string' ? e : (e && e.message) || 'Erreur inconnue.');

export const appInfo = () => call('app_info');
export const appUpdateInstall = () => call('app_update_install');
export const authStatus = () => call('auth_status');
export const authStart = (mode = 'browser') => call('auth_start', { mode });
export const authCancel = () => call('auth_cancel');
export const identitySet = (name) => call('identity_set', { name });
export const identitySkin = () => call('identity_skin');
export const authSignOut = () => call('auth_sign_out');
export const packsList = () => call('packs_list');
export const packInstall = (id) => call('pack_install', { id });
export const packChangelog = (id) => call('pack_changelog', { id });
export const packMods = (id) => call('pack_mods', { id });
export const packUninstall = (id) => call('pack_uninstall', { id });
export const packRepair = (id) => call('pack_repair', { id });
export const play = (id) => call('play', { id });
export const officialCloseLauncher = () => call('official_close_launcher');
export const serverStatus = (id) => call('server_status', { id });
export const playtime = () => call('playtime');
export const runStats = (id) => call('run_stats', { id });
export const runHistory = (id, run = null) => call('run_history', { id, run });
export const settingsGet = () => call('settings_get');
export const borderlessStatus = () => call('borderless_status');
export const settingsSet = (patch) => call('settings_set', { patch });
export const mapInfo = (id) => call('map_info', { id });
export const openMap = (id) => call('open_map', { id });
export const settingsCopy = (from, to, categories) => call('settings_copy', { from, to, categories });
export const shadersGet = (id) => call('shaders_get', { id });
/** Captures du pack, plus récentes d'abord ; `url` affichable (protocole asset de Tauri). */
export const screenshotsList = async (id) => (await call('screenshots_list', { id })).map((f) => ({ ...f, url: isTauri ? T.core.convertFileSrc(f.path) : f.path }));
export const logTail = (id) => call('log_tail', { id });
export const shaderSet = (id, name = null) => call('shader_set', { id, name });
export const shaderSearch = (id, query) => call('shader_search', { id, query });
export const shaderAdd = (id, projectId) => call('shader_add', { id, projectId });
export const openFolder = (id, kind) => call('open_folder', { id, kind });
export const backupsList = (id) => call('backups_list', { id });
export const backupRestore = (id, backupId) => call('backup_restore', { id, backupId });
export const extraModsList = (id) => call('extra_mods_list', { id });
export const extraModAdd = (id, path) => call('extra_mod_add', { id, path });
export const extraModImport = (id, name, dataBase64) => call('extra_mod_import', { id, name, dataBase64 });
export const extraModRemove = (id, file) => call('extra_mod_remove', { id, file });
export const extraPacksList = (id) => call('extra_packs_list', { id });
export const extraPackAdd = (id, path) => call('extra_pack_add', { id, path });
export const extraPackImport = (id, name, dataBase64) => call('extra_pack_import', { id, name, dataBase64 });
export const extraPackRemove = (id, kind, file) => call('extra_pack_remove', { id, kind, file });
export const modrinthSearch = (id, query) => call('modrinth_search', { id, query });
export const modrinthAdd = (id, projectId) => call('modrinth_add', { id, projectId });
export const skinsList = () => call('skins_list');
export const skinImport = (name, model, pngBase64) => call('skin_import', { name, model, pngBase64 });
export const skinApply = (id) => call('skin_apply', { id });
export const skinRename = (id, name) => call('skin_rename', { id, name });
export const skinDelete = (id) => call('skin_delete', { id });
export const capesList = () => call('capes_list');
export const capeSet = (id) => call('cape_set', { id });
export const friends = () => call('friends');
export const playerHead = (uuid) => call('player_head', { uuid });

let texDir = null;
export const loadTextures = async () => { texDir = (await call('textures_dir')).dir; return !!texDir; };
/** URL d'une texture `<nom>.png` du client.jar, ou null si indisponible. */
export const texUrl = (name) => {
  if (!texDir) return null;
  if (!isTauri) return demo.texUrl(name);
  return T.core.convertFileSrc(texDir.replace(/[\\/]+$/, '') + '/' + name + '.png');
};

/** Ouvre une URL dans le navigateur (aucune commande dédiée au contrat : plugin opener/shell si présent). */
export const openUrl = (url) => {
  const o = isTauri && (T.opener?.openUrl || T.shell?.open);
  if (o) return (T.opener?.openUrl ? T.opener.openUrl(url) : T.shell.open(url));
  window.open(url, '_blank', 'noopener');
};

// Fenêtre : API window de Tauri ; en démo, géré par chrome.js.
const win = () => T.window.getCurrentWindow();
export const windowApi = {
  minimize: () => (isTauri ? win().minimize() : undefined),
  toggleMaximize: () => (isTauri ? win().toggleMaximize() : undefined),
  close: () => (isTauri ? win().close() : undefined),
  isMaximized: () => (isTauri ? win().isMaximized() : Promise.resolve(false)),
  onResized: (cb) => (isTauri ? win().onResized(cb) : Promise.resolve(() => {})),
};

export const demoControls = isTauri ? null : demo.controls;
