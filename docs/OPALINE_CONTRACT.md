# Contrat Opaline

Contrat entre l'interface web (`opaline/ui/`), le backend Tauri (`opaline/src-tauri/`), le cœur Rust (`launcher/`, devenu bibliothèque) et la CI (`packs/`, `.github/`). **Seul l'orchestrateur modifie ce fichier.** Un agent qui a besoin d'un changement le demande dans son rapport.

Toutes les données échangées sont en JSON, avec des clés en `camelCase`.

## 1. Organisation du dépôt

```
launcher/              cœur Rust (crate bibliothèque `opaline_core`, sans interface graphique)
opaline/src-tauri/     app Tauri 2 (commandes ci-dessous), dépend de ../../launcher
opaline/ui/            interface web statique (HTML/CSS/JS sans bundler), portée depuis design/launcher-mockup/
packs/<id>/            un pack par dossier : pack.toml, index.toml, mods/, shaderpacks/… + opaline.toml
```

Le dossier `pack/` actuel devient `packs/classroom-run/`.

### `packs/<id>/opaline.toml`

```toml
name = "Classroom Run"
description = "Hardcore coopératif : vie partagée, mort collective."
thumbnail = "ender_eye"            # nom d'une texture item/ ou block/ du client.jar
widgets = ["classroomRun", "server"]   # parmi : classroomRun, server, news
[server]
host = "home-server"                 # nom MagicDNS court ; suffixe tailnet ajouté à l'exécution
port = 25565
motd = "Classroom Run"             # le MOTD du serveur doit contenir ce texte
statsPort = 25580                  # 0 si pas d'endpoint HTTP (packs non Classroom)
```

## 2. Release (tag `vX.Y.Z`) : assets

| Asset | Contenu |
|---|---|
| `packs.json` | catalogue (format §3) |
| `pack-<id>.zip` | le dossier `packs/<id>/` publié (avec, pour classroom-run, `mods/classroomrun.pw.toml` généré) ; à la racine du zip, un dossier `pack/` |
| `classroomrun-X.Y.Z.jar` | le mod |
| installeurs Opaline | Windows : `Opaline_X.Y.Z_x64-setup.exe` ; Linux : `Opaline_X.Y.Z_amd64.AppImage` |
| `launcher.json` | `{"version": "X.Y.Z", "windows": "<url installeur>", "linux": "<url AppImage>"}` |
| `classroom-run-prism.zip` | instance Prism (inchangé) |

