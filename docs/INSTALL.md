# Installation Classroom Run

Les téléchargements sont sur la page **Releases** du dépôt.

## Joueur

1. Ouvrir la page **Releases** du dépôt et télécharger le launcher : `classroom-launcher-windows.exe` (Windows) ou `classroom-launcher-linux` (Linux).
2. Le lancer et se connecter avec son compte Microsoft (le compte doit posséder Minecraft Java).
3. Cliquer **Installer** (Minecraft 26.3, Fabric, mods, Sodium, Iris et shaders), puis **Jouer**.
4. Les mises à jour du pack sont appliquées par le launcher.

### Alternative : Prism Launcher

1. Télécharger `classroom-run-prism.zip` depuis la même page Releases (ne pas le décompresser).
2. Dans Prism Launcher : **Ajouter une instance** → **Importer** → choisir le zip → **OK**.
3. Lancer l'instance (Java 25 : Prism le propose automatiquement). À chaque nouvelle version : réimporter le nouveau zip et supprimer l'ancienne instance.

L'instance contient : Fabric API, le mod `classroomrun`, et côté client Sodium, Iris et le shaderpack Complementary Reimagined.

## Hôte du serveur

Serveur Fabric 26.3, loader 0.20.1, Java 25. Dans `mods/` :

- le jar `classroomrun-<version>.jar` (page Releases) ;
- les mods `side = "server"` ou `"both"` de `pack/mods/*.pw.toml` : Fabric API ;
- Fantasy est déjà inclus dans le jar `classroomrun` : ne pas l'ajouter.

### Sécurité de l'endpoint HTTP

`GET /stats` n'a **aucune authentification** : quiconque peut joindre le port lit les stats de la run.

- Dans `config/classroomrun/config.json`, mets `httpBind` sur l'interface LAN voulue (pas `0.0.0.0` si le serveur est exposé sur Internet), ou `httpPort` à `0` pour désactiver l'endpoint (l'historique des morts reste visible).

## Pour les mainteneurs

`pack/` est un manifeste packwiz, source de vérité des mods pour Prism et pour le launcher (`side = "client" | "server" | "both"`). Il ne contient volontairement pas `classroomrun.pw.toml` : en dev local, le launcher prend le jar de `build/libs`. Après toute modification d'un `.pw.toml`, `python3 scripts/pack-tools.py index` régénère `index.toml` et le hash d'index de `pack.toml`.

### Publier une version

1. Mettre le même numéro `X.Y.Z` dans `version` de `gradle.properties` et de `launcher/Cargo.toml`, puis commit.
2. Créer le tag et le pousser : `git tag vX.Y.Z` puis `git push origin vX.Y.Z`.
3. La CI (`release.yml`) vérifie version == tag, construit le mod et le launcher (Windows et Linux), génère `pack/mods/classroomrun.pw.toml` (sha512 du jar, URL vers l'asset de release du dépôt courant), puis crée la release avec le jar, les deux launchers, `classroom-run-prism.zip`, `pack.zip` (contenu de `pack/` publié, avec `classroomrun.pw.toml` et l'index recalculé) et `launcher.json`. Rien n'est commité sur `main` par la CI.
