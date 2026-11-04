"""Génère structure/classroom.nbt (format structure Minecraft 26.3) + aperçus PNG.

Usage : python build_classroom.py            génère, vérifie, écrit structure/ (aperçus inclus)
        python build_classroom.py --install  idem + copie vers src/main/resources
        python build_classroom.py --check    vérifie seulement structure/classroom.nbt
Axes : x = est (0..14), y = haut (0..6), z = sud (0..11). Panneau de stats sur le mur nord (z=0),
porte sur le mur sud (z=11), cheminée sur le mur est (x=14). Origine (0,0,0) = coin nord-ouest du sol.
Style : salle à poutres vertes (terracotta/béton vert, mousse, feuillage d'azalée, bois clair).
"""
import shutil
import sys
from collections import deque
from pathlib import Path

import nbtlib
from nbtlib import Byte, Compound, Double, Float, Int, List, String
from PIL import Image, ImageChops, ImageDraw

HERE = Path(__file__).resolve().parent
OUT = HERE / "classroom.nbt"
DEST = HERE.parent / "src/main/resources/data/classroomrun/structure/classroom.nbt"
DATA_VERSION = 5023  # version.json "world_version" du client 26.3
W, H, D = 15, 7, 12
AIR = ("minecraft:air", ())
N_PLAYERS = 6
CHEST_POSITIONS = [(2, 10), (4, 10), (5, 10), (9, 10), (10, 10), (12, 10)]
SPAWN = (7, 1, 6)
PANEL_X = (5, 9)       # panneau de stats : x 5..9 (5 de large), y 2..4 (3 de haut), mur nord
BOARD_POS = (7.5, 2.35, 1.02)  # pied-centre du texte (le texte pousse vers le haut depuis pos)
POST_X = {0: (0, 14), 11: (0, 5, 10, 14)}  # poteaux des murs nord (z=0) / sud (z=11)
POST_Z = (0, 4, 8, 11)                      # poteaux des murs ouest / est
LANTERNS = [(3, 4, 4), (7, 4, 4), (10, 4, 4), (3, 4, 8), (7, 4, 8), (10, 4, 8), (4, 4, 1), (10, 4, 1),
            (1, 5, 6), (7, 5, 10), (3, 5, 10), (11, 5, 10), (1, 5, 1), (1, 5, 10)]
SIDES = {"north": (0, -1), "south": (0, 1), "east": (1, 0), "west": (-1, 0)}

grid = {}  # (x,y,z) -> (name, ((prop, value), ...))


def put(x, y, z, name, **props):
    grid[(x, y, z)] = ("minecraft:" + name, tuple(sorted((k, str(v).lower()) for k, v in props.items())))


def fill(x0, x1, y0, y1, z0, z1, name, **props):
    for x in range(x0, x1 + 1):
        for y in range(y0, y1 + 1):
            for z in range(z0, z1 + 1):
                put(x, y, z, name, **props)


def name_at(p):
    return grid.get(p, AIR)[0]


def props_at(p):
    return dict(grid[p][1])


def is_air(p):
    return name_at(p) == "minecraft:air"


