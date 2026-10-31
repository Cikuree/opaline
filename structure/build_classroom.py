"""Génère structure/classroom.nbt (format structure Minecraft 26.3) + aperçus PNG.

Usage : python build_classroom.py            génère, vérifie, copie vers src/main/resources
        python build_classroom.py --check    vérifie seulement structure/classroom.nbt
Axes : x = est (0..14), y = haut (0..6), z = sud (0..11). Tableau sur le mur nord (z=0),
porte sur le mur sud (z=11). Origine (0,0,0) = coin nord-ouest du sol.
"""
import shutil
import sys
from collections import deque
from pathlib import Path

import nbtlib
from nbtlib import Byte, Compound, Double, Float, Int, IntArray, List, String
from PIL import Image, ImageDraw

HERE = Path(__file__).resolve().parent
OUT = HERE / "classroom.nbt"
DEST = HERE.parent / "src/main/resources/data/classroomrun/structure/classroom.nbt"
DATA_VERSION = 5023  # version.json "world_version" du client 26.3
W, H, D = 15, 7, 12
AIR = ("minecraft:air", ())
N_PLAYERS = 6
CHEST_POSITIONS = [(2, 10), (4, 10), (5, 10), (9, 10), (10, 10), (12, 10)]
SPAWN = (7, 1, 6)

grid = {}  # (x,y,z) -> (name, ((prop, value), ...))


def put(x, y, z, name, **props):
    grid[(x, y, z)] = ("minecraft:" + name, tuple(sorted((k, str(v).lower()) for k, v in props.items())))


def name_at(p):
    return grid.get(p, AIR)[0]


