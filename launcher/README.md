# opaline_core

Bibliothèque Rust (sans interface graphique) du launcher Opaline ; le backend Tauri (`opaline/src-tauri`) l'appelle via
`service`. Un petit binaire de développement, `opaline-cli`, sert aux tests manuels :
`cargo run --bin opaline-cli -- install <id> [--pack <dossier packwiz>] [--root <dossier>]` puis `play <id> --nick <pseudo>`
avec `CLASSROOM_DEV_OFFLINE=1`.

## Dossier de données

`%APPDATA%\Opaline\` (Windows), `~/.local/share/Opaline/` (Linux), surchargeable par `OPALINE_HOME`. Jamais `.minecraft`.
- `shared/` : `java/`, `libraries/`, `assets/`, `versions/`, `downloads/` (communs à tous les packs, téléchargés une fois).
- `instances/<id>/` : dossier de jeu isolé d'un pack (`mods/`, `logs/`, mondes), `installed.ok` (version, hash de l'index = `packHash`),
  `opaline-pack.json` (manifeste installé : sert au changelog et à la liste des mods).
- `textures/` (PNG extraits du `client.jar`, jamais redistribués), `skins/` (garde-robe), `cache/` (catalogue, têtes, amis), `config.json` (sans secret).

## Catalogue multi-pack

`packs.json` de la dernière release (contrat §3), repli sur le dernier catalogue en cache hors ligne. Entrées invalides ignorées une à une
(id non simple, URL non https, `packHash` absent). Loader autre que `fabric` : `supported: false`. Surcharge de dev : `OPALINE_PACKS_URL`
(URL ou chemin de fichier). Le zip téléchargé doit avoir un index dont le hash égale `packHash`.

## Autres modules

- `skins` : garde-robe, PNG 64x64 / 64x32 validé, API Mojang officielle (jeton obligatoire, jamais dans une erreur), textures depuis `textures.minecraft.net` seulement.
- `head` : profil `sessionserver.mojang.com`, visage 8x8 + chapeau, cache 24 h. `friends` : joueurs vus par ping, mémorisés localement.
- `serverview` : ping (+ diagnostic Tailscale pour un nom court ; ping direct pour un serveur public). `servers` : ajout du serveur du pack à `servers.dat` (NBT). `stats::fetch_value` : `/stats`, `/history[/<n>]` (404 = absent).

## Pack, mods et shaders

- Le pack vient de `pack-<id>.zip` (URL du catalogue ; racine : dossier `pack/`).
  Le zip est extrait dans `shared/downloads/pack-<id>` (zip-slip refusé), puis les hashes sont
  vérifiés : `pack.toml` -> `index.toml` -> chaque `.pw.toml` -> fichier téléchargé.
- Dev : `--pack` (opaline-cli) = dossier local contenant `pack.toml`, qui remplace le zip.
- Seuls les `side = client|both` sont installés. Un métafichier est installé dans le dossier d'instance qui porte le nom
  de son dossier dans le pack : `mods/` -> `instance/mods/`, `shaderpacks/` -> `instance/shaderpacks/`.
- Le jar classroomrun vient du pack (`mods/classroomrun.pw.toml`). Repli dev sans entrée dans le pack : `mod_jar`
  (fichier ou dossier), uniquement si fourni explicitement ; opaline-cli cherche `build/libs` au-dessus du pack.
- Fichiers absents du pack, supprimés à chaque installation dans l'instance isolée seulement (`.minecraft` n'est jamais touché) :
  `.jar` de `mods/`, `.zip` de `shaderpacks/` et de `resourcepacks/`. Les réglages (`*.zip.txt`) et les dossiers sont conservés.
- `pack.zip` : délai global de 120 s, 50 Mo maximum, extraction bornée (5000 entrées, 300 Mo décompressés) ; le JRE est borné aussi (50 000 entrées, 2 Go).
- Détection « le pack a changé » : `installed.ok` mémorise le hash de l'index (`[index] hash` de `pack.toml`). Au clic sur Jouer, le champ
  `pack` de `launcher.json` (la CI y publie ce même hash) est comparé ; s'il diffère, le launcher propose la mise à jour avant de lancer.
  Hors ligne, erreur, champ absent ou pack local : on joue avec l'installé.
- `installed.ok` mémorise pack, Minecraft, Fabric et le zip de shaders ; le lancement s'en sert sans réseau.

## Modes de performance (`perf.rs`)

Faible / Équilibré / Qualité / Personnalisé (config.json `perf_mode`). Hors Personnalisé, à chaque lancement le launcher
fusionne ses clés dans `instance/options.txt` (autres lignes conservées, fichier créé si absent) et `instance/config/iris.properties`,
et fixe la mémoire (2048 / 4096 / 6144 Mo, Qualité plafonnée à 70 % de la RAM physique). Valeurs vérifiées dans les sources 26.3 :
`graphicsPreset` `"fast"|"fancy"|"fabulous"`, `renderClouds` `"false"|"fast"|"true"`, `particles` entier (0 tout, 1 réduit, 2 minimal).
Personnalisé : aucune écriture.

## Connexion Microsoft (`auth.rs`)

Principal (`mode: browser`, `auth_browser.rs`) : code d'autorisation OAuth 2.0 + PKCE S256 (client public, tenant `consumers`, scope `XboxLive.signin offline_access`,
`prompt=select_account`, `state` aléatoire vérifié). Opaline ouvre lui-même la page Microsoft ; la redirection arrive sur `http://localhost:<port>`
(port éphémère, serveur sur 127.0.0.1 uniquement). Le serveur accepte une seule requête `GET /` valide (bon `state`, `code` ou `error`), répond une page
statique sans script ni contenu reflété, rejette le reste (404/405/400), s'arrête après la réponse, expire après 5 min et s'annule par `auth_cancel`.
Port : Microsoft ignore le port des URI de redirection loopback `localhost` des apps de bureau (RFC 8252 §7.3), donc l'URI enregistrée `http://localhost` suffit.
Secours (`mode: device`) : code d'appareil (nécessite « Autoriser les flux de clients publics »).
Ensuite, pour les deux : XBL -> XSTS -> `login_with_xbox` -> entitlements -> profil.
- ID client Azure (public) fixé à la compilation : `CLASSROOM_MS_CLIENT_ID=... cargo build --release`. Vide = « Connexion Microsoft non configurée » et Jouer désactivé.
- Le refresh token est stocké dans le trousseau système (crate `keyring` 4.2, Credential Manager Windows, Secret Service Linux), en octets
  (`set_secret`), jamais dans un fichier, un log ou un `Debug`. Si le trousseau échoue (limite de taille, service absent), un avertissement
  s'affiche et la connexion reste valable pour cette exécution.
