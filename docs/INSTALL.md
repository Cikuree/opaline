# Installation Classroom Run

Dépôt public de distribution : https://github.com/Cikuree/classroom-run-dist (aucun code source).

## Joueur

1. Ouvrir la page **Releases** de https://github.com/Cikuree/classroom-run-dist et télécharger le launcher : `classroom-launcher-windows.exe` (Windows) ou `classroom-launcher-linux` (Linux).
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

### Créer le secret DIST_TOKEN (une fois)

1. Se connecter à GitHub avec le compte `Cikuree`.
2. **Settings** du compte → **Developer settings** → **Personal access tokens** → **Fine-grained tokens** → **Generate new token**.
3. **Repository access** : **Only select repositories** → `classroom-run-dist`.
4. **Permissions** → **Repository permissions** → **Contents** : **Read and write**.
5. Générer le jeton et le copier.
6. Depuis un compte ayant accès au dépôt privé : `gh secret set DIST_TOKEN -R OddNode-design/classroom-run`, puis coller le jeton à l'invite interactive. Ne jamais le passer en argument de ligne de commande.

### Publier une version

1. Mettre le même numéro `X.Y.Z` dans `version` de `gradle.properties` et de `launcher/Cargo.toml`, puis commit.
2. Créer le tag et le pousser : `git tag vX.Y.Z` puis `git push origin vX.Y.Z`.
3. La CI (`release.yml`) vérifie version == tag, construit le mod et le launcher (Windows et Linux), génère `pack/mods/classroomrun.pw.toml` (sha512 du jar, URL vers l'asset public), crée la release privée, puis la release publique `vX.Y.Z` et le commit sur `main` de `Cikuree/classroom-run-dist` (`pack/`, `launcher.json`, `CREDITS.md`).
4. Si `DIST_TOKEN` est absent, la release privée est faite et l'étape publique échoue avec un message explicite : créer le secret, puis relancer le job.
