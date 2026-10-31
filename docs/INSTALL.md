# Installation Classroom Run

Le dépôt est privé : il faut y avoir accès.

## Joueur

1. Accepter l'invitation de collaborateur au dépôt GitHub (e-mail ou https://github.com/notifications).
2. Ouvrir la page **Releases** du dépôt et télécharger `classroom-run-prism.zip` de la dernière version (ne pas le décompresser).
3. Dans Prism Launcher : **Ajouter une instance** → **Importer** → choisir le zip → **OK**.
4. Lancer l'instance (Minecraft 26.3, Fabric Loader 0.20.1, Java 25 : Prism le propose automatiquement).
5. À chaque nouvelle version : télécharger le nouveau zip et le réimporter (nouvelle instance ; supprimer l'ancienne).

L'instance contient : Fabric API et le mod `classroomrun`.

## Hôte du serveur

Serveur Fabric 26.3, loader 0.20.1, Java 25. Dans `mods/` :

- le jar `classroomrun-<version>.jar` (page Releases) ;
- les mods `side = "server"` ou `"both"` de `pack/mods/*.pw.toml` : Fabric API ;
- Fantasy est déjà inclus dans le jar `classroomrun` : ne pas l'ajouter.

### Sécurité de l'endpoint HTTP

`GET /stats` n'a **aucune authentification** : quiconque peut joindre le port lit les stats de la run.

- Dans `config/classroomrun/config.json`, mets `httpBind` sur l'interface LAN voulue (pas `0.0.0.0` si le serveur est exposé sur Internet), ou `httpPort` à `0` pour désactiver l'endpoint (l'historique des morts reste visible).

## Pour les mainteneurs

`pack/` est un manifeste packwiz (source de vérité des mods). Le jar `classroomrun` n'est pas sur Modrinth : la CI l'ajoute au zip Prism (`scripts/build-prism-instance.py`). Tag `v*` → release GitHub privée avec le jar et le zip.