- Session et refresh token restent aussi en mémoire (`AuthManager`) : Jouer ne dépend pas du trousseau dans la même exécution.
  Une déconnexion invalide toute connexion ou rafraîchissement en cours (aucun stockage après `sign_out`).
- Case « Ne pas mémoriser la connexion sur ce PC » (config.json `no_remember`) : rien n'est écrit dans le trousseau, le jeton ne vit
  qu'en mémoire jusqu'à la fermeture du launcher. À cocher sur un PC partagé ; la cocher efface aussi un jeton déjà mémorisé.
- Les jetons sont en mémoire et passés à Java en ligne de commande (`--accessToken`), comme dans tous les launchers : un autre
  processus du même utilisateur peut les lire pendant la partie. Les logs masquent le jeton. La ligne de commande loggée masque le token et l'adresse du serveur.
- Rafraîchissement silencieux au démarrage et avant chaque lancement ; refresh token rejeté (`invalid_grant`) : effacé, reconnexion demandée.
- Erreurs expliquées : XSTS 2148916233 (pas de compte Xbox), 2148916238 (compte enfant), pas de licence, 403 sur `login_with_xbox` (app non approuvée par Mojang).
- URL injectables : `auth::Endpoints` (`single_base` pour un serveur de fixtures).
- Hors ligne : seulement avec `CLASSROOM_DEV_OFFLINE=1` (champ pseudo visible alors). L'XUID n'est pas récupéré (`--xuid` vide) ; `--userType` n'existe pas dans le JSON 26.3 et n'est pas ajouté.

