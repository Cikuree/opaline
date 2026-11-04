# Structure classroom

Génération : `python -m venv .venv && .venv/Scripts/pip install -r requirements.txt && .venv/Scripts/python build_classroom.py`
(`--check` : relit et vérifie `classroom.nbt` ; `--install` : copie aussi vers
`src/main/resources/data/classroomrun/structure/classroom.nbt`, sinon n'écrit que dans `structure/`).
Aperçus : `preview_iso.png` (murs nord/ouest visibles), `preview_iso_back.png` (murs sud/est), `preview_top.png`
(vue de dessus, teinte = lumière, cercle rouge = spawn, flèche blanche = regard de l'assis).

Format : structure Minecraft 26.3 (gzip, DataVersion 5023, size 15x7x12, `palette`/`blocks`/`entities`).
Style : salle à poutres vertes (terracotta/béton vert, mousse, azalée), bois clair, cheminée en pierre.
Axes : x = est, z = sud, y = haut ; panneau de stats sur le mur nord (z=0, x=5..9, y=2..4), porte double sud
(z=11, x=7..8), cheminée est (x=12..14, z=4..8).

## Marqueurs lus par le serveur (entités du template, `entities[].nbt`) — seules entités du template
- **Spawn** : `minecraft:marker`, `Tags: ["classroomrun:spawn"]`, `pos` = (7.5, 1.0, 6.5).
- **Tableau** : `minecraft:text_display`, `Tags: ["classroomrun:board"]`, `text` = composant NBT
  (`{text:"Classroom Run",...}`, remplaçable), `billboard:"fixed"`, `scale` 1.8, `line_width` 100, `background` 0,
  `pos` = (7.5, 2.35, 1.02) : le texte pousse vers le haut depuis `pos` (6 lignes ≈ 2.4 blocs, ≤ 4.5 blocs de large).
  Dos = panneau `dark_oak_planks` 5x3, lisible depuis le spawn (7,1,6).
- **Coffres de départ** : 6 `minecraft:chest` (facing=north) en (2,1,10) (4,1,10) (5,1,10) (9,1,10) (10,1,10) (12,1,10).