def build():
    fill(0, W - 1, 0, H - 1, 0, D - 1, "air")
    # sol clair (chêne) avec bordure sapin ; fondation sombre, plafond sapin
    for x in range(W):
        for z in range(D):
            outer = x in (0, W - 1) or z in (0, D - 1)
            border = x in (1, W - 2) or z in (1, D - 2)
            put(x, 0, z, "dark_oak_planks" if outer else "spruce_planks" if border else "oak_planks")
            put(x, 6, z, "spruce_planks")
    # murs : soubassement bois sombre, terracotta verte, bandeau béton vert, lisse haute, poteaux
    for x in range(W):
        for z in range(D):
            if not (x in (0, W - 1) or z in (0, D - 1)):
                continue
            along = "x" if z in (0, D - 1) else "z"
            corner = x in (0, W - 1) and z in (0, D - 1)
            post = corner or (x in POST_X[z] if along == "x" else z in POST_Z)
            for y in range(1, 6):
                if corner:
                    put(x, y, z, "stripped_spruce_log", axis="y")
                elif post and y < 5:
                    put(x, y, z, "oak_log", axis="y")
                elif y == 5:
                    put(x, y, z, "stripped_oak_log", axis=along)
                elif y == 1:
                    put(x, y, z, "dark_oak_planks")
                elif (x * 7 + y * 13 + z * 5) % 8 == 0 and not post:
                    put(x, y, z, "moss_block")
                else:
                    put(x, y, z, "green_terracotta" if y < 4 else "green_concrete")
    # --- panneau de stats (mur nord) : dos sombre 5x3, cadre en rondins, tablette, consoles de lanternes ---
    x0, x1 = PANEL_X
    fill(x0, x1, 2, 4, 0, 0, "dark_oak_planks")
    for x in (x0 - 1, x1 + 1):
        fill(x, x, 2, 4, 0, 0, "stripped_oak_log", axis="y")
        put(x, 5, 1, "stripped_oak_log", axis="z")
    for x in range(x0, x1 + 1):
        put(x, 1, 1, "spruce_slab", type="bottom")
    # --- porte double sud + mosaïque au-dessus ---
    for x, hinge in ((7, "right"), (8, "left")):
        put(x, 1, D - 1, "oak_door", facing="south", half="lower", hinge=hinge, open="false")
        put(x, 2, D - 1, "oak_door", facing="south", half="upper", hinge=hinge, open="false")
    for x, a, b in ((6, "yellow", "white"), (7, "orange", "brown"), (8, "orange", "brown"), (9, "yellow", "white")):
        put(x, 3, D - 1, a + "_terracotta")
        put(x, 4, D - 1, b + "_terracotta")
    # --- cheminée est : tout en pierre, braises derrière un muret ---
    for x in (12, 13):
        for z in range(4, 9):
            for y in range(0, 6):
                h = (x * 5 + y * 3 + z * 7) % 6
                put(x, y, z, "stone_bricks" if h > 1 else "mossy_stone_bricks" if h == 1 else "cobblestone")
            put(x, 6, z, "stone_bricks")
    for z in range(4, 9):
        for y in range(1, 6):
            put(14, y, z, "stone_bricks")
        put(12, 0, z, "stone_bricks")
        put(13, 0, z, "stone_bricks")
    for z in (5, 6, 7):
        for y in (1, 2):
            put(13, y, z, "air")
        put(13, 1, z, "campfire", facing="west", lit="true", signal_fire="false")
        put(12, 2, z, "air")
        put(12, 1, z, "stone_brick_wall", up="true", north="low" if z > 5 else "none",
            south="low" if z < 7 else "none", east="none", west="none")
    # --- fenêtres (vitres reliées dans un second passage) ---
    for z in (1, 2, 3, 9, 10):
        for y in (2, 3):
            put(W - 1, y, z, "glass_pane")
    for z in (5, 6, 7, 9, 10):
        for y in (2, 3):
            put(0, y, z, "glass_pane")
    for x in (2, 3, 11, 12):
        for y in (2, 3):
            put(x, y, D - 1, "glass_pane")
    # --- salle à manger (ouest) sur tapis de sol en terracotta ---
    for x in range(1, 6):
        for z in range(2, 10):
            put(x, 0, z, "brown_terracotta" if x in (1, 5) or z in (2, 9) else "terracotta")
    fill(3, 3, 1, 1, 4, 7, "spruce_slab", type="top")
    for z in (3, 8):
        put(3, 1, z, "oak_fence")
    for z in range(4, 8):  # dossier = facing ; l'assis regarde l'opposé (vers la table)
        put(2, 1, z, "spruce_stairs", facing="west", half="bottom", shape="straight")
        put(4, 1, z, "spruce_stairs", facing="east", half="bottom", shape="straight")
    put(3, 2, 5, "candle", candles=3, lit="true")
    put(3, 2, 7, "candle", candles=2, lit="true")
    put(3, 2, 3, "potted_poppy")
    put(3, 2, 8, "potted_oxeye_daisy")
    put(3, 2, 4, "potted_fern")
    # --- coin salon (est, face au feu) : tapis blanc/gris, canapé en escaliers, accoudoirs en trappes ---
    for z in range(4, 9):
        for x in (10, 11):
            put(x, 1, z, "white_carpet" if x == 10 or z in (4, 8) else "gray_carpet")
    for z in (5, 6, 7):
        put(9, 1, z, "spruce_stairs", facing="west", half="bottom", shape="straight")
    put(9, 1, 4, "spruce_trapdoor", facing="north", half="bottom", open="true")
    put(9, 1, 8, "spruce_trapdoor", facing="south", half="bottom", open="true")
    put(10, 1, 6, "spruce_slab", type="top")
    put(10, 2, 6, "candle", candles=3, lit="true")
    # --- coffres de départ, tonneaux, poteries, mousse au pied du mur sud ---
    for x, z in CHEST_POSITIONS:
        put(x, 1, z, "chest", facing="north", type="single")
    for x in (3, 11):
        put(x, 1, D - 2, "barrel", facing="up", open="false")
    for x in (1, W - 2):
        put(x, 1, D - 2, "decorated_pot", facing="north")
    for x in (1, W - 2):
        put(x, 1, D - 3, "moss_carpet")
    # --- bibliothèque nord-ouest ---
    fill(1, 1, 1, 3, 1, 2, "bookshelf")
    # --- poutres transversales + lanternes suspendues ---
    for z in (4, 8):
        for x in range(1, 12):
            put(x, 5, z, "stripped_oak_log", axis="x")
    for p in LANTERNS:
        put(*p, "lantern", hanging="true")
    # --- volets (trappes ouvertes) de part et d'autre des fenêtres ---
    for y in (2, 3):
        for x in (1, 4, 10, 13):
            put(x, y, D - 2, "spruce_trapdoor", facing="north", half="bottom", open="true")
        for z in (4, 8):
            put(1, y, z, "spruce_trapdoor", facing="east", half="bottom", open="true")
    # --- étagères murales (dalles hautes) + plantes en pot ---
    pots = ["potted_azalea_bush", "potted_fern", "potted_flowering_azalea_bush", "potted_red_tulip"]
    for i, (x, z) in enumerate([(6, D - 2), (9, D - 2), (W - 2, 1), (W - 2, 3), (1, 9)]):
        if is_air((x, 3, z)) and is_air((x, 4, z)):
            put(x, 3, z, "spruce_slab", type="top")
            put(x, 4, z, pots[i % len(pots)])
    # --- feuillage retombant le long des murs (y=4) + lianes (y=3) ---
    for x in range(1, W - 1):
        for z in range(1, D - 1):
            ring = x in (1, W - 2) or z in (1, D - 2)
            if not ring or (x * 3 + z * 5) % 3 or not is_air((x, 4, z)):
                continue
            put(x, 4, z, "flowering_azalea_leaves" if (x + z) % 2 else "azalea_leaves", persistent="true")
            if is_air((x, 3, z)):
                sides = {d: "true" for d, (dx, dz) in SIDES.items() if name_at((x + dx, 3, z + dz)) in OPAQUE}
                if sides:
                    put(x, 3, z, "vine", **sides)
    # --- connexions des vitres ---
    for (x, y, z), (n, _) in list(grid.items()):
        if n != "minecraft:glass_pane":
            continue
        conn = {d: name_at((x + dx, y, z + dz)) in OPAQUE | {"minecraft:glass_pane"} for d, (dx, dz) in SIDES.items()}
        put(x, y, z, "glass_pane", **conn)


