# Installation Classroom Run

Le dépôt est privé : il faut y avoir accès.

## Joueur

1. Accepter l'invitation de collaborateur au dépôt GitHub (e-mail ou https://github.com/notifications).
2. Ouvrir la page **Releases** du dépôt et télécharger `classroom-run-prism.zip` de la dernière version (ne pas le décompresser).
3. Dans Prism Launcher : **Ajouter une instance** → **Importer** → choisir le zip → **OK**.
4. Lancer l'instance (Minecraft 26.3, Fabric Loader 0.20.1, Java 25 : Prism le propose automatiquement).
5. À chaque nouvelle version : télécharger le nouveau zip et le réimporter (nouvelle instance ; supprimer l'ancienne).

L'instance contient : Fabric API, ReplayMod et le mod `classroomrun`.

## Hôte du serveur

Serveur Fabric 26.3, loader 0.20.1, Java 25. Dans `mods/` :

- le jar `classroomrun-<version>.jar` (page Releases) ;
- les mods `side = "server"` ou `"both"` de `pack/mods/*.pw.toml` : Fabric API, ServerReplay, Fabric Language Kotlin (dépendance requise de ServerReplay) ;
- Fantasy est déjà inclus dans le jar `classroomrun` : ne pas l'ajouter.

Ne pas installer ReplayMod côté serveur (client uniquement).

## Pour les mainteneurs

`pack/` est un manifeste packwiz (source de vérité des mods). Le jar `classroomrun` n'est pas sur Modrinth : la CI l'ajoute au zip Prism (`scripts/build-prism-instance.py`). Tag `v*` → release GitHub privée avec le jar et le zip.