Opaline lit `https://github.com/Cikuree/opaline/releases/latest/download/launcher.json` (mise à jour d'Opaline).

### Release permanente `packs` (catalogue et packs)

- Les packs sont publiés indépendamment des versions d'Opaline : à chaque push sur `main` qui touche `packs/**`, le workflow `packs.yml` reconstruit `packs.json` et chaque `pack-<id>.zip`, puis les remplace dans la release au tag fixe **`packs`** (pré-release, assets écrasés).
- Opaline lit le catalogue à l'adresse fixe `https://github.com/Cikuree/opaline/releases/download/packs/packs.json` ; dans `packs.json`, `pack` pointe vers `.../releases/download/packs/pack-<id>.zip`.
- Le pack `classroom-run` référence le jar du mod de la dernière release `v*` (URL + sha512), régénéré par `packs.yml`.
- Fichiers d'un pack qui ne sont pas sur Modrinth (configs, `options.txt`, mods ou packs de ressources introuvables) : committés dans `packs/<id>/` et indexés par hash comme fichiers non-métafichier.
- Une release `v*` ne publie plus `packs.json` ni les `pack-<id>.zip` (elle déclenche seulement `packs.yml` pour rafraîchir l'URL du jar).

## 3. `packs.json`

```json
{"packs": [{
  "id": "classroom-run", "name": "Classroom Run", "description": "…",
  "mc": "26.3", "loader": "fabric", "loaderVersion": "0.20.1",
  "modsCount": 4, "thumbnail": "ender_eye", "widgets": ["classroomRun", "server"],
  "pack": "https://github.com/Cikuree/opaline/releases/download/vX.Y.Z/pack-classroom-run.zip",
  "packHash": "<sha256 de index.toml, = [index] hash de pack.toml>",
  "server": {"host": "home-server", "port": 25565, "motd": "Classroom Run", "statsPort": 25580}
}]}
```

- `loader` vaut `fabric` pour l'instant. Les autres loaders (neoforge…) sont refusés proprement : le pack est affiché « non pris en charge ».
- L'hôte du serveur n'est jamais affiché dans l'interface.

## 4. Commandes Tauri (`invoke(nom, args)`)

| Commande | Arguments | Retour |
|---|---|---|
| `app_info` | – | `{version, update: null \| {version, url}}` |
| `auth_status` | – | `{state: "signedOut" \| "signedIn" \| "notConfigured", profile: null \| {name, uuid}, warning: null \| string}` |
| `auth_start` | `{mode: "browser" \| "device"}` (défaut `browser`) | `browser` : `{attempt, mode: "browser", authorizeUrl}`. Opaline ouvre lui-même la page Microsoft dans le navigateur par défaut (code d'autorisation + PKCE, redirection vers `http://localhost:<port>` sur 127.0.0.1). `device` (secours) : `{attempt, mode: "device", userCode, verificationUri, expiresIn}`. Dans les deux cas, événement `auth://done` portant le même `attempt` |
| `auth_cancel` | – | – |
| `auth_sign_out` | – | `{warning: null \| string}` |
| `packs_list` | – | `[PackView]` (§5) |
| `pack_install` | `{id}` | –, puis événements `install://progress` et `install://done` (installe ou met à jour) |
| `pack_changelog` | `{id}` | `{added: [string], removed: [string], updated: [{name, from, to}]}` (différence entre installé et publié) |
| `pack_mods` | `{id}` | `[{name, version, side, file, verified}]` |
| `pack_repair` | `{id}` | comme `pack_install` (revérifie tout) |
| `play` | `{id}` | `{started: bool, reason: null \| string}`, puis événement `game://exited` |
| `server_status` | `{id}` | `ServerView` (§5) |
| `run_stats` | `{id}` | JSON de `GET /stats` du serveur, ou `null` (seulement pour le widget `classroomRun`) |
| `run_history` | `{id, run: null \| number}` | JSON de `GET /history` (index) ou `GET /history/<n>` (détail), schémas de CONTRACT.md §Historique, ou `null` |
| `settings_get` | – | `{perfMode: "low" \| "balanced" \| "quality" \| "custom", memoryMb, noRemember, systemRamMb}` |
| `settings_set` | `{perfMode?, memoryMb?, noRemember?}` | settings à jour |
| `open_folder` | `{id, kind: "mods" \| "shaderpacks" \| "screenshots" \| "logs" \| "instance"}` | – |
| `textures_dir` | – | `{dir: string \| null}`. Au démarrage, si les textures manquent, Opaline télécharge le client.jar officiel (hash vérifié, conservé dans `shared/`) et les extrait, puis émet `textures://ready`. `null` tant que ce n'est pas fait. Les PNG sont nommés `<nom>.png` et servis par `convertFileSrc` |
| `skins_list` | – | `[{id, name, model: "classic" \| "slim", active, dataUrl}]` |
| `skin_import` | `{name, model, pngBase64}` | `SkinView` (PNG 64×64 ou 64×32 validé) |
| `skin_apply` | `{id}` | – (API officielle `POST /minecraft/profile/skins`) |
| `skin_rename` | `{id, name}` | – |
| `skin_delete` | `{id}` | – |
| `capes_list` | – | `[{id, alias, active, url}]` |
| `cape_set` | `{id: string \| null}` | – |
| `friends` | – | `[{name, uuid, online, inGame, lastSeen: null \| string}]` |
| `player_head` | `{uuid}` | `{dataUrl}` (visage 8×8 + calque du chapeau, PNG, mis en cache) |

**Erreurs** : une commande en échec renvoie une chaîne lisible en français, sans jeton, sans adresse de serveur, sans chemin personnel complet.

### `friends` : périmètre v1

- La liste réunit les joueurs vus en ligne sur les serveurs des packs (échantillon du ping et `/stats`), mémorisés localement avec la date de dernière visite.
- `inGame` vaut vrai si le joueur est connecté au serveur d'un pack.
- Il n'y a pas de service de présence : le statut personnel (En ligne / Absent / Invisible) de la maquette n'est pas réalisé en v1.

## 5. Vues

```
PackView = {id, name, description, mc, loader, loaderVersion, modsCount, thumbnail, widgets,
            supported: bool, installed: bool, updateAvailable: bool, installedVersionHash: string | null}
ServerView = {online: bool, latencyMs: number | null, players: {online, max, sample: [{name, uuid}]},
              version: string | null, motdOk: bool,
              help: null | "tailscaleNotInstalled" | "tailscaleDisconnected" | "noAccess" | "unexpectedServer"}
SkinView = {id, name, model, active, dataUrl}
```

## 6. Événements (`listen(nom)`)

| Événement | Charge utile |
|---|---|
| `auth://done` | `{attempt: number, ok: bool, profile: null \| {name, uuid}, error: null \| string}`, jamais émis pour une tentative annulée |
| `install://progress` | `{packId, stage, done, total}`, avec les étapes toujours émises dans cet ordre : `java`, `minecraft`, `assets`, `loader`, `mods`, `textures` |
| `install://done` | `{packId, ok: bool, error: null \| string}` |
| `textures://ready` | `{dir: string}` : textures disponibles (émis au démarrage dès qu'elles sont extraites, y compris avant toute installation de pack) |
| `game://exited` | `{packId, code: number \| null, logTail: null \| string}` (log filtré : ni jeton ni adresse) |

## 7. Fenêtre

- `decorations: false`, redimensionnable, taille minimale 800×600, ombre native.
- La bande du haut porte `data-tauri-drag-region`.
- Les boutons de fenêtre appellent l'API window de Tauri : `minimize`, `toggleMaximize` et `close`.
- Le nom de l'application est `Opaline`, et son dossier de données `%APPDATA%\Opaline` (Windows) ou `~/.local/share/Opaline` (Linux).
- L'ancien dossier `ClassroomRunLauncher` est ignoré, sans migration.
