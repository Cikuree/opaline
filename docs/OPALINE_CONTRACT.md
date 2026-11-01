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
[server]                           # section FACULTATIVE : sans elle, pas de carte serveur ni d'amis pour ce pack
host = "home-server"                 # nom court (sans point) = nom MagicDNS Tailscale, suffixe du tailnet ajouté à l'exécution ; nom complet (avec un point, ex. monserveur.minestrator.com) ou IP = utilisé tel quel, sans Tailscale
port = 25565
motd = "Classroom Run"             # le MOTD du serveur doit contenir ce texte
statsPort = 25580                  # 0 si pas d'endpoint HTTP (packs non Classroom)
map = "http://home-server:8100"    # FACULTATIF : carte du serveur (BlueMap...), URL http(s) ; voir « Carte du serveur » (§4)
[overrides]                        # section FACULTATIVE : archive des fichiers non commités (resourcepacks/, datapacks/, shaderpacks/...), publiée à part
url = "https://github.com/Cikuree/opaline/releases/download/packs/overrides-<id>-<8hex>.zip"   # préfixe imposé + nom de fichier simple .zip
sha256 = "<64 hex minuscules>"
size = 123456789                   # octets, entier > 0 et <= 2 Go (2147483648)
```

## 2. Release (tag `vX.Y.Z`) : assets

| Asset | Contenu |
|---|---|
| `packs.json` | catalogue (format §3) |
| `pack-<id>.zip` | le dossier `packs/<id>/` publié (avec, pour classroom-run, `mods/classroomrun.pw.toml` généré) ; à la racine du zip, un dossier `pack/` |
| installeurs Opaline | Windows : `Opaline_X.Y.Z_x64-setup.exe` ; Linux : `Opaline_X.Y.Z_amd64.AppImage` |
| `launcher.json` | `{"version": "X.Y.Z", "windows": "<url installeur>", "linux": "<url AppImage>", "windowsSha256": "<hex>", "linuxSha256": "<hex>"}` : les sha256 sont ceux des fichiers de la release ; sans hash (anciennes releases), pas de mise à jour automatique |
| `classroom-run-prism.zip` | instance Prism (inchangé) |

Opaline lit `https://github.com/Cikuree/opaline/releases/latest/download/launcher.json` (mise à jour d'Opaline).

### Release permanente `packs` (catalogue et packs)

- Les packs sont publiés indépendamment des versions d'Opaline : à chaque push sur `main` qui touche `packs/**`, le workflow `packs.yml` reconstruit `packs.json` et chaque `pack-<id>.zip`, puis les remplace dans la release au tag fixe **`packs`** (pré-release, assets écrasés).
- Opaline lit le catalogue à l'adresse fixe `https://github.com/Cikuree/opaline/releases/download/packs/packs.json` ; dans `packs.json`, `pack` pointe vers `.../releases/download/packs/pack-<id>.zip`.
- Le mod est compilé par `packs.yml` (version de `gradle.properties`, jar reproductible) et publié sur la release `packs` (`classroomrun-X.Y.Z.jar`) ; le pack `classroom-run` le référence (URL + sha512). Un changement du mod ne demande donc pas de nouvelle version d'Opaline.
- Fichiers d'un pack qui ne sont pas sur Modrinth (configs, `options.txt`, mods ou packs de ressources introuvables) : committés dans `packs/<id>/` et indexés par hash comme fichiers non-métafichier.
- Une release `v*` ne publie plus `packs.json` ni les `pack-<id>.zip` (elle déclenche seulement `packs.yml`).

## 3. `packs.json`

```json
{"packs": [{
  "id": "classroom-run", "name": "Classroom Run", "description": "…",
  "mc": "26.3", "loader": "fabric", "loaderVersion": "0.20.1",
  "modsCount": 4, "thumbnail": "ender_eye", "widgets": ["classroomRun", "server"],
  "pack": "https://github.com/Cikuree/opaline/releases/download/vX.Y.Z/pack-classroom-run.zip",
  "packHash": "<sha256 de index.toml, = [index] hash de pack.toml>",
  "server": {"host": "home-server", "port": 25565, "motd": "Classroom Run", "statsPort": 25580, "map": "http://home-server:8100"},
  "overrides": {"url": "https://github.com/Cikuree/opaline/releases/download/packs/overrides-classroom-run-0123abcd.zip", "sha256": "<64 hex>", "size": 123456789}
}]}
```

