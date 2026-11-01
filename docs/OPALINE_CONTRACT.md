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

- `loader` vaut `fabric`, `quilt`, `neoforge` ou `forge` (tous pris en charge ; `loaderVersion` = version du loader). Installation : voir docs/api/loaders.md (Fabric/Quilt via leur meta ; NeoForge/Forge en exécutant les processors client de l'installeur officiel dans l'instance, sorties vérifiées par sha1 quand l'installeur les fournit). Un loader inconnu donne `supported: false`.
- L'hôte du serveur n'est jamais affiché dans l'interface.

## 4. Commandes Tauri (`invoke(nom, args)`)

| Commande | Arguments | Retour |
|---|---|---|
| `app_info` | – | `{version, update: null \| {version, url}}` |
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
| `play` | `{id}` | `{started: bool, reason: null \| string, serverListed: bool, via: "direct" \| "official", message?: string}` (`via` et `serverListed` toujours présents, y compris si `started` est faux). `via: "direct"` (session Opaline valide) : puis événement `game://exited`. `via: "official"` (pas de session) : Opaline écrit la version du loader sous l'identifiant propre `opaline-<packId>` (`.minecraft/versions/opaline-<packId>/`, champ `id` réécrit, jar renommé, marqueur `opaline.managed` ; un dossier de ce nom sans marqueur n'est jamais touché), copie vers `.minecraft/libraries` les libraries du loader présentes et, pour Forge / NeoForge, les fichiers que le Launcher ne peut pas télécharger (sorties des processors, libraries sans URL : liste `opaline-loader-files.json` écrite à l'installation ; absente, il faut « Vérifier et réparer »), fusionne le profil `opaline-<packId>` (`Opaline · <nom>`, `lastUsed` récent, `gameDir` = instance du pack, `javaArgs` = arguments par défaut du Launcher officiel avec seulement `-Xmx<RAM>M` remplacé) dans `launcher_profiles.json` (sauvegarde `.opaline.bak`, écriture atomique, reste du fichier intact) et ouvre le Launcher Minecraft ; `message` : « Profil prêt dans le Launcher Minecraft : clique sur Jouer », ou « Ouvre le Launcher Minecraft et choisis le profil Opaline · <pack> » s'il est introuvable. Si le Launcher officiel tourne déjà (il réécrirait le fichier), `started` vaut faux avec « Ferme le Launcher Minecraft puis relance JOUER (il écraserait le profil Opaline) ». Pas de `game://exited` (processus non possédé) ; l'id du pack n'est réservé que pendant la préparation. Erreur lisible si `launcher_profiles.json` manque (« Installe et ouvre une fois le Launcher Minecraft ») ou est illisible. Mode direct : Lance le jeu dans l'instance du pack, avec ses mods, jusqu'au **menu principal** de Minecraft : aucune connexion automatique, aucun ping ni appel à Tailscale. Si le pack déclare un serveur, Opaline l'ajoute au préalable à la liste Multijoueur de l'instance (`servers.dat`, entrée nommée d'après le pack) : ajoutée si aucune entrée n'a déjà cette adresse ; si une entrée porte déjà le nom du pack avec une autre adresse, seule son adresse est mise à jour ; les autres entrées ne sont jamais modifiées. Un nom court n'est écrit que si le suffixe MagicDNS est déjà connu (dernier `server_status`), sinon l'entrée est écrite au lancement suivant. `serverListed` vaut vrai si l'entrée figure dans la liste à l'issue (faux : pas de serveur, suffixe inconnu, nom/adresse trop long, `servers.dat` illisible ou écriture impossible) ; l'interface n'affiche « Ajouté à ta liste Multijoueur » que si vrai |
| `server_status` | `{id}` | `ServerView` (§5), ou `null` si le pack n'a pas de serveur |
| `run_stats` | `{id}` | JSON de `GET /stats` du serveur, ou `null` (seulement pour le widget `classroomRun`) |
| `run_history` | `{id, run: null \| number}` | JSON de `GET /history` (index) ou `GET /history/<n>` (détail), schémas de CONTRACT.md §Historique, ou `null` |
| `settings_get` | – | `{perfMode: "low" \| "balanced" \| "quality" \| "custom", memoryMb, noRemember, launchVia: "opaline" \| "official", systemRamMb}`. `launchVia` ne s'applique qu'avec une session Opaline : sans session, JOUER passe toujours par le Launcher Minecraft |
| `settings_set` | `{perfMode?, memoryMb?, noRemember?, launchVia?}` | settings à jour |
| `open_folder` | `{id, kind: "mods" \| "shaderpacks" \| "screenshots" \| "logs" \| "instance"}` | – |
| `textures_dir` | – | `{dir: string \| null}`. Au démarrage, si les textures manquent, Opaline télécharge le client.jar officiel (hash vérifié, conservé dans `shared/`) et les extrait, puis émet `textures://ready`. `null` tant que ce n'est pas fait. Les PNG sont nommés `<nom>.png` et servis par `convertFileSrc` |
| `skins_list` | – | `[{id, name, model: "classic" \| "slim", active, dataUrl}]` |
| `skin_import` | `{name, model, pngBase64}` | `SkinView` (PNG 64×64 ou 64×32 validé) |
| `skin_apply` | `{id}` | – (API officielle `POST /minecraft/profile/skins`) |
| `skin_rename` | `{id, name}` | – |
| `skin_delete` | `{id}` | – |
| `capes_list` | – | `[{id, alias, active, url}]` |
| `cape_set` | `{id: string \| null}` | – |
| `friends` | – | `{friends: [{name, uuid, online, inGame, lastSeen: null \| string, friend: bool, p2p: bool, packId: null \| string, packVersion: null \| string}], p2p: {state: "ok" \| "noIdentity" \| "noTailscale", listenFailed: bool}}` |
| `friend_add` | `{name}` | `{name, uuid}` (pseudo validé puis résolu en UUID par l'API publique Mojang ; erreur si inconnu, si c'est soi-même ou au-delà de 200 amis) |
| `friend_remove` | `{uuid}` | – |
| `friend_invite` | `{uuid, packId}` | – (invitation P2P à un ami vu sur le tailnet à la dernière actualisation ; erreur lisible s'il est injoignable, ne t'a pas dans sa liste, ou si l'invitation précédente date de moins de 30 s) |
| `player_head` | `{uuid}` | `{dataUrl}` (visage 8×8 + calque du chapeau, PNG, mis en cache) |

**Erreurs** : une commande en échec renvoie une chaîne lisible en français, sans jeton, sans adresse de serveur, sans chemin personnel complet.

### `friends` : périmètre v1

- La liste réunit les joueurs vus en ligne sur les serveurs des packs (échantillon du ping et `/stats`), mémorisés localement avec la date de dernière visite.
- `inGame` vaut vrai si le joueur est connecté au serveur d'un pack.
- Le statut personnel (En ligne / Absent / Invisible) de la maquette n'est pas réalisé en v1.
- Amis choisis : `friend_add` / `friend_remove`, stockés dans `friends_list.json` à la racine du dossier de données (écriture atomique). Une ligne par UUID : joueurs vus en ligne, amis ajoutés et pairs Opaline du tailnet sont fusionnés. `friend` : dans la liste ; `p2p` : une présence Opaline a répondu (si `friend` est faux, l'interface propose « Ajouter » sous « Sur ton Tailscale »).
- Identité de l'utilisateur : profil de la session Opaline, sinon `mc_name` / `mc_uuid` du mode officiel. Sans identité : `p2p.state = "noIdentity"`, ni écoute ni découverte. Tailscale non connecté : `"noTailscale"` (présence inconnue). `listenFailed` : l'écoute a échoué (pare-feu Windows, port pris) ; Opaline ne modifie jamais le pare-feu.

### Protocole P2P v1 (présence et invitations sur le tailnet)

Le tailnet est la frontière de confiance : l'UUID annoncé n'est pas authentifié, seuls les appareils du tailnet joignent le port.

- Écoute uniquement sur l'IPv4 Tailscale de ce PC (`tailscale ip -4`, plage 100.64.0.0/10 vérifiée), port **25590**, jamais `0.0.0.0` ni l'IP LAN ; démarrée seulement si Tailscale est `Running` et qu'une identité existe, arrêtée ou déplacée quand l'état change (contrôle toutes les 5 s tant qu'elle n'écoute pas, puis 15 s).
- `GET /opaline/v1/presence` : `{app: "opaline", v: 1, uuid, name, inGame, packId: null \| string, packVersion: null \| string, appVersion}` (`packVersion` = hash de version du pack installé). Rien d'autre : ni chemin, ni IP, ni jeton. `inGame` ne vaut vrai que pour un pack lancé par Opaline (mode direct) : en mode Launcher officiel le processus n'est pas suivi, `inGame` reste faux.
- `POST /opaline/v1/invite` avec `{fromUuid, fromName, packId}` (JSON strict, ≤ 1 Ko, pseudo / UUID / id validés) : 200 si `fromUuid` est dans la liste d'amis locale ET que l'IP source de la connexion est celle connue pour cet ami, ou, à défaut (fenêtre masquée), confirmée à la volée par sa `presence` (délai 800 ms, UUID annoncé identique ; après l'anti-spam) (événement `friends://invite`), 400 invalide, 403 inconnu, 429 si moins de 30 s depuis la dernière invitation du même expéditeur ou si 20 expéditeurs ont déjà invité dans la fenêtre de 30 s.
- Limites : en-têtes ≤ 8 Ko, lecture 2 s au total, 8 connexions simultanées au plus, une requête par connexion (`Connection: close`), méthode ou chemin inconnu : 404, corps > 1 Ko : 413.
- Découverte : `tailscale status --json`, pairs `Online`, requête `presence` vers leur première IPv4 Tailscale (délai 800 ms, 8 en parallèle) à chaque appel de `friends` (toutes les 15 s fenêtre visible). Adresses et noms de machine ne sont ni affichés, ni loggés, ni conservés hors mémoire.

### Mode de lancement et écran de connexion

- Session Opaline valide : lancement direct. Sinon : Launcher Minecraft officiel (aucun contournement, aucun hors ligne). La connexion Microsoft est facultative ; dès que Mojang approuve l'app, la connexion réussit et le mode direct revient seul.
- L'écran de connexion propose « Se connecter avec Microsoft » et « Jouer via le Launcher Minecraft » (champ facultatif « Ton pseudo Minecraft » : tête et skin, sans authentification). Après `NOT_APPROVED`, il affiche « Opaline attend la validation de Mojang. En attendant, tu joues via le Launcher Minecraft : ton compte et ton skin y sont gérés. » puis continue en mode officiel.
- Mode officiel : vue Compte sans cape ni changement de skin (état vide explicite) ; têtes des amis et statut serveur inchangés. Seul `launcher_profiles.json` du Launcher officiel est lu ou écrit : jamais ses fichiers de comptes ni de jetons.

## 5. Vues

```
PackView = {id, name, description, mc, loader, loaderVersion, modsCount, thumbnail, widgets,
            supported: bool, installed: bool, updateAvailable: bool, installedVersionHash: string | null,
            hasServer: bool, hasStats: bool}
ServerView = {online: bool, latencyMs: number | null, players: {online, max, sample: [{name, uuid}]},
              version: string | null, motdOk: bool, viaTailscale: bool,
              help: null | "tailscaleNotInstalled" | "tailscaleDisconnected" | "noAccess" | "unexpectedServer" | "serverOffline"}
SkinView = {id, name, model, active, dataUrl}
```

## 6. Événements (`listen(nom)`)

| Événement | Charge utile |
|---|---|
| `auth://done` | `{attempt: number, ok: bool, profile: null \| {name, uuid}, error: null \| string, mojangPending: bool}`, jamais émis pour une tentative annulée. `mojangPending` vrai : refus `NOT_APPROVED`, l'interface explique que Mojang n'a pas encore validé l'app et continue en mode officiel |
| `install://progress` | `{packId, stage, done, total}`, avec les étapes toujours émises dans cet ordre : `java`, `minecraft`, `assets`, `loader`, `mods`, `textures` |
| `install://done` | `{packId, ok: bool, error: null \| string}` |
| `textures://ready` | `{dir: string}` : textures disponibles (émis au démarrage dès qu'elles sont extraites, y compris avant toute installation de pack) |
| `game://exited` | `{packId, code: number \| null, logTail: null \| string}` (log filtré : ni jeton ni adresse) |
| `friends://invite` | `{fromUuid, fromName, packId}` : invitation reçue d'un ami (protocole P2P ci-dessus) ; l'interface propose « Rejoindre » |

## 7. Fenêtre

- `decorations: false`, redimensionnable, taille minimale 800×600, ombre native.
- La bande du haut porte `data-tauri-drag-region`.
- Les boutons de fenêtre appellent l'API window de Tauri : `minimize`, `toggleMaximize` et `close`.
- Le nom de l'application est `Opaline`, et son dossier de données `%APPDATA%\Opaline` (Windows) ou `~/.local/share/Opaline` (Linux).
- L'ancien dossier `ClassroomRunLauncher` est ignoré, sans migration.
