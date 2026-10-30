# Launcher Classroom Run

Launcher Rust (eframe/egui) : installe Java 25, Minecraft 26.3, Fabric Loader et les mods du pack packwiz dans une
instance isolée, puis lance le jeu connecté au serveur. Étape 2 : mode hors ligne uniquement (auth Microsoft = étape 3,
emplacement prévu dans `src/auth.rs`).

## Dossier de données

`%APPDATA%\ClassroomRunLauncher\` (Windows), surchargeable avec la variable `CLASSROOM_LAUNCHER_HOME`.
Le launcher n'écrit jamais ailleurs et jamais dans `.minecraft`. Sous ce dossier : `config.json` (sans secret),
`java/`, `libraries/`, `assets/`, `versions/`, `downloads/`, `instance/` (dossier de jeu : `mods/`, `logs/`, mondes).
Les chemins issus des manifestes sont normalisés et refusés s'ils sortent du dossier (`..`, absolus, `:`).

## Manifeste et jar classroomrun

- Le manifeste packwiz est lu depuis un DOSSIER LOCAL (`pack_dir`, contenant `pack.toml`), pas d'URL distante.
  Seuls les mods `side = client|both` sont installés ; hashes de l'index et des fichiers vérifiés (sha1/256/512, md5).
  Les fins de ligne CRLF (checkout Git Windows) sont tolérées pour le hash des fichiers .toml.
- Le jar classroomrun n'est pas dans le manifeste : `mod_jar` (fichier, ou dossier dont le plus récent
  `classroomrun*.jar` hors `-sources`/`-dev` est pris). Vide = `<pack_dir>/../build/libs`.
- À chaque installation, les `.jar` de `instance/mods` absents du manifeste sont supprimés (dossier isolé uniquement).

## Utilisation

- Interface : `cargo run --release` (pseudo, serveur `hôte[:port]`, dossier du pack, jar, Installer, Jouer, stats
  `http://<hôte>:25580/stats` rafraîchies toutes les 10 s).
- CLI (tests/dev) : `classroom-launcher install|play [--root D] [--pack D] [--mod-jar P] [--nick N] [--host H[:P]] [--no-server]`.
  Le jeu est lancé avec `--quickPlayMultiplayer <hôte:port>`; sortie du jeu dans `instance/logs/launcher-game.log`.

## Réseau injectable

`install::Endpoints` (manifeste Mojang, meta Fabric, API Adoptium, serveur d'assets) peut pointer vers un serveur de
fixtures ; les URL de téléchargement des libraries/mods viennent des JSON et peuvent aussi y pointer.
