# Structure classroom

Génération : `python -m venv .venv && .venv/Scripts/pip install -r requirements.txt && .venv/Scripts/python build_classroom.py`
(`--check` : relit et vérifie `classroom.nbt`). Écrit `structure/classroom.nbt` et le copie vers
`src/main/resources/data/classroomrun/structure/classroom.nbt`. Aperçus : `preview_iso.png`, `preview_top.png`.

Format : structure Minecraft 26.3 (gzip, DataVersion 5023, size 15x7x12, `palette`/`blocks`/`entities`).
Axes : x = est, z = sud, y = haut ; tableau sur le mur nord (z=0), porte double sur le mur sud (z=11, x=7..8).

## Marqueurs lus par le serveur (entités du template, `entities[].nbt`)
- **Spawn** : entité `minecraft:marker` avec `Tags: ["classroomrun:spawn"]`, `pos` = (7.5, 1.0, 6.5) relatif au coin du template.
  Choisi plutôt qu'un structure_block DATA : pas de bloc résiduel, et l'entité est taggée une fois posée.
- **Tableau** : entité `minecraft:text_display`, `Tags: ["classroomrun:board"]`, champ de texte `text` = composant
  NBT `{text:"Classroom Run", color:"white", bold:1b}` (ComponentSerialization.CODEC), `billboard:"fixed"`,
  `transformation.scale = [4f,4f,4f]`, `pos` = (7.5, 2.9, 1.02), `Rotation [0f,0f]` (lisible depuis le sud).
  Pour changer le texte : remplacer la clé `text`.
- **Coffres de départ** : 6 blocs `minecraft:chest` (facing=north, type=single) en (2,1,10) (4,1,10) (5,1,10) (9,1,10) (10,1,10) (12,1,10), vides.
- **Horloge** : `minecraft:item_frame` (`Facing:3b`, `Item:{id:"minecraft:clock",count:1}`, `block_pos:[13,4,1]`).
