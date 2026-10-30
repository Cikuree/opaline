# Changelog

## Étape 1 — MVP (0.1.0)
- Mod Fabric `classroomrun` (Minecraft 26.3, Fabric Loader 0.20.1, Fabric API 0.165.0+26.3, Fantasy 0.8.4+26.3 inclus).
- Contrat réseau serveur → client figé (`CONTRACT.md`) : `RunStatePayload`, `DamagePayload`, `HealPayload`, `DeathPayload`, splits.
- Serveur :
  - vie, absorption et faim partagées ; faim et régénération naturelle portées par un seul joueur ;
  - totem de la victime qui sauve l'équipe ;
  - mort collective sans écran de mort, puis reset après un court délai ;
  - nouveau trio de mondes Fantasy (Overworld, Nether, End) à chaque run, portails Nether et End redirigés par mixins, ancien trio supprimé ;
  - salle de classe collée au spawn (config, puis jar, puis plateforme de secours) ; tableau noir dynamique ;
  - 9 splits (advancements, dimension ou inventaire), chrono cumulé, morts, stats JSON (écriture atomique, reprise sur fichier corrompu) ;
  - `/run start`, `/run reset`, `/run stats` ; endpoint `GET /stats` (port 25580).
- Client :
  - HUD des splits en haut à gauche, avec chrono et morts ;
  - feed des dégâts et soins avec têtes de joueurs au-dessus de la vie ;
  - commande `/crdebug` pour tester le HUD en solo.
- Salle de classe générée par `structure/build_classroom.py` (15×7×12, DataVersion 5023), avec aperçus PNG.
- Modpack packwiz (`pack/`), script d'instance Prism, CI GitHub Actions (build sur chaque push, release privée sur tag `v*`), `docs/INSTALL.md`.

## Étape 0
- Initialisation du dépôt et du `.gitignore`.