OPAQUE = {"minecraft:" + n for n in (
    "dark_oak_planks", "oak_planks", "spruce_planks", "green_terracotta", "green_concrete", "moss_block",
    "stripped_oak_log", "oak_log", "stripped_spruce_log", "stone_bricks", "mossy_stone_bricks", "cobblestone",
    "bookshelf", "yellow_terracotta", "white_terracotta", "orange_terracotta", "brown_terracotta", "terracotta")}
# marcheable (traversable à pied)
PASSABLE = {"minecraft:" + n for n in ("air", "oak_door", "lantern", "vine", "white_carpet", "gray_carpet", "moss_carpet")}
LIGHT_SRC = {"minecraft:lantern": 15, "minecraft:campfire": 15}
FLAMMABLE_KEYS = ("planks", "log", "leaves", "vine", "bookshelf", "carpet", "trapdoor", "fence", "spruce_", "oak_", "door")
DIRS6 = ((1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1))


def light_map():
    """Propagation de lumière de bloc approximative : -1 par pas, bloquée par les blocs opaques."""
    lv, q = {}, deque()
    for p, (n, _) in grid.items():
        if n in LIGHT_SRC:
            lv[p] = LIGHT_SRC[n]
            q.append(p)
    while q:
        p = q.popleft()
        for d in DIRS6:
            nb = (p[0] + d[0], p[1] + d[1], p[2] + d[2])
            if nb in grid and name_at(nb) not in OPAQUE and lv[p] - 1 > lv.get(nb, 0):
                lv[nb] = lv[p] - 1
                q.append(nb)
    return lv