## Serveur, Tailscale, mise à jour

- Adresse du serveur : jamais affichée ni saisie. Nom court (sans point) : `<nom>.<MagicDNSSuffix>` avec le suffixe de `tailscale status --json`
  (calculé à l'exécution lors de `server_status`, gardé en mémoire seulement, jamais écrit ni loggé) ; nom complet ou IP : tel quel, sans Tailscale.
  Port omis s'il vaut 25565. Surcharge dev : `CLASSROOM_SERVER`. Stats : `statsPort` (0 = indisponibles).
- Jouer lance le jeu jusqu'au menu principal (aucune connexion automatique, aucun ping ni contrôle Tailscale) ; avant, le serveur du pack est ajouté à
  `servers.dat` de l'instance (entrée nommée d'après le pack ; si le suffixe MagicDNS d'un nom court est encore inconnu, l'entrée est écrite au lancement suivant).
  Le statut du serveur (carte d'Opaline) : ping SLP, le MOTD doit contenir le texte du pack (sinon « Serveur inattendu »).
- Polling du device code : une erreur réseau ponctuelle ne casse pas le flux (réessai jusqu'à l'échéance).
- `ping::ping` : Server List Ping, protocole 777 (26.3). `sysmem::total_ram_mb` : RAM physique.
- Au démarrage : lecture de `.../releases/latest/download/launcher.json` ; version plus récente = bandeau + bouton qui ouvre l'URL de l'OS (https seulement). Erreur réseau silencieuse (`CLASSROOM_DEBUG=1` pour la trace). Mise à jour en un clic : `update::download` (sha256 de `launcher.json` obligatoire, URL limitée aux releases de Cikuree/opaline) puis `update::apply` (installeur NSIS `/P /R /UPDATE` sous Windows, remplacement atomique de `$APPIMAGE` sous Linux) ; sans hash ou hors AppImage, seul le lien manuel reste.
- `config::PackProfile` regroupe les éléments propres à un pack (URL, serveur) en vue du multi-pack.

## Utilisation

- Interface : `cargo run --release` (connexion Microsoft, mode de performance, Installer, Jouer, stats du run).
- CLI (tests/dev) : `classroom-launcher install|play [--root D] [--pack D] [--mod-jar P] [--nick N]`.
  Le jeu est lancé au menu principal ; sortie du jeu dans `instance/logs/launcher-game.log`.

## Réseau injectable

`install::Endpoints` et `auth::Endpoints` (manifeste Mojang, meta Fabric, API Adoptium, serveur d'assets) peut pointer vers un serveur de
fixtures ; les URL de téléchargement des libraries/mods viennent des JSON et peuvent aussi y pointer.

## Intégrité et confiance

- Mojang : hashes sha1 du JSON de version et des fichiers, tailles contrôlées quand elles sont connues.
- Java : sha256 fourni par l'API Adoptium (même source que l'archive : protège de la corruption, pas d'une compromission d'Adoptium).
- Mods : hash de l'index packwiz puis hash de chaque métafichier (sha512 le plus souvent).
- Libraries Fabric : le profil de meta.fabricmc.net fournit un sha1 ; s'il manque, le launcher télécharge le fichier
  `.sha1` publié à côté sur le Maven Fabric. Ce `.sha1` vient du même serveur que le jar : il détecte la corruption
  mais fait confiance à maven.fabricmc.net / meta.fabricmc.net (HTTPS) contre une compromission du serveur.
- `installed.ok` (racine des données) est écrit en toute fin d'installation (pack, Minecraft, Fabric) et exigé pour jouer ;
  supprimé au début de chaque installation. Fichiers non-mods de l'index packwiz : copiés depuis le dossier du pack
  vers `instance/` après vérification du hash.
- Réseau : timeout de connexion 15 s et abandon après 60 s sans donnée reçue (pas de timeout total).
