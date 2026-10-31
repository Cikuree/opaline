# Launcher Classroom Run

Launcher Rust (eframe/egui, interface minimale en attendant l'interface web) : installe Java 25, Minecraft 26.3,
Fabric Loader, les mods et les shaders du pack packwiz dans une instance isolée, puis lance le jeu connecté au serveur.
La logique (pack distant, mise à jour, modes de performance, auth Microsoft, ping, Tailscale) est dans des modules
sans dépendance à egui, réutilisables comme commandes Tauri.

## Dossier de données

`%APPDATA%\ClassroomRunLauncher\` (Windows), surchargeable avec la variable `CLASSROOM_LAUNCHER_HOME`.
Le launcher n'écrit jamais ailleurs et jamais dans `.minecraft`. Sous ce dossier : `config.json` (sans secret),
`java/`, `libraries/`, `assets/`, `versions/`, `downloads/`, `instance/` (dossier de jeu : `mods/`, `logs/`, mondes).
Les chemins issus des manifestes sont normalisés et refusés s'ils sortent du dossier (`..`, absolus, `:`).

## Pack, mods et shaders

- Par défaut le pack vient de `https://github.com/Cikuree/classroom-run/releases/latest/download/pack.zip`
  (constante `config::REPO`). Le zip est extrait dans `downloads/pack-remote` (zip-slip refusé), puis les hashes sont
  vérifiés : `pack.toml` -> `index.toml` -> chaque `.pw.toml` -> fichier téléchargé.
- Option avancée (dev) : `pack_dir` (config.json, ou `--pack`) = dossier local contenant `pack.toml`, qui remplace le zip.
- Seuls les `side = client|both` sont installés. Un métafichier est installé dans le dossier d'instance qui porte le nom
  de son dossier dans le pack : `mods/` -> `instance/mods/`, `shaderpacks/` -> `instance/shaderpacks/`.
- Le jar classroomrun vient du pack (`mods/classroomrun.pw.toml`). Repli dev sans entrée dans le pack : `mod_jar`
  (fichier ou dossier), vide = `<pack_dir>/../build/libs`.
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

Device code (tenant `consumers`, scope `XboxLive.signin offline_access`) -> XBL -> XSTS -> `login_with_xbox` -> entitlements -> profil.
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

- Adresse du serveur : jamais affichée ni saisie. Au lancement, `classroom.<MagicDNSSuffix>:25565` où le suffixe vient de
  `tailscale status --json` (calculé à l'exécution, jamais écrit ni loggé), repli `classroom:25565`. Surcharge dev : `CLASSROOM_SERVER`
  (le contrôle du MOTD est alors ignoré). Stats : port 25580.
- Jouer est autorisé si le ping SLP répond et que le MOTD contient « Classroom Run » (sinon « Serveur inattendu »). Côté serveur :
  `server.properties` `motd=Classroom Run`. L'état Tailscale (`tailscale status --json`, `BackendState` = `Running`) ne sert qu'au message d'aide quand le ping échoue.
- Polling du device code : une erreur réseau ponctuelle ne casse pas le flux (réessai jusqu'à l'échéance).
- `ping::ping` : Server List Ping, protocole 777 (26.3). `sysmem::total_ram_mb` : RAM physique.
- Au démarrage : lecture de `.../releases/latest/download/launcher.json` ; version plus récente = bandeau + bouton qui ouvre l'URL de l'OS (https seulement). Erreur réseau silencieuse (`CLASSROOM_DEBUG=1` pour la trace). Pas d'auto-remplacement.
- `config::PackProfile` regroupe les éléments propres à un pack (URL, serveur) en vue du multi-pack.

## Utilisation

- Interface : `cargo run --release` (connexion Microsoft, mode de performance, Installer, Jouer, stats du run).
- CLI (tests/dev) : `classroom-launcher install|play [--root D] [--pack D] [--mod-jar P] [--nick N]`.
  Le jeu est lancé avec `--quickPlayMultiplayer`; sortie du jeu dans `instance/logs/launcher-game.log`.

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