- `overrides` : absent sans section `[overrides]` (le `server` suit la même règle côté Opaline : absent ou `null`). `catalog` le valide (URL, sha256, taille). Opaline ignore une entrée dont `overrides` est invalide.
- `packHash` : sans `overrides`, hash d'index (`[index] hash` de `pack.toml`), inchangé (aucune mise à jour fantôme) ; avec `overrides`, `sha256_hex(utf8(<hash d'index en minuscules> + "
" + <sha256 de l'archive en minuscules>))` (`"
"` = un saut de ligne LF, pas de saut final). Une archive modifiée change donc `packHash` et déclenche la mise à jour. `pack-<id>.zip` ne contient jamais l'archive.
- Installation des overrides (étape `overrides` de `install://progress`, entre `mods` et `textures`, émise seulement pour un pack qui en a) : téléchargement de l'archive (sha256 vérifié, au plus `size` octets, reprise), puis extraction dans l'instance du pack. Refusés (rien n'est écrit) : chemin absolu, `..`, lecteur Windows, lien symbolique, plus de 60 000 entrées, plus de 1 Go décompressé. Jamais extraits : `options.txt` (c'est le fichier géré par les modes de performance : l'archive ne l'écrase pas), `installed.ok`, `opaline-pack.json` et `mods/` (les mods viennent du manifeste). La liste des fichiers posés est mémorisée dans `installed.ok` ; à la mise à jour, seuls les fichiers de cette liste absents de la nouvelle archive sont supprimés (saves/, screenshots/ et tout autre fichier ne sont pas touchés). Même sha256 déjà installé : ni téléchargement ni extraction. L'archive téléchargée est supprimée après extraction.

