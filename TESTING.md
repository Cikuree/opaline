# Checklist de test en jeu

Coche chaque ligne et note ce qui ne va pas (capture d'écran bienvenue). Les numéros servent à me répondre (« 2.3 KO : … »).

## 0. Mise en place

Tester en multijoueur sur ta machine : un serveur et un ou plusieurs clients.

1. Compile le jar :

   ```bash
   ./gradlew build
   ```

   Il est produit dans `build/libs/classroomrun-0.1.0.jar` (n'utilise pas le `-sources`).
2. Lance le serveur de dev. Il démarre dans `run/`, avec Fabric API et Fantasy fournis automatiquement :

   ```bash
   ./gradlew runServer
   ```

   Pour ServerReplay, rien à faire à cette étape : il sert à l'étape 2.
3. Lance le client, au choix :
   - `./gradlew runClient`, qui ouvre un client de dev au pseudo aléatoire ;
   - une instance Prism 26.3 + Fabric 0.20.1 + Fabric API, avec le jar copié dans `mods/`.
4. Connecte le client au serveur : `localhost`.
5. Mets-toi opérateur depuis la console serveur : `op <pseudo>`.
6. Pour tester à plusieurs joueurs : deux clients, ou des amis connectés sur ton réseau.

## 1. HUD seul (solo, sans serveur)

Dans un monde solo avec le mod :

- [ ] 1.1 `/crdebug state` : la barre des splits apparaît **en haut à gauche**, avec 9 icônes.
  - Les 4 premières sont en couleur.
  - La 5ᵉ (verrue du Nether) est encadrée en rose avec une petite flèche.
  - Les 4 dernières sont grisées.
  - Le chrono cyan part de 2:14:00 et avance.
  - Le crâne affiche 7.
- [ ] 1.2 `/crdebug clear` : la barre disparaît.
- [ ] 1.3 `/crdebug damage 1` : ta tête avec « -1 » rouge au-dessus des cœurs, qui monte un peu et s'efface en environ 1,5 s. `/crdebug damage 0.5` affiche « -0.5 ».
- [ ] 1.4 Deux `/crdebug damage 1` en moins de 0,5 s : une seule entrée « -2 ». Plus lentement : deux têtes côte à côte.
- [ ] 1.5 `/crdebug damage 1 Truc` : tête par défaut, celle d'un joueur absent de la liste Tab.
- [ ] 1.6 `/crdebug heal 1` : « +1 » vert.
- [ ] 1.7 `/crdebug burst` : 3 têtes côte à côte, « -2 », « -2 », « -0.5 ».
- [ ] 1.8 Le feed ne chevauche ni les cœurs, ni l'armure, ni la barre de faim. Teste aussi avec de l'absorption (pomme dorée) et de l'armure.
- [ ] 1.9 La barre des splits ne gêne ni le chat, ni la liste Tab, ni l'écran F3.

## 2. Salle de classe et reset (serveur)

- [ ] 2.1 À la connexion, tu apparais dans la salle de classe, au centre, face au tableau.
- [ ] 2.2 Le tableau affiche « Run #N — M morts — meilleur split : … », lisible depuis les pupitres.
- [ ] 2.3 La salle est complète :
  - parquet, fenêtres, porte double qui s'ouvre normalement vers l'extérieur ;
  - 6 pupitres avec chaises tournées vers le tableau ;
  - bureau du prof, horloge, étagères, 6 coffres.
- [ ] 2.4 Aucune zone sombre : pas de monstre qui apparaît dans la salle la nuit.
- [ ] 2.5 La salle est posée sur le terrain, ni enterrée ni flottante, et la sortie donne sur l'extérieur.
- [ ] 2.6 `/run reset` : nouveau monde (paysage différent), tout le monde est téléporté dans une nouvelle salle. Inventaire, XP, effets et advancements sont vidés.
- [ ] 2.7 `/run stats` affiche le numéro de run, le chrono, les morts et le prochain split.
- [ ] 2.8 Navigateur sur `http://localhost:25580/stats` : du JSON avec les stats.

## 3. Démarrage et chrono

- [ ] 3.1 Avant `/run start` : le chrono est à l'arrêt et tu ne prends aucun dégât (chute, mob).
- [ ] 3.2 `/run start` : le chrono démarre (HUD) et les dégâts redeviennent actifs.
- [ ] 3.3 Le chrono continue entre deux runs (cumulé) et survit à un redémarrage du serveur. Au redémarrage, une nouvelle run commence.

## 4. Vie partagée (2 joueurs minimum)

- [ ] 4.1 A prend un coup : la vie de B baisse d'autant. Tout le monde voit la tête de A avec « -x ».
- [ ] 4.2 A et B touchés au même moment, par exemple par la même explosion : la perte n'est pas doublée.
- [ ] 4.3 Faim : elle descend à la même vitesse qu'en solo, pas plus vite à plusieurs. La saturation est partagée.
- [ ] 4.4 Régénération naturelle : elle remonte à la même vitesse qu'en solo, sans « +1 » affiché.
- [ ] 4.5 Potion de soin instantané, ou pomme dorée : « +x » vert pour celui qui la prend, et toute l'équipe est soignée.
- [ ] 4.6 Absorption (pomme dorée) partagée par tous.
- [ ] 4.7 Un joueur passe de créatif à survie : l'équipe n'est **pas** soignée, et il prend la vie commune.
- [ ] 4.8 Totem tenu par la victime d'un coup fatal :
  - le totem est consommé ;
  - il n'y a pas de reset ;
  - toute l'équipe passe à 1 cœur.
- [ ] 4.9 Totem tenu seulement par un coéquipier de la victime : mort collective.

## 5. Mort collective

- [ ] 5.1 Un joueur meurt :
  - pas d'écran de mort, pas de drop ;
  - environ 3 s de pause ;
  - toute l'équipe est téléportée dans une nouvelle salle (Run #N+1) ;
  - le compteur de morts augmente de 1, au HUD et sur le tableau.
- [ ] 5.2 Mort pendant que des diamants sont posés dans la grille de craft : rien n'est conservé dans la nouvelle run.
- [ ] 5.3 Un joueur déconnecté pendant le reset, qui se reconnecte ensuite : il arrive dans la nouvelle salle, l'inventaire vide.

## 6. Splits et dimensions

- [ ] 6.1 Les splits s'allument dans l'ordre :
  - pioche (miner de la pierre) ;
  - lingot de fer (en cuire un) ;
  - diamant (en miner un).
- [ ] 6.2 **Allumer un portail du Nether au briquet** : il s'allume, et le split briquet valide à l'entrée dans le Nether.
- [ ] 6.3 Le portail mène au Nether **de la run**, et le retour ramène dans l'overworld de la run, pas dans le monde de départ.
- [ ] 6.4 Verrue du Nether (forteresse), bâton de blaze, œil de l'Ender (le lancer) : splits validés.
- [ ] 6.5 Portail de l'End : on arrive dans l'End de la run, avec le dragon et sa barre de vie. Le panneau des splits ne cache pas la barre du dragon.
- [ ] 6.6 Tuer le dragon : le chrono s'arrête, même si un split a été sauté (par exemple pas de diamant miné).
- [ ] 6.7 Sauter dans le portail de sortie : retour dans la salle de classe.
- [ ] 6.8 `/run reset` pendant le combat contre le dragon : la barre du dragon disparaît bien de l'écran.

## 7. Journal et historique des morts (étape 2)

Serveur de dev : `./gradlew runServer` charge ServerReplay automatiquement. Côté client, il faut ReplayMod pour la section 8 : l'instance Prism ou le launcher l'installent.

- [ ] 7.1 Après `/run start` puis quelques coups reçus et une mort, le fichier `run/config/classroomrun/runs/<n>/journal.jsonl` existe et contient une ligne par événement.
- [ ] 7.2 La touche **H** (ou `/crhistory`) ouvre l'écran « Historique des morts » avec la liste des runs, de la plus récente à la plus ancienne.
- [ ] 7.3 Clic sur une run : ses morts s'affichent. Clic sur une mort : cause, message, attaquant, position, dimension, « coup fatal par » et graphe des vies sur les 10 s précédentes.
- [ ] 7.4 Arrête le serveur sauvagement (fermer la fenêtre ou tuer le processus) en pleine run, puis relance-le : l'historique de cette run est toujours lisible.
- [ ] 7.5 La touche H se reconfigure dans Options, Commandes, catégorie « Classroom Run ».

## 8. Replay

- [ ] 8.1 Un enregistrement par joueur et par run apparaît dans `run/recordings/classroomrun/run_<n>/<uuid>/`. Le fichier `.mcpr` est finalisé après la fin de la run ou ta déconnexion.
- [ ] 8.2 Dans l'historique, le bouton « Voir le replay » d'une mort :
  - une confirmation s'affiche ;
  - le client se déconnecte ;
  - ReplayMod s'ouvre sur l'enregistrement de la victime, quelques secondes avant sa mort.
- [ ] 8.3 Un marqueur « Mort : … » est visible sur la timeline de ReplayMod, et les splits aussi.
- [ ] 8.4 Clic sur « Voir le replay » juste après la mort, pendant la sauvegarde : le message « en cours de sauvegarde » s'affiche, et ça fonctionne quelques secondes plus tard.
- [ ] 8.5 Sans ReplayMod côté client : l'écran d'historique fonctionne, mais le bouton est remplacé par un texte explicatif.
- [ ] 8.6 Un navigateur sur `http://localhost:25580/replay?path=../../server.properties` renvoie une erreur 404.

## 9. Launcher Rust

1. Compile le launcher :

   ```bash
   cd launcher && cargo build --release
   ```

2. Lance `launcher/target/release/classroom-launcher.exe` (voir `launcher/README.md` pour les options).

- [ ] 9.1 La fenêtre s'ouvre avec les champs pseudo, serveur et chemin du pack (le dossier `pack/` du dépôt).
- [ ] 9.2 « Installer » télécharge Java 25, Minecraft 26.3, Fabric et les mods dans `%APPDATA%\ClassroomRunLauncher\`, avec une barre de progression. Ton `.minecraft` n'est pas touché.
- [ ] 9.3 Coupe le réseau pendant l'installation, puis réessaie : l'installation reprend sans tout retélécharger.
- [ ] 9.4 « Jouer » lance le jeu en hors ligne et se connecte directement au serveur indiqué.
- [ ] 9.5 Le panneau des stats affiche la run en cours depuis le serveur (port 25580). Avec le serveur éteint, une erreur s'affiche sans faire planter le launcher.
- [ ] 9.6 Pendant une partie, le bouton « Installer » est désactivé.
- [ ] 9.7 Si l'installation est interrompue, « Jouer » refuse de lancer le jeu et demande de réinstaller.

## Pour me répondre

Indique les numéros KO, avec ce que tu as vu. Les logs serveur sont dans `run/logs/latest.log`, et les logs client dans `.minecraft/logs/latest.log` de l'instance.