def build():
    for x in range(W):
        for y in range(H):
            for z in range(D):
                put(x, y, z, "air")
    # sol parquet (alternance chêne / sapin), plafond, murs
    for x in range(W):
        for z in range(D):
            put(x, 0, z, "oak_planks" if (x // 2 + z) % 2 == 0 else "spruce_planks")
            put(x, 6, z, "smooth_stone")
    for y in range(1, 6):
        for x in range(W):
            put(x, y, 0, "white_concrete")
            put(x, y, D - 1, "white_concrete")
        for z in range(D):
            put(0, y, z, "white_concrete")
            put(W - 1, y, z, "white_concrete")
        for x, z in [(0, 0), (W - 1, 0), (0, D - 1), (W - 1, D - 1)]:
            put(x, y, z, "stripped_spruce_log", axis="y")
    # fenêtres (vitres reliées entre elles et aux murs dans un second passage)
    for x in (0, W - 1):
        for z in (5, 6, 8, 9):
            for y in (2, 3):
                put(x, y, z, "glass_pane")
    for x in (3, 4, 10, 11):
        for y in (2, 3):
            put(x, y, D - 1, "glass_pane")
    # porte double sud, bandeau rouge "sortie" au-dessus
    for x, hinge in ((7, "right"), (8, "left")):
        put(x, 1, D - 1, "oak_door", facing="south", half="lower", hinge=hinge, open="false")
        put(x, 2, D - 1, "oak_door", facing="south", half="upper", hinge=hinge, open="false")
    for x in range(6, 10):
        put(x, 3, D - 1, "red_concrete")
    # tableau noir (mur nord), cadre en chêne, tablette de craie
    for x in range(2, 13):
        for y in (2, 3, 4):
            put(x, y, 0, "black_concrete")
        put(x, 1, 0, "oak_planks")
        put(x, 5, 0, "oak_planks")
        put(x, 1, 1, "oak_slab", type="bottom")
    for y in (2, 3, 4):
        put(1, y, 0, "oak_planks")
        put(13, y, 0, "oak_planks")
    # bureau du prof : pieds en barrière, plateau en dalles hautes, chaise face aux élèves
    put(5, 1, 3, "oak_fence")
    put(9, 1, 3, "oak_fence")
    for x in (6, 7, 8):
        put(x, 1, 3, "oak_slab", type="top")
    put(7, 1, 2, "oak_stairs", facing="south", half="bottom", shape="straight")
    put(10, 1, 2, "lectern", facing="south")
    # pupitres : barrière - dalle haute - barrière ; chaise = escalier regardant le tableau (nord)
    for cx in (3, 11):
        for dz in (4, 6, 8):
            put(cx - 1, 1, dz, "oak_fence")
            put(cx + 1, 1, dz, "oak_fence")
            put(cx, 1, dz, "oak_slab", type="top")
            put(cx, 1, dz + 1, "oak_stairs", facing="north", half="bottom", shape="straight")
    # coffres de départ (devant le mur sud, ouverts vers le nord)
    for x, z in CHEST_POSITIONS:
        put(x, 1, z, "chest", facing="north", type="single")
    # étagères
    for x in (1, 13):
        for z in (2, 3):
            for y in (1, 2, 3):
                put(x, y, z, "bookshelf")
    for x in (1, 13):
        for y in (1, 2, 3):
            put(x, y, D - 2, "bookshelf")
    # lanternes suspendues au plafond
    for x in (3, 7, 11):
        for z in (2, 6, 9):
            put(x, 5, z, "lantern", hanging="true")
    # connexions des vitres
    for (x, y, z), (n, _) in list(grid.items()):
        if n != "minecraft:glass_pane":
            continue
        conn = {}
        for d, (dx, dz) in {"north": (0, -1), "south": (0, 1), "east": (1, 0), "west": (-1, 0)}.items():
            nb = name_at((x + dx, y, z + dz))
            conn[d] = nb == "minecraft:glass_pane" or nb in OPAQUE
        put(x, y, z, "glass_pane", **conn)


OPAQUE = {"minecraft:" + n for n in (
    "white_concrete", "oak_planks", "spruce_planks", "smooth_stone", "black_concrete",
    "red_concrete", "bookshelf", "stripped_spruce_log")}
# marcheable (traversable à pied) : air, portes, lanterne
PASSABLE = {"minecraft:air", "minecraft:oak_door", "minecraft:lantern"}


def light_map():
    """Propagation de lumière de bloc approximative : 15 aux lanternes, -1 par pas, bloquée par les blocs opaques."""
    lv = {}
    q = deque()
    for p, (n, _) in grid.items():
        if n == "minecraft:lantern":
            lv[p] = 15
            q.append(p)
    while q:
        p = q.popleft()
        for d in ((1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)):
            nb = (p[0] + d[0], p[1] + d[1], p[2] + d[2])
            if nb in grid and name_at(nb) not in OPAQUE and lv[p] - 1 > lv.get(nb, 0):
                lv[nb] = lv[p] - 1
                q.append(nb)
    return lv


def sanity():
    errs = []
    lv = light_map()
    dark = [(x, z) for x in range(1, W - 1) for z in range(1, D - 1)
            if name_at((x, 1, z)) not in OPAQUE and lv.get((x, 1, z), 0) < 1]
    if dark:
        errs.append(f"zones sombres (lumière < 1) : {dark}")
    # blocs flottants (hors murs/plafond) : y>=2 intérieur doit reposer sur un bloc non-air
    for (x, y, z), (n, _) in grid.items():
        if n == "minecraft:air" or not (1 <= x <= W - 2 and 2 <= y <= 5 and 1 <= z <= D - 2):
            continue
        below = name_at((x, y - 1, z)) != "minecraft:air"
        above = name_at((x, y + 1, z)) != "minecraft:air"
        side = any(name_at((x + dx, y, z + dz)) != "minecraft:air" for dx, dz in ((1, 0), (-1, 0), (0, 1), (0, -1)))
        if not (above if n == "minecraft:lantern" else below or side):
            errs.append(f"bloc flottant {n} en {(x, y, z)}")
    # porte atteignable depuis le spawn (2 blocs de haut libres), sortie hors de la salle (z = D)
    seen, q = {SPAWN}, deque([SPAWN])
    while q:
        x, y, z = q.popleft()
        for dx, dz in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            p = (x + dx, 1, z + dz)
            if p in seen or not (0 <= p[0] < W and 0 <= p[2] < D):
                continue
            if name_at(p) in PASSABLE and name_at((p[0], 2, p[2])) in PASSABLE:
                seen.add(p)
                q.append(p)
    if not any(p[2] == D - 1 and name_at(p) == "minecraft:oak_door" for p in seen):
        errs.append("porte non atteignable depuis le spawn")
    # chaque coffre : cellule voisine nord libre (accès)
    for x, z in CHEST_POSITIONS:
        if name_at((x, 1, z - 1)) not in PASSABLE:
            errs.append(f"coffre {(x, z)} inaccessible")
    return errs, lv


def entities():
    board = Compound({
        "id": String("minecraft:text_display"),
        "Tags": List[String]([String("classroomrun:board")]),
        "text": Compound({"text": String("Classroom Run"), "color": String("white"), "bold": Byte(1)}),
        "billboard": String("fixed"),
        "alignment": String("center"),
        "line_width": Int(200),
        "background": Int(0),
        "Rotation": List[Float]([Float(0.0), Float(0.0)]),
        "transformation": Compound({
            "translation": List[Float]([Float(0), Float(0), Float(0)]),
            "left_rotation": List[Float]([Float(0), Float(0), Float(0), Float(1)]),
            "scale": List[Float]([Float(2), Float(2), Float(2)]),
            "right_rotation": List[Float]([Float(0), Float(0), Float(0), Float(1)]),
        }),
    })
    spawn = Compound({"id": String("minecraft:marker"), "Tags": List[String]([String("classroomrun:spawn")])})
    clock = Compound({
        "id": String("minecraft:item_frame"),
        "Facing": Byte(3),  # 3 = sud (Direction.LEGACY_ID_CODEC)
        "Item": Compound({"id": String("minecraft:clock"), "count": Int(1)}),
        "block_pos": IntArray([13, 4, 1]),
    })

    def ent(pos, nbt):
        return Compound({
            "pos": List[Double]([Double(v) for v in pos]),
            "blockPos": List[Int]([Int(int(v // 1)) for v in pos]),
            "nbt": nbt,
        })
    return [ent((7.5, 2.75, 1.02), board), ent((SPAWN[0] + .5, SPAWN[1], SPAWN[2] + .5), spawn),
            ent((13.5, 4.5, 1.5), clock)]


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
    spawns = [e for e in ents if "classroomrun:spawn" in tags(e)]
    boards = [e for e in ents if "classroomrun:board" in tags(e) and str(e["nbt"]["id"]) == "minecraft:text_display"]
    if len(spawns) != 1:
        errs.append("marqueur de spawn absent/multiple")
    if len(boards) != 1:
        errs.append("text_display classroomrun:board absent/multiple")
    elif str(boards[0]["nbt"]["text"]["text"]) != "Classroom Run":
        errs.append("texte du tableau incorrect")
    return errs, chests


COLORS = {
    "air": None, "oak_planks": (170, 140, 90), "spruce_planks": (150, 120, 75), "white_concrete": (225, 228, 228),
    "smooth_stone": (160, 160, 160), "black_concrete": (30, 36, 40), "red_concrete": (190, 40, 40),
    "bookshelf": (120, 80, 40), "stripped_spruce_log": (140, 105, 60), "glass_pane": (150, 210, 235),
    "oak_door": (30, 160, 70), "oak_slab": (235, 205, 150), "oak_fence": (90, 55, 25),
    "oak_stairs": (230, 100, 40), "chest": (200, 140, 30), "lantern": (255, 210, 80), "lectern": (160, 120, 70),
}
ARROW = {"north": (0, -1), "south": (0, 1), "east": (1, 0), "west": (-1, 0)}


def shade(c, f):
    return tuple(min(255, int(v * f)) for v in c)


def preview():
    S = 22
    img = Image.new("RGB", (W * S * 2 + 40, (W + D) * S // 2 + H * S + 40), (245, 245, 245))
    d = ImageDraw.Draw(img)
    ox, oy = D * S + 20, H * S + 20
    P = lambda x, y, z: (ox + (x - z) * S, oy + (x + z) * S / 2 - y * S)
    cells = sorted(grid, key=lambda p: (p[0] + p[2], p[1], p[0]))
    for (x, y, z) in cells:
        n = name_at((x, y, z)).split(":")[1]
        col = COLORS.get(n)
        if col is None or y == 6:  # plafond retiré (coupe)
            continue
        if (x == W - 1 or z == D - 1) and y > 1 and n not in ("oak_door", "red_concrete"):  # murs proches coupés
            continue
        lo, hi = 0, 1
        if n == "oak_slab":
            lo, hi = (0.5, 1) if dict(grid[(x, y, z)][1]).get("type") == "top" else (0, 0.5)
        if n == "lantern":
            lo, hi = 0.4, 1
        a, b = y + lo, y + hi
        d.polygon([P(x, b, z), P(x + 1, b, z), P(x + 1, b, z + 1), P(x, b, z + 1)], fill=shade(col, 1.15), outline=(60, 60, 60))
        d.polygon([P(x + 1, b, z), P(x + 1, b, z + 1), P(x + 1, a, z + 1), P(x + 1, a, z)], fill=shade(col, .8), outline=(60, 60, 60))
        d.polygon([P(x, b, z + 1), P(x + 1, b, z + 1), P(x + 1, a, z + 1), P(x, a, z + 1)], fill=shade(col, .95), outline=(60, 60, 60))
        if n == "oak_stairs":  # flèche = direction vers laquelle regarde l'assis
            fx, fz = ARROW[dict(grid[(x, y, z)][1])["facing"]]
            cx, cy = P(x + .5, b, z + .5)
            ex, ey = P(x + .5 + fx * .5, b, z + .5 + fz * .5)
            d.line([cx, cy, ex, ey], fill=(255, 255, 255), width=3)
    img.crop(img.getbbox()).save(HERE / "preview_iso.png")

    T = 40
    top = Image.new("RGB", (W * T, D * T), (255, 255, 255))
    t = ImageDraw.Draw(top)
    lv = light_map()
    for x in range(W):
        for z in range(D):
            n = col = None
            for y in range(5, -1, -1):
                nn = name_at((x, y, z)).split(":")[1]
                if COLORS.get(nn) and y < 6 and nn != "lantern":
                    n, col = nn, COLORS[nn]
                    break
            if y == 0 and n and 1 <= x <= W - 2 and 1 <= z <= D - 2:  # sol : teinte selon luminosité
                col = shade(col, 0.55 + 0.45 * lv.get((x, 1, z), 0) / 15)
            t.rectangle([x * T, z * T, x * T + T - 1, z * T + T - 1], fill=col, outline=(70, 70, 70))
            if n == "oak_stairs":
                fx, fz = ARROW[dict(grid[(x, 1, z)][1])["facing"]]
                c = (x * T + T // 2, z * T + T // 2)
                t.line([c, (c[0] + fx * 16, c[1] + fz * 16)], fill=(255, 255, 255), width=5)
    top.save(HERE / "preview_top.png")


if __name__ == "__main__":
    if "--check" not in sys.argv:
        build()
        errs, lv = sanity()
        if errs:
            sys.exit("ERREURS : " + "; ".join(errs))
        write()
        preview()
        DEST.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(OUT, DEST)
        print("lumière min sol :", min(lv.get((x, 1, z), 0) for x in range(1, W - 1) for z in range(1, D - 1) if name_at((x, 1, z)) not in OPAQUE))
    errs, chests = check()
    if errs:
        sys.exit("CHECK ÉCHEC : " + "; ".join(errs))
    print(f"OK : {W}x{H}x{D}, DataVersion {DATA_VERSION}, {chests} coffres")
