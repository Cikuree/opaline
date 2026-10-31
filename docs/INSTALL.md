# Installation Classroom Run

Les téléchargements sont sur la page **Releases** du dépôt.

## Joueur

1. Ouvrir la page **Releases** du dépôt Cikuree/opaline et télécharger l'installeur **Opaline** : `Opaline_X.Y.Z_x64-setup.exe` (Windows) ou `Opaline_X.Y.Z_amd64.AppImage` (Linux ; `chmod +x` puis lancer).
2. Lancer Opaline et se connecter avec son compte Microsoft (le compte doit posséder Minecraft Java).
3. Choisir un pack dans la liste, cliquer **Installer**, puis **Jouer**.
4. Les mises à jour d'un pack sont proposées par Opaline ; Opaline signale aussi ses propres mises à jour.
5. Pour rejoindre un serveur de pack, [Tailscale](https://tailscale.com/download) doit être installé et connecté au tailnet de l'hôte.

### Alternative : Prism Launcher

1. Télécharger `classroom-run-prism.zip` depuis la même page Releases (ne pas le décompresser).
2. Dans Prism Launcher : **Ajouter une instance** → **Importer** → choisir le zip → **OK**.
3. Lancer l'instance (Java 25 : Prism le propose automatiquement). À chaque nouvelle version : réimporter le nouveau zip et supprimer l'ancienne instance.

L'instance contient : Fabric API, le mod `classroomrun`, et côté client Sodium, Iris et le shaderpack Complementary Reimagined.

## Hôte du serveur

Serveur Fabric 26.3, loader 0.20.1, Java 25. Dans `mods/` :

- le jar `classroomrun-<version>.jar` (page Releases) ;
- les mods `side = "server"` ou `"both"` de `packs/classroom-run/mods/*.pw.toml` : Fabric API ;
- Fantasy est déjà inclus dans le jar `classroomrun` : ne pas l'ajouter.

### Sécurité de l'endpoint HTTP

`GET /stats`, `GET /history` et `GET /history/<n>` n'ont **aucune authentification** : quiconque peut joindre le port lit les stats de la run, l'index des runs, et les noms de joueurs et causes de mort (déjà visibles en jeu).

- Dans `config/classroomrun/config.json`, mets `httpBind` sur l'interface LAN voulue (pas `0.0.0.0` si le serveur est exposé sur Internet), ou `httpPort` à `0` pour désactiver l'endpoint (l'historique des morts reste visible).

## Pour les mainteneurs

Chaque pack est un manifeste packwiz dans `packs/<id>/` (avec `opaline.toml`), source de vérité des mods pour Prism et pour Opaline (`side = "client" | "server" | "both"`). Ajouter un pack : voir [PACKS.md](PACKS.md). `packs/classroom-run` ne contient volontairement pas `classroomrun.pw.toml` : en dev local, Opaline prend le jar de `build/libs`. Après toute modification d'un `.pw.toml`, `python3 scripts/pack-tools.py index` régénère `index.toml` et le hash d'index de `pack.toml`.

### Publier une version

1. Mettre le même numéro `X.Y.Z` dans `version` de `gradle.properties`, `opaline/src-tauri/tauri.conf.json` et `opaline/src-tauri/Cargo.toml` (et `launcher/Cargo.toml`), puis commit. La CI ignore un fichier absent mais refuse un fichier dont la version diffère du tag.
2. Créer le tag et le pousser : `git tag vX.Y.Z` puis `git push origin vX.Y.Z`.
3. La CI (`release.yml`) construit Opaline (NSIS Windows, AppImage Linux), le mod, génère `packs/classroom-run/mods/classroomrun.pw.toml` (sha512 du jar, URL vers l'asset de release), puis crée la release : jar, installeurs Opaline, `pack-<id>.zip` par pack (dossier `pack/` à la racine), `packs.json`, `launcher.json`, `classroom-run-prism.zip`. Rien n'est commité sur `main` par la CI. La variable de dépôt `MS_CLIENT_ID` est lue au build.