def floor_cells():
    return [(x, z) for x in range(1, W - 1) for z in range(1, D - 1) if name_at((x, 1, z)) not in OPAQUE]


def sanity():
    errs = []
    lv = light_map()
    dark = [(x, z, lv.get((x, 1, z), 0)) for x, z in floor_cells() if lv.get((x, 1, z), 0) < 8]
    if dark:
        errs.append(f"lumière < 8 au sol : {dark}")
    for p in grid:  # enveloppe fermée : coque sans air ni liane
        x, y, z = p
        if (x in (0, W - 1) or y in (0, H - 1) or z in (0, D - 1)) and (is_air(p) or name_at(p) == "minecraft:vine"):
            errs.append(f"trou dans la coque en {p}")
    for (x, y, z), (n, _) in grid.items():  # blocs flottants
        if n == "minecraft:air" or not (1 <= x <= W - 2 and 2 <= y <= 5 and 1 <= z <= D - 2):
            continue
        below = not is_air((x, y - 1, z))
        above = not is_air((x, y + 1, z))
        side = any(not is_air((x + dx, y, z + dz)) for dx, dz in SIDES.values())
        if not (above if n == "minecraft:lantern" else below or side):
            errs.append(f"bloc flottant {n} en {(x, y, z)}")
    for p, (n, _) in grid.items():
        if n == "minecraft:lantern" and name_at((p[0], p[1] + 1, p[2])) not in OPAQUE:
            errs.append(f"lanterne non accrochée en {p}")
        if n == "minecraft:vine" and not any(name_at((p[0] + dx, p[1], p[2] + dz)) in OPAQUE for dx, dz in SIDES.values()):
            errs.append(f"liane sans support en {p}")
        if n == "minecraft:campfire":  # feu : aucun voisin inflammable
            for d in DIRS6:
                nn = name_at((p[0] + d[0], p[1] + d[1], p[2] + d[2]))
                if any(k in nn for k in FLAMMABLE_KEYS):
                    errs.append(f"feu au contact de {nn} en {p}")
    for x in range(PANEL_X[0], PANEL_X[1] + 1):  # devant le panneau, rien
        for y in (2, 3, 4):
            if not is_air((x, y, 1)):
                errs.append(f"panneau obstrué en {(x, y, 1)}")
    seen, q = {SPAWN}, deque([SPAWN])  # porte atteignable depuis le spawn (2 blocs de haut libres)
    while q:
        x, y, z = q.popleft()
        for dx, dz in SIDES.values():
            p = (x + dx, 1, z + dz)
            if p in seen or not (0 <= p[0] < W and 0 <= p[2] < D):
                continue
            if name_at(p) in PASSABLE and name_at((p[0], 2, p[2])) in PASSABLE:
                seen.add(p)
                q.append(p)
    if not any(p[2] == D - 1 and name_at(p) == "minecraft:oak_door" for p in seen):
        errs.append("porte non atteignable depuis le spawn")
    for x in (7, 8):
        if name_at((x, 1, D - 2)) not in PASSABLE or name_at((x, 2, D - 2)) not in PASSABLE:
            errs.append(f"devant la porte bloqué en {x}")
    if name_at(SPAWN) not in PASSABLE or name_at((SPAWN[0], 2, SPAWN[2])) not in PASSABLE:
        errs.append("spawn obstrué")
    for x, z in CHEST_POSITIONS:
        if (x, 1, z - 1) not in seen:
            errs.append(f"coffre {(x, z)} inaccessible")
    return errs, lv