- `server.map` (facultatif) : validé par `pack-tools.py catalog` (refus du catalogue entier avec un message clair) et par Opaline (carte invalide : seule la carte est ignorée, le pack reste utilisable). Règle commune aux deux : `http://` ou `https://`, hôte présent, aucun identifiant (`user:mot@`), ASCII visible sans antislash ni espace, 300 caractères au plus, port différent de 0. Un nom d'hôte court (sans point, hors `localhost`) est un nom MagicDNS Tailscale : Opaline lui ajoute le suffixe du tailnet connu, comme pour le serveur de jeu.
- `loader` vaut `fabric`, `quilt`, `neoforge` ou `forge` (tous pris en charge ; `loaderVersion` = version du loader). Installation : voir docs/api/loaders.md (Fabric/Quilt via leur meta ; NeoForge/Forge en exécutant les processors client de l'installeur officiel dans l'instance, sorties vérifiées par sha1 quand l'installeur les fournit). Un loader inconnu donne `supported: false`.
- L'hôte du serveur n'est jamais affiché dans l'interface.

## 4. Commandes Tauri (`invoke(nom, args)`)

| Commande | Arguments | Retour |
|---|---|---|
| `app_info` | – | `{version, update: null \| {version, url, auto}}` ; `auto` vrai : « Mettre à jour » est proposé (hash publié ; Windows, ou Linux lancé depuis une AppImage) |
| `app_update_install` | – | –, puis `update://progress` ; sur succès Opaline quitte (installeur NSIS `/P /R /UPDATE` sous Windows, remplacement atomique de l'AppImage puis relance sous Linux) ; sur échec `update://done`. Refusé (erreur) pendant un jeu ou une installation. L'URL doit commencer par `https://github.com/Cikuree/opaline/releases/download/` et le sha256 est vérifié |
| `auth_status` | – | `{state: "signedOut" \| "signedIn" \| "notConfigured", profile: null \| {name, uuid}, warning: null \| string, mode: "direct" \| "official", mojangPending: bool, identity: null \| {name, uuid}}`. `mode` vaut `direct` avec une session Opaline valide et `launchVia` = `opaline`, sinon `official` (lancement via le Launcher Minecraft : voir `play`). `mojangPending` : le dernier essai de connexion a été refusé par Mojang (app Azure non approuvée, `NOT_APPROVED`), mémorisé en mémoire seulement. `identity` : pseudo Minecraft mémorisé (mode officiel seulement) |
| `identity_set` | `{name}` | `{name, uuid}`, ou `null` si `name` est vide (oublie le pseudo). Mode officiel : le pseudo (`[A-Za-z0-9_]{3,16}`, validé avant tout appel) est résolu via `api.mojang.com/users/profiles/minecraft/<pseudo>` (204 / 404 : erreur « Aucun joueur Minecraft nommé … »), puis mémorisé dans la config ; aucune authentification |
| `identity_skin` | – | `null` (pas de pseudo) ou `{dataUrl, model: "classic" \| "slim"}` : skin du pseudo mémorisé (sessionserver public, hôte de texture vérifié), affiché en lecture seule |
| `auth_start` | `{mode: "browser" \| "device"}` (défaut `browser`) | `browser` : `{attempt, mode: "browser", authorizeUrl}`. Opaline ouvre lui-même la page Microsoft dans le navigateur par défaut (code d'autorisation + PKCE, redirection vers `http://localhost:<port>` sur 127.0.0.1). `device` (secours) : `{attempt, mode: "device", userCode, verificationUri, expiresIn}`. Dans les deux cas, événement `auth://done` portant le même `attempt` |
| `auth_cancel` | – | – |
| `auth_sign_out` | – | `{warning: null \| string}` |
| `packs_list` | – | `[PackView]` (§5) |
| `pack_install` | `{id}` | –, puis événements `install://progress` et `install://done` (installe ou met à jour) |
| `pack_changelog` | `{id}` | `{added: [string], removed: [string], updated: [{name, from, to}]}` (différence entre installé et publié) |
| `pack_mods` | `{id}` | `[{name, version, side, file, verified}]` |
| `pack_repair` | `{id}` | comme `pack_install` (revérifie tout) |
| `play` | `{id}` | `{started: bool, reason: null \| string, serverListed: bool, via: "direct" \| "official", message?: string, joining?: bool}` (`via` et `serverListed` toujours présents, y compris si `started` est faux). `joining` (mode direct, `started` vrai) : le jeu a reçu `--quickPlayMultiplayer` et rejoint tout seul le serveur du pack (réglage `joinServer`, Minecraft 1.20+, pack avec serveur, adresse connue : un nom court n'est utilisé que si le suffixe MagicDNS est déjà connu) ; sinon le jeu s'arrête au menu principal. `via: "direct"` (session Opaline valide) : puis événement `game://exited`. `via: "official"` (pas de session) : Opaline écrit la version du loader sous l'identifiant propre `opaline-<packId>` (`.minecraft/versions/opaline-<packId>/`, champ `id` réécrit, jar renommé, marqueur `opaline.managed` ; un dossier de ce nom sans marqueur n'est jamais touché), copie vers `.minecraft/libraries` les libraries du loader présentes et, pour Forge / NeoForge, les fichiers que le Launcher ne peut pas télécharger (sorties des processors, libraries sans URL : liste `opaline-loader-files.json` écrite à l'installation ; absente, il faut « Vérifier et réparer »), fusionne le profil `opaline-<packId>` (`Opaline · <nom>`, `lastUsed` récent, `gameDir` = instance du pack, `javaArgs` = arguments par défaut du Launcher officiel avec seulement `-Xmx<RAM>M` remplacé) dans `launcher_profiles.json` (sauvegarde `.opaline.bak`, écriture atomique, reste du fichier intact) et ouvre le Launcher Minecraft ; `message` : « Profil prêt dans le Launcher Minecraft : clique sur Jouer », ou « Ouvre le Launcher Minecraft et choisis le profil Opaline · <pack> » s'il est introuvable. Si le Launcher officiel tourne déjà (il réécrirait le fichier) : `started` vaut faux et `reason` vaut **`"officialLauncherOpen"`** (code, pas un texte) ; l'interface demande alors « Le Launcher Minecraft est ouvert. Opaline doit le fermer pour préparer le profil du pack. » [Fermer et jouer] [Annuler], et sur accord appelle `official_close_launcher` puis relance `play`. Si une partie lancée par le Launcher officiel tourne (processus `java` / `javaw` d'un `.minecraft` ou d'une instance d'Opaline, ou `session.lock` verrouillé), `started` vaut faux avec « Une partie est en cours dans le Launcher Minecraft : quitte le jeu d'abord » et rien n'est fermé. Pas de `game://exited` (processus non possédé) ; l'id du pack n'est réservé que pendant la préparation. Erreur lisible si `launcher_profiles.json` manque (« Installe et ouvre une fois le Launcher Minecraft ») ou est illisible. Mode direct : lance le jeu dans l'instance du pack, avec ses mods (Mes mods recopiés dans `mods/`), jusqu'au menu principal ou, si `joining`, directement sur le serveur ; aucun ping ni appel à Tailscale. Pendant la partie : temps de jeu mesuré (`playtime`), statut Discord et réduction dans la zone de notification (voir « Bureau pendant une partie »). Si le pack déclare un serveur, Opaline l'ajoute au préalable à la liste Multijoueur de l'instance (`servers.dat`, entrée nommée d'après le pack) : ajoutée si aucune entrée n'a déjà cette adresse ; si une entrée porte déjà le nom du pack avec une autre adresse, seule son adresse est mise à jour ; les autres entrées ne sont jamais modifiées. Un nom court n'est écrit que si le suffixe MagicDNS est déjà connu (dernier `server_status`), sinon l'entrée est écrite au lancement suivant. `serverListed` vaut vrai si l'entrée figure dans la liste à l'issue (faux : pas de serveur, suffixe inconnu, nom/adresse trop long, `servers.dat` illisible ou écriture impossible) ; l'interface n'affiche « Ajouté à ta liste Multijoueur » que si vrai |
| `server_status` | `{id}` | `ServerView` (§5), ou `null` si le pack n'a pas de serveur |
| `run_stats` | `{id}` | JSON de `GET /stats` du serveur, ou `null` (seulement pour le widget `classroomRun`) |
| `run_history` | `{id, run: null \| number}` | JSON de `GET /history` (index) ou `GET /history/<n>` (détail), schémas de CONTRACT.md §Historique, ou `null` |
| `settings_get` | – | `{perfMode: "low" \| "balanced" \| "quality" \| "custom", memoryMb, noRemember, launchVia: "opaline" \| "official", joinServer, notifyServer, discordPresence, trayOnGame, autoTuned, systemRamMb}`. `joinServer` (défaut vrai) : Quick Play au lancement direct. `notifyServer` (vrai) : notification quand le serveur d'un pack passe de hors ligne à en ligne (relevé toutes les 90 s, le premier relevé ne notifie jamais, aucune notification pour les connexions de joueurs). `discordPresence` (vrai) : statut Discord. `trayOnGame` (vrai) : réduction dans la zone de notification. `autoTuned` : `perfMode` et `memoryMb` ont été choisis automatiquement au premier lancement et pas encore modifiés (l'interface affiche « Réglé automatiquement pour ton PC »). `launchVia` ne s'applique qu'avec une session Opaline : sans session, JOUER passe toujours par le Launcher Minecraft |
| `settings_set` | `{patch: {perfMode?, memoryMb?, noRemember?, launchVia?, joinServer?, notifyServer?, discordPresence?, trayOnGame?}}` (les champs absents ne changent pas ; changer `perfMode` ou `memoryMb` remet `autoTuned` à faux) | settings à jour |
| `settings_copy` | `{from, to, categories: ["keys" \| "mouse" \| "fov" \| "sound" \| "language" \| "accessibility"]}` | nombre de réglages copiés. Fusionne dans `options.txt` du pack `to` les lignes `clé:valeur` des catégories choisies lues dans celui de `from` (réutilise la fusion des modes de performance : les autres lignes de la destination sont conservées) : `keys` = `key_*` ; `mouse` = sensibilité, inversion, molette, défilement discret, souris brute ; `fov` = `fov`, `fovEffectScale`, `screenEffectScale`, `darknessEffectScale` ; `sound` = `soundCategory_*` ; `language` = `lang` ; `accessibility` = narrateur, contraste, sous-titres, chat (opacité, taille, largeur, délai…), bascule accroupi/sprint, etc. Les clés des modes de performance (`graphicsPreset`, `renderDistance`, `simulationDistance`, `particles`, `renderClouds`) ne sont jamais copiées. L'ancien `options.txt` de la destination est conservé dans `options.txt.bak`. Erreurs : mêmes packs, catégorie inconnue, destination non installée, source sans réglages ; refusé pendant le jeu ou une installation du pack `to` |
| `shaders_get` | `{id}` | `{available: bool, files: [string], active: null \| string}` : `available` = un jar `iris*` figure dans `mods/` (sinon l'interface masque la section) ; `files` = zips et dossiers de `shaderpacks/` ; `active` = `shaderPack=` de `config/iris.properties` si `enableShaders` n'est pas `false` et que le fichier existe |
| `shader_set` | `{id, name: null \| string}` | – : écrit `enableShaders` et `shaderPack` dans `config/iris.properties` par la fusion habituelle (`name` null : `enableShaders=false`, `shaderPack=` vide). `name` doit être un fichier de `shaderpacks/`. Refusé pendant le jeu. Le choix survit aux lancements : le mode de performance ne remplace `shaderPack` que si le zip choisi a disparu (Faible coupe toujours les shaders ; « Aucun » n'est jamais rallumé) |
| `open_folder` | `{id, kind: "mods" \| "shaderpacks" \| "screenshots" \| "logs" \| "instance" \| "backups"}` | – (`backups` : `backups/<pack>/`, créé au besoin ; les autres exigent un pack installé) |
| `pack_uninstall` | `{id}` | – : sauvegarde d'abord les mondes (`backups/<pack>/`), puis supprime l'instance de ce pack seulement (mods, Mes mods, configs, mondes). Jamais `shared/`, `backups/` ni les autres packs ; le pack repasse dans « À installer ». Refusé (erreur) si le jeu ou une installation de ce pack tourne, ou si un monde est ouvert (`session.lock`) : rien n'est alors supprimé |
| `official_close_launcher` | – | – : ferme le Launcher Minecraft seulement (`MinecraftLauncher.exe` et `Minecraft.exe` ; Linux `minecraft-launcher`), jamais `java` / `javaw` : fermeture douce, attente de 5 s au plus, puis forcée. Refusé avec « Une partie est en cours dans le Launcher Minecraft : quitte le jeu d'abord » si une partie tourne. L'interface relance ensuite `play` |
| `map_info` | `{id}` | `{name, url, reachable: bool, embeddable: bool}` : adresse de la carte résolue (suffixe MagicDNS pour un nom court) et sonde (un GET, corps ignoré). `embeddable` faux si le site envoie `X-Frame-Options` ou un `frame-ancestors` sans `*` ni http(s). Erreur si le pack n'a pas de carte |
| `open_map` | `{id}` | – : ouvre la même URL dans une fenêtre Tauri séparée « Carte – <Pack> » (`WebviewWindow` vers l'URL externe). Cette fenêtre n'a **aucune capability** (`capabilities/default.json` ne vise que `main`) et le gestionnaire de commandes refuse tout appel venant d'une fenêtre autre que `main` |
| `backups_list` | `{id}` | `[{id, createdAt, sizeBytes}]` (`createdAt` ISO 8601 UTC relu du nom du dossier), du plus récent au plus ancien |
| `backup_restore` | `{id, backupId}` | – : copie complète de la sauvegarde dans un dossier voisin, sauvegarde du `saves/` actuel (protégé de la rotation), puis échange. Refusé pendant le jeu, une installation du pack ou avec un monde ouvert ; `backupId` inconnu ou contenant `/`, `\`, `.` refusé |
| `playtime` | – | `{<packId>: {totalSecs, lastPlayed}}`, `lastPlayed` en secondes Unix : temps mesuré par Opaline en lancement direct (le mode officiel n'est pas mesuré : Opaline ne possède pas le processus) |
| `extra_mods_list` | `{id}` | `[{file, sizeBytes, title: null \| string, auto: bool, requiredBy: [string], warning: null \| string}]` : « Mes mods » du pack. `auto` : dépendance ajoutée automatiquement ; `warning` : même identifiant de mod que celui d'un mod du pack |
| `extra_mod_add` | `{id, path}` | nom du fichier : copie d'un `.jar` déposé sur la fenêtre (le chemin doit provenir du dernier événement natif de dépôt : sinon refusé). Nom simple `.jar`, 200 Mo au plus |
| `extra_mod_import` | `{id, name, dataBase64}` | nom du fichier : même chose depuis le sélecteur de fichier de l'interface (contenu en base64) |
| `extra_mod_remove` | `{id, file}` | fichiers retirés : le mod et les dépendances automatiques qui ne servent plus |
| `modrinth_search` | `{id, query}` | `[{projectId, title, description, downloads, iconUrl: null \| string}]` (`iconUrl` seulement en https sur `cdn.modrinth.com`), filtré sur le Minecraft et le loader du pack |
| `modrinth_add` | `{id, projectId}` | `{added: {project_id, title, version, file}, deps: [{project_id, title, version, file}], provided: [string]}` : version compatible, dépendances obligatoires résolues, hash sha512 vérifié ; `provided` = dépendances déjà fournies par le pack ; refus si le mod ou une incompatibilité est déjà présent |

Les commandes `extra_mod_*`, `modrinth_add`, `pack_uninstall`, `shader_set`, `settings_copy` et `backup_restore` réservent le pack (refus pendant le jeu ou une installation de ce pack, y compris une restauration en cours).
| `textures_dir` | – | `{dir: string \| null}`. Au démarrage, si les textures manquent, Opaline télécharge le client.jar officiel (hash vérifié, conservé dans `shared/`) et les extrait, puis émet `textures://ready`. `null` tant que ce n'est pas fait. Les PNG sont nommés `<nom>.png` et servis par `convertFileSrc` |
| `skins_list` | – | `[{id, name, model: "classic" \| "slim", active, dataUrl}]` |
| `skin_import` | `{name, model, pngBase64}` | `SkinView` (PNG 64×64 ou 64×32 validé) |
| `skin_apply` | `{id}` | – (API officielle `POST /minecraft/profile/skins`) |
| `skin_rename` | `{id, name}` | – |
| `skin_delete` | `{id}` | – |
| `capes_list` | – | `[{id, alias, active, url}]` |
| `cape_set` | `{id: string \| null}` | – |
| `friends` | – | `[{name, uuid, online, inGame, lastSeen: null \| string}]` (en ligne d'abord ; l'interface retire le joueur lui-même, même UUID ou même pseudo) |
| `player_head` | `{uuid}` | `{dataUrl}` (visage 8×8 + calque du chapeau, PNG, mis en cache) |

**Erreurs** : une commande en échec renvoie une chaîne lisible en français, sans jeton, sans adresse de serveur, sans chemin personnel complet.

### `friends` : périmètre v1

- La liste réunit les joueurs vus en ligne sur les serveurs des packs (échantillon du ping et `/stats`), mémorisés localement avec la date de dernière visite.
- `inGame` vaut vrai si le joueur est connecté au serveur d'un pack.
- Le statut personnel (En ligne / Absent / Invisible) de la maquette n'est pas réalisé en v1.
- « En ligne » = vu connecté sur le serveur d’un pack au dernier relevé ; aucune présence pair à pair, aucun port ouvert.

### Mode de lancement et écran de connexion

- Session Opaline valide : lancement direct. Sinon : Launcher Minecraft officiel (aucun contournement, aucun hors ligne). La connexion Microsoft est facultative ; dès que Mojang approuve l'app, la connexion réussit et le mode direct revient seul.
- L'écran de connexion propose « Se connecter avec Microsoft » et « Jouer via le Launcher Minecraft » (champ facultatif « Ton pseudo Minecraft » : tête et skin, sans authentification). Après `NOT_APPROVED`, il affiche « Opaline attend la validation de Mojang. En attendant, tu joues via le Launcher Minecraft : ton compte et ton skin y sont gérés. » puis continue en mode officiel.
- Mode officiel : vue Compte sans cape ni changement de skin (état vide explicite) ; têtes des amis et statut serveur inchangés. Seul `launcher_profiles.json` du Launcher officiel est lu ou écrit : jamais ses fichiers de comptes ni de jetons.

### Carte du serveur

- Vue « Carte » (icône du rail, visible seulement si le pack courant a `map`) : `<iframe sandbox="allow-scripts allow-same-origin" referrerpolicy="no-referrer">` plein cadre vers `map_info.url`, chargé à l'ouverture de la vue et déchargé en la quittant. Site injoignable ou qui refuse l'intégration : message clair et bouton « Ouvrir en grand » (`open_map`).
- Isolation : le cadre est d'une autre origine. Les scripts du noyau Tauri (`__TAURI__`, IPC) ne sont injectés que dans le cadre principal, et une origine distante n'obtient aucune commande sans capability (aucune n'existe pour elle) ; en plus, le gestionnaire de commandes d'Opaline refuse toute fenêtre autre que `main`.
- CSP (`tauri.conf.json`) : `frame-src http: https:`. **Compromis** : la liste blanche ne peut pas être propre à chaque pack (la CSP est statique), donc n'importe quel site http(s) peut être cadré. Il n'est jamais choisi que par le catalogue (`server.map`, validé), jamais par l'utilisateur ni par une page.

### Bureau pendant une partie (lancement direct seulement)

Une machine à états (`opaline_core::shell`, testée sans interface) décide à chaque début et fin de partie :

- **Zone de notification** (`trayOnGame`) : au lancement, la fenêtre se cache derrière une icône (menu « Afficher Opaline » / « Quitter » ; clic gauche : afficher) ; à `game://exited` elle est réaffichée, même si le réglage a été coupé entre-temps. Si l'icône ne peut pas être créée, la fenêtre reste visible. Plusieurs parties : réaffichage à la fin de la dernière. « Quitter » ne ferme pas le jeu.
- **Statut Discord** (`discordPresence`) : « Joue à <Pack> », horodatage de début, image `opaline`, aucun bouton. Uniquement si Discord tourne ; tout échec est silencieux. L'identifiant d'application est la constante de compilation `DISCORD_APP_ID` (`option_env!`) : absent ou non numérique, la fonctionnalité est désactivée (aucun fil, aucune connexion). Création de l'application : `opaline/README.md`. Mode officiel : pas de statut (jeu non suivi).

### Fond animé et gestion des packs (interface)

- « Fond animé » (Réglages) est un réglage de l'interface seulement (`localStorage` `opaline.bg`, actif par défaut ; la préférence « réduire les animations » du système est ignorée). Désactivé : image fixe du dégradé, sans boucle. Pauses automatiques : fenêtre cachée ou réduite, partie en cours. Sans WebGL : fond uni.
- Accueil : « Mes packs » (installés ; clic droit ou touche Menu / Maj+F10 : « Ouvrir le dossier » `open_folder`, « Vérifier et réparer » `pack_repair`, « Désinstaller… » `pack_uninstall` après confirmation) puis « À installer » (replié derrière « Voir les N packs disponibles » au-delà de 4 ; non pris en charge en dernier, grisés). Cliquer une carte « À installer » la sélectionne ; seul le bouton « Installer » lance `pack_install`. « Fichiers & mods » ne propose que les packs installés.

## 5. Vues

```
PackView = {id, name, description, mc, loader, loaderVersion, modsCount, thumbnail, widgets,
            supported: bool, installed: bool, updateAvailable: bool, installedVersionHash: string | null,
            hasServer: bool, hasStats: bool, hasMap: bool}
ServerView = {online: bool, latencyMs: number | null, players: {online, max, sample: [{name, uuid}]},
              version: string | null, motdOk: bool, viaTailscale: bool,
              help: null | "tailscaleNotInstalled" | "tailscaleDisconnected" | "noAccess" | "unexpectedServer" | "serverOffline"}
SkinView = {id, name, model, active, dataUrl}
```

## 6. Événements (`listen(nom)`)

| Événement | Charge utile |
|---|---|
| `auth://done` | `{attempt: number, ok: bool, profile: null \| {name, uuid}, error: null \| string, mojangPending: bool}`, jamais émis pour une tentative annulée. `mojangPending` vrai : refus `NOT_APPROVED`, l'interface explique que Mojang n'a pas encore validé l'app et continue en mode officiel |
| `install://progress` | `{packId, stage, done, total}`, avec les étapes toujours émises dans cet ordre : `java`, `minecraft`, `assets`, `loader`, `mods`, `overrides` (seulement si le pack a des overrides), `textures` |
| `install://done` | `{packId, ok: bool, error: null \| string}` |
| `update://progress` | `{done: octets reçus, total: octets \| null}` (téléchargement de l'installeur d'Opaline) |
| `update://done` | `{ok: false, error: string}` : émis seulement en cas d'échec (sur succès Opaline quitte) |
| `textures://ready` | `{dir: string}` : textures disponibles (émis au démarrage dès qu'elles sont extraites, y compris avant toute installation de pack) |
| `game://exited` | `{packId, code: number \| null, logTail: null \| string, crash: null \| {findings: [{kind, text}], crashFile: null \| string, report: string}}` (log filtré : ni jeton, ni adresse, ni chemin personnel). `crash` n'est renseigné que si `code` n'est pas 0 : causes en français (`kind` parmi `missingMod`, `modVersion`, `incompatible`, `java`, `mixin`, `suspect`, `memory`, `error`, 8 au plus) tirées du `latest.log` et du rapport de crash de cette session (`crashFile` = `crash-reports/<nom>`), `report` = texte à copier |

## 6 bis. Fichiers écrits par Opaline

Sous `%APPDATA%\Opaline` (jamais dans `.minecraft`, sauf le profil du mode officiel) :

- `config.json` : réglages (`settings_get`), sans secret ; absent au premier lancement : Opaline le crée avec le mode et la mémoire choisis d'après la RAM et le GPU (voir ci-dessous).
- `playtime.json` : `{<packId>: {totalSecs, lastPlayed}}`, écriture atomique, mis à jour à la fin de chaque partie directe.
- `backups/<pack>/<horodatage>/` : copies de `saves/` (`AAAAMMJJ-HHMMSS`), faites avant installation, mise à jour, réparation, restauration et désinstallation ; 3 sont gardées (les 2 plus récentes et la plus ancienne) ; pas de nouvelle copie si `saves/` est inchangé (empreinte `<id>.fp`).
- `instances/<pack>/opaline-extra-mods/index.json` : index de « Mes mods » (jars perso, projets Modrinth, dépendances automatiques) à côté des jars ; recopiés dans `mods/` à chaque lancement, jamais supprimés par une mise à jour du pack.
- `instances/<pack>/options.txt.bak` : ancien `options.txt` avant `settings_copy`.

Réglage automatique (premier lancement seulement, aucun `config.json`) : RAM < 6 Go → Faible, < 12 Go → Équilibré, sinon Qualité ; GPU intégré (nom Windows `Win32_VideoController` via PowerShell, Linux `lspci`) : jamais plus que Équilibré, et Faible sous 8 Go ; mémoire = valeur du mode plafonnée à 70 % de la RAM ; RAM inconnue : Équilibré. Aucun GPU trouvé : seule la RAM compte.

## 7. Fenêtre

- `decorations: false`, redimensionnable, taille minimale 800×600, ombre native.
- La bande du haut porte `data-tauri-drag-region`.
- Les boutons de fenêtre appellent l'API window de Tauri : `minimize`, `toggleMaximize` et `close`.
- Le nom de l'application est `Opaline`, et son dossier de données `%APPDATA%\Opaline` (Windows) ou `~/.local/share/Opaline` (Linux).
- L'ancien dossier `ClassroomRunLauncher` est ignoré, sans migration.