def entities():
    board = Compound({
        "id": String("minecraft:text_display"),
        "Tags": List[String]([String("classroomrun:board")]),
        "text": Compound({"text": String("Classroom Run"), "color": String("white"), "bold": Byte(1)}),
        "billboard": String("fixed"),
        "alignment": String("center"),
        "line_width": Int(100),  # px de texte ; x scale 1.8 x 0.025 = 4.5 blocs de large max
        "background": Int(0),    # transparent : le dos sombre est le panneau en dark_oak_planks
        "Rotation": List[Float]([Float(0.0), Float(0.0)]),
        "transformation": Compound({
            "translation": List[Float]([Float(0), Float(0), Float(0)]),
            "left_rotation": List[Float]([Float(0), Float(0), Float(0), Float(1)]),
            "scale": List[Float]([Float(1.8), Float(1.8), Float(1.8)]),
            "right_rotation": List[Float]([Float(0), Float(0), Float(0), Float(1)]),
        }),
    })
    spawn = Compound({"id": String("minecraft:marker"), "Tags": List[String]([String("classroomrun:spawn")])})

    def ent(pos, nbt):
        return Compound({
            "pos": List[Double]([Double(v) for v in pos]),
            "blockPos": List[Int]([Int(int(v // 1)) for v in pos]),
            "nbt": nbt,
        })
    return [ent(BOARD_POS, board), ent((SPAWN[0] + .5, SPAWN[1], SPAWN[2] + .5), spawn)]


def write():
    pal, order = {}, []
    blocks = List[Compound]()
    for p in sorted(grid, key=lambda p: (p[1], p[2], p[0])):
        st = grid[p]
        if st not in pal:
            pal[st] = len(order)
            order.append(st)
        blocks.append(Compound({"pos": List[Int]([Int(c) for c in p]), "state": Int(pal[st])}))
    palette = List[Compound]()
    for n, props in order:
        c = Compound({"id": String(n)})
        if props:
            c["properties"] = Compound({k: String(v) for k, v in props})
        palette.append(c)
    root = Compound({
        "DataVersion": Int(DATA_VERSION),
        "size": List[Int]([Int(W), Int(H), Int(D)]),
        "palette": palette,
        "blocks": blocks,
        "entities": List[Compound](entities()),
    })
    nbtlib.File(root, gzipped=True).save(OUT)


def check(path=OUT):
    f = nbtlib.load(path)
    errs = []
    if f.get("DataVersion") != DATA_VERSION:
        errs.append(f"DataVersion {f.get('DataVersion')}")
    if list(f["size"]) != [W, H, D]:
        errs.append(f"size {list(f['size'])}")
    if len(f["blocks"]) != W * H * D:
        errs.append("nombre de blocs incohérent")
    names = [str(p["id"]) for p in f["palette"]]
    chest_ids = {i for i, n in enumerate(names) if n == "minecraft:chest"}
    chests = sum(1 for b in f["blocks"] if int(b["state"]) in chest_ids)
    if chests != len(CHEST_POSITIONS) or chests < N_PLAYERS:
        errs.append(f"coffres : {chests}, attendu {len(CHEST_POSITIONS)} (>= {N_PLAYERS})")
    ents = f["entities"]
    tags = lambda e: [str(t) for t in e["nbt"].get("Tags", [])]
    spawns = [e for e in ents if "classroomrun:spawn" in tags(e) and str(e["nbt"]["id"]) == "minecraft:marker"]
    boards = [e for e in ents if "classroomrun:board" in tags(e) and str(e["nbt"]["id"]) == "minecraft:text_display"]
    if len(ents) != 2:
        errs.append(f"{len(ents)} entités, attendu 2 (spawn + tableau)")
    if len(spawns) != 1:
        errs.append("marqueur de spawn absent/multiple")
    if len(boards) != 1:
        errs.append("text_display classroomrun:board absent/multiple")
    elif str(boards[0]["nbt"]["text"]["text"]) != "Classroom Run":
        errs.append("texte du tableau incorrect")
    return errs, chests


# ---------------------------------------------------------------- aperçus
COLORS = {
    "oak_planks": (196, 160, 100), "spruce_planks": (122, 90, 52), "dark_oak_planks": (70, 45, 22),
    "green_terracotta": (76, 90, 44), "green_concrete": (73, 91, 36), "moss_block": (90, 110, 40),
    "stripped_oak_log": (178, 144, 86), "oak_log": (110, 85, 50), "stripped_spruce_log": (115, 88, 52),
    "stone_bricks": (125, 125, 125), "mossy_stone_bricks": (110, 120, 95), "cobblestone": (110, 110, 110),
    "bookshelf": (150, 110, 60), "glass_pane": (170, 220, 240), "oak_door": (140, 100, 50),
    "spruce_slab": (122, 90, 52), "spruce_stairs": (140, 100, 58), "oak_fence": (100, 70, 35),
    "spruce_trapdoor": (105, 78, 45), "lantern": (255, 210, 80), "campfire": (240, 120, 30),
    "candle": (245, 235, 200), "chest": (200, 140, 30), "barrel": (140, 100, 55), "decorated_pot": (160, 70, 50),
    "azalea_leaves": (70, 130, 50), "flowering_azalea_leaves": (110, 140, 70), "vine": (50, 110, 40),
    "white_carpet": (235, 235, 235), "gray_carpet": (110, 115, 120), "moss_carpet": (90, 120, 40),
    "terracotta": (152, 94, 68), "brown_terracotta": (77, 51, 36), "yellow_terracotta": (186, 133, 35),
    "orange_terracotta": (161, 83, 37), "white_terracotta": (209, 178, 161), "stone_brick_wall": (130, 130, 130),
}
POT = [(200, 60, 60), (230, 120, 170), (120, 170, 90)]  # pots fleuris
ARROW = {"north": (0, -1), "south": (0, 1), "east": (1, 0), "west": (-1, 0)}
FLIP = {"north": "south", "south": "north", "east": "west", "west": "east"}


def shade(c, f):
    return tuple(min(255, int(v * f)) for v in c)


def boxes(n, pr):
    """Boîtes (x0,x1,y0,y1,z0,z1) locales à la cellule : approximation visuelle du bloc (orientation d'origine)."""
    if n == "oak_door":
        return [(0, 1, 0, 1, .8, 1)]
    if n.endswith("_slab"):
        return [(0, 1, .5, 1, 0, 1) if pr.get("type") == "top" else (0, 1, 0, .5, 0, 1)]
    if n.endswith("carpet"):
        return [(0, 1, 0, .06, 0, 1)]
    if n in ("campfire", "candle") or n.startswith("potted_"):
        return [(.2, .8, 0, .45, .2, .8)]
    if n == "decorated_pot":
        return [(.1, .9, 0, .85, .1, .9)]
    if n == "lantern":
        return [(.3, .7, .1, .6, .3, .7)]
    if n == "oak_fence":
        return [(.35, .65, 0, 1, .35, .65)]
    if n == "stone_brick_wall":
        return [(.25, .75, 0, 1, .25, .75)]
    if n == "spruce_stairs":
        back = {"north": (0, 1, 0, .5), "south": (0, 1, .5, 1), "west": (0, .5, 0, 1), "east": (.5, 1, 0, 1)}[pr["facing"]]
        return [(0, 1, 0, .5, 0, 1), (back[0], back[1], .5, 1, back[2], back[3])] if pr["facing"] in ("north", "south") \
            else [(0, 1, 0, .5, 0, 1), (back[0], back[1], .5, 1, 0, 1)]
    if n == "spruce_trapdoor":  # ouvert : panneau du côté opposé à facing
        return [{"north": (0, 1, 0, 1, .82, 1), "south": (0, 1, 0, 1, 0, .18),
                 "west": (.82, 1, 0, 1, 0, 1), "east": (0, .18, 0, 1, 0, 1)}[pr["facing"]]]
    if n == "vine":
        bx = []
        if pr.get("north") == "true":
            bx.append((0, 1, 0, 1, 0, .12))
        if pr.get("south") == "true":
            bx.append((0, 1, 0, 1, .88, 1))
        if pr.get("west") == "true":
            bx.append((0, .12, 0, 1, 0, 1))
        if pr.get("east") == "true":
            bx.append((.88, 1, 0, 1, 0, 1))
        return bx
    if n == "glass_pane":
        return [(.4, .6, 0, 1, 0, 1)] if pr.get("east") == "false" and pr.get("west") == "false" else [(0, 1, 0, 1, .4, .6)]
    return [(0, 1, 0, 1, 0, 1)]


def render(path, rot):
    """Vue isométrique. rot=False : murs sud/est coupés (on voit nord/ouest) ; rot=True : l'inverse."""
    S = 30
    bg = (238, 238, 232)
    img = Image.new("RGB", ((W + D) * S + 40, (W + D) * S // 2 + H * S + 40), bg)
    d = ImageDraw.Draw(img)
    ox, oy = D * S + 20, H * S + 20
    P = lambda x, y, z: (ox + (x - z) * S, oy + (x + z) * S / 2 - y * S)
    cells = []
    for (x, y, z), (n, pr) in grid.items():
        n = n.split(":")[1]
        if n == "air" or y == 6 or (y == 5 and n == "stripped_oak_log" and 1 <= x <= W - 2 and 1 <= z <= D - 2):
            continue  # plafond et poutres intérieures retirés (coupe)
        pr = dict(pr)
        for bx in boxes(n, pr):
            if rot:
                bx = (1 - bx[1], 1 - bx[0], bx[2], bx[3], 1 - bx[5], 1 - bx[4])
            cells.append((W - 1 - x if rot else x, y, D - 1 - z if rot else z, n, pr, bx))
    cells.sort(key=lambda c: (c[0] + c[2], c[1], c[0]))
    for x, y, z, n, pr, (u0, u1, v0, v1, w0, w1) in cells:
        if (x == W - 1 or z == D - 1) and y > 1 and n != "oak_door":
            continue  # murs proches coupés
        col = COLORS.get(n) or POT[(x + z) % 3] if n.startswith("potted_") else COLORS.get(n, (200, 0, 200))
        X0, X1, Y0, Y1, Z0, Z1 = x + u0, x + u1, y + v0, y + v1, z + w0, z + w1
        ol = (55, 55, 55)
        d.polygon([P(X0, Y1, Z0), P(X1, Y1, Z0), P(X1, Y1, Z1), P(X0, Y1, Z1)], fill=shade(col, 1.15), outline=ol)
        d.polygon([P(X1, Y1, Z0), P(X1, Y1, Z1), P(X1, Y0, Z1), P(X1, Y0, Z0)], fill=shade(col, .78), outline=ol)
        d.polygon([P(X0, Y1, Z1), P(X1, Y1, Z1), P(X1, Y0, Z1), P(X0, Y0, Z1)], fill=shade(col, .93), outline=ol)
        if n == "spruce_stairs" and v1 == .5:  # flèche = direction du regard de l'assis (opposée au dossier)
            f = FLIP[pr["facing"]]
            f = FLIP[f] if rot else f
            fx, fz = ARROW[f]
            f2 = FLIP[pr["facing"]]
            fx, fz = ARROW[FLIP[f2] if rot else f2]
            c = P(x + .5, y + .55, z + .5)
            d.line([c, P(x + .5 + fx * .45, y + .55, z + .5 + fz * .45)], fill=(255, 255, 255), width=3)
    if not rot:  # texte du panneau, illustratif (6 lignes)
        cx = (PANEL_X[0] + PANEL_X[1] + 1) / 2
        for i, s in enumerate(("Run 12", "Chrono 01:23:45", "Morts 7", "Best 04:12", "Top morts:", "A 3  B 2  C 2")):
            x, y = P(cx - 1.0, 4.7 - i * .42, 1.02)
            d.text((x, y), s, fill=(255, 255, 255))
    img.crop(ImageChops.difference(img, Image.new("RGB", img.size, bg)).getbbox()).save(path)


def preview():
    render(HERE / "preview_iso.png", False)
    render(HERE / "preview_iso_back.png", True)
    T = 40
    top = Image.new("RGB", (W * T, D * T), (255, 255, 255))
    t = ImageDraw.Draw(top)
    lv = light_map()
    for x in range(W):
        for z in range(D):
            col = (200, 0, 200)
            for y in range(5, -1, -1):
                nn = name_at((x, y, z)).split(":")[1]
                if COLORS.get(nn) and nn not in ("lantern", "vine") and not (y == 5 and nn.endswith("_log")) and y < 6:
                    col = COLORS[nn]
                    break
            if 1 <= x <= W - 2 and 1 <= z <= D - 2:
                col = shade(col, 0.55 + 0.45 * lv.get((x, 1, z), 0) / 15)
            t.rectangle([x * T, z * T, x * T + T - 1, z * T + T - 1], fill=col, outline=(70, 70, 70))
            if name_at((x, 1, z)) == "minecraft:spruce_stairs":
                fx, fz = ARROW[FLIP[props_at((x, 1, z))["facing"]]]
                c = (x * T + T // 2, z * T + T // 2)
                t.line([c, (c[0] + fx * 16, c[1] + fz * 16)], fill=(255, 255, 255), width=5)
    sx, sz = SPAWN[0] * T + T // 2, SPAWN[2] * T + T // 2
    t.ellipse([sx - 9, sz - 9, sx + 9, sz + 9], outline=(255, 0, 0), width=4)
    top.save(HERE / "preview_top.png")


if __name__ == "__main__":
    if "--check" not in sys.argv:
        build()
        errs, lv = sanity()
        if errs:
            sys.exit("ERREURS : " + "; ".join(errs))
        write()
        preview()
        if "--install" in sys.argv:
            DEST.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(OUT, DEST)
            print("copié vers", DEST)
        print("lumière min sol :", min(lv.get((x, 1, z), 0) for x, z in floor_cells()))
    errs, chests = check()
    if errs:
        sys.exit("CHECK ÉCHEC : " + "; ".join(errs))
    print(f"OK : {W}x{H}x{D}, DataVersion {DATA_VERSION}, {chests} coffres")
