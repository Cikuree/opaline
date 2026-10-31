// Données statiques : libellés, textures de repli, notes de version.
export const SPLITS = [
  { k: 'PICKAXE', label: 'Pioche', tex: 'wooden_pickaxe', fb: 'pickaxe' },
  { k: 'IRON_INGOT', label: 'Lingot de fer', tex: 'iron_ingot', fb: 'iron' },
  { k: 'DIAMOND', label: 'Diamant', tex: 'diamond', fb: 'diamond' },
  { k: 'NETHER_PORTAL', label: 'Portail du Nether', tex: 'flint_and_steel', fb: 'portal' },
  { k: 'NETHER_WART', label: 'Forteresse', tex: 'nether_wart', fb: 'gear' },
  { k: 'BLAZE_ROD', label: 'Bâton de blaze', tex: 'blaze_rod', fb: 'blaze' },
  { k: 'ENDER_EYE', label: 'Œil de l’Ender', tex: 'ender_eye', fb: 'eye' },
  { k: 'ENTER_END', label: 'Entrée dans l’End', tex: 'end_stone', fb: 'endp' },
  { k: 'KILL_DRAGON', label: 'Ender Dragon', tex: 'dragon_egg', fb: 'dragon' },
];
export const splitByKey = (k) => SPLITS.find((s) => s.k === k);

export const MODTEX = { 'Fabric API': 'string', 'Classroom Run': 'book', Sodium: 'redstone', Lithium: 'lapis_lazuli', 'Iris Shaders': 'spyglass', 'Sodium Extra': 'glowstone_dust', 'Mod Menu': 'crafting_table_front', Fantasy: 'ender_pearl', ServerReplay: 'clock_00', 'Replay Mod': 'filled_map', Jade: 'emerald', AppleSkin: 'golden_apple' };

// cause de mort (partie après le « : ») -> [libellé, texture]
export const CAUSES = {
  fall: ['Chute de haut', 'feather'], lava: ['Tombé dans la lave', 'lava_bucket'], in_fire: ['Brûlé', 'flint_and_steel'], on_fire: ['Brûlé', 'flint_and_steel'],
  explosion: ['Explosion', 'gunpowder'], player_explosion: ['Explosion', 'gunpowder'], arrow: ['Flèche', 'arrow'], mob_attack: ['Attaque de monstre', 'bone'],
  player_attack: ['Attaque de joueur', 'iron_pickaxe'], fireball: ['Boule de feu', 'blaze_rod'], drown: ['Noyade', 'bone'], starve: ['Faim', 'rotten_flesh'],
  magic: ['Magie', 'glowstone_dust'], out_of_world: ['Tombé dans le vide', 'ender_pearl'], generic: ['Dégâts', 'bone'],
};

export const HELP = {
  tailscaleNotInstalled: ['Tailscale non installé', 'Tailscale n’est pas installé sur ce PC.'],
  tailscaleDisconnected: ['Tailscale non connecté', 'Tailscale n’est pas connecté : ouvre Tailscale et connecte-toi.'],
  noAccess: ['Accès refusé', 'Tu n’as pas accès à ce serveur : demande à l’hôte de t’autoriser sur le réseau.'],
  unexpectedServer: ['Serveur inattendu', 'Le serveur joint ne correspond pas à ce pack.'],
};

export const STAGES = { java: 'Installation de Java', minecraft: 'Téléchargement de Minecraft', loader: 'Installation du chargeur', mods: 'Téléchargement des mods', assets: 'Téléchargement des ressources', textures: 'Extraction des textures' };
export const STAGE_ORDER = ['java', 'minecraft', 'loader', 'mods', 'assets', 'textures'];

export const PERF = [
  { id: 'low', name: 'Faible', sub: 'Pour les PC modestes', gb: 3 },
  { id: 'balanced', name: 'Équilibré', sub: 'Recommandé', gb: 6 },
  { id: 'quality', name: 'Qualité', sub: 'Pour les machines puissantes', gb: 8 },
  { id: 'custom', name: 'Personnalisé', sub: 'Tes propres valeurs', gb: null },
];

// Notes de version statiques (pas de commande dans le contrat v1).
export const OPALINE_NOTES = [
  { v: '0.1.0', fresh: true, items: ['Première version : connexion Microsoft par code, installation et lancement des modpacks.', 'Détection de Tailscale et aide pour rejoindre le réseau.', 'Garde-robe de skins, capes et historique des morts avec graphe de vie.'] },
];

// Pixel-art de repli quand les textures du client.jar ne sont pas disponibles.
export const ICONS = {
  log: { p: { d: '#4a2f1a', b: '#8a5a30', l: '#b98a55' }, g: ['dddddddd', 'dbbbbbbd', 'dblllbbd', 'dblbblbd', 'dblbblbd', 'dbbllbbd', 'dbbbbbbd', 'dddddddd'] },
  pickaxe: { p: { c: '#cfd6dc', b: '#8a5a30' }, g: ['..ccccc.', '.cc...cc', 'c.....bc', '......b.', '.....b..', '....b...', '...b....', '..b.....'] },
  iron: { p: { w: '#ffffff', i: '#d8dde3', s: '#9aa3ad' }, g: ['........', '..wwwwww', '.wiiiiis', 'wiiiiiis', 'wiiiiiss', 'ssssss..', '........', '........'] },
  diamond: { p: { d: '#5ee7e0', e: '#25a9b8', w: '#e8ffff' }, g: ['..dddd..', '.dwddde.', 'dwddddde', 'dddddeee', '.dddeee.', '..deee..', '...ee...', '........'] },
  portal: { p: { o: '#1b1030', P: '#8a4dff', q: '#d2a9ff' }, g: ['oooooooo', 'oPPPPPPo', 'oPqPPqPo', 'oPPPPPPo', 'oPPqPPPo', 'oPPPPqPo', 'oPPPPPPo', 'oooooooo'] },
  blaze: { p: { y: '#ffd24a', o: '#ff8a1f' }, g: ['......yy', '.....yoy', '....yoy.', '...yoy..', '..yoy...', '.yoy....', 'yoy.....', 'yy......'] },
  eye: { p: { g: '#1f8f68', G: '#62e0a2', k: '#0d2a22', p: '#f7fff0' }, g: ['..gggg..', '.gGGGGg.', 'gGGkkGGg', 'gGkppkGg', 'gGkppkGg', 'gGGkkGGg', '.gGGGGg.', '..gggg..'] },
  endp: { p: { s: '#c9c47c', K: '#0b1a26', t: '#37c7b0' }, g: ['ssssssss', 'sKKKKKKs', 'sKtKKtKs', 'sKKKKKKs', 'sKKtKKKs', 'sKKKKtKs', 'sKKKKKKs', 'ssssssss'] },
  grass: { p: { g: '#5fb04a', G: '#7fd05f', d: '#8a5a30', D: '#6e4524' }, g: ['GgGgGgGg', 'gGgGgGgG', 'dgdDdgdd', 'DdddDddd', 'ddDdddDd', 'dddddDdd', 'dDdddddd', 'dddDdddD'] },
  gear: { p: { c: '#cfd6dc' }, g: ['...cc...', '.c.cc.c.', '..cccc..', 'ccc..ccc', 'ccc..ccc', '..cccc..', '.c.cc.c.', '...cc...'] },
  dragon: { p: { k: '#2a1b35', j: '#5b4a73', p: '#e04df0', w: '#fff1e0' }, g: ['k......k', 'kkkkkkkk', 'kpkkkkpk', 'kkkkkkkk', 'kkkkkkkk', '.kjjjjk.', '.kwjjwk.', '..k..k..'] },
};
export const HEADS = [['#4a2f1a', '#e0ac86', '#2f6fd6'], ['#e0574f', '#f2c9a5', '#2a8f5a'], ['#2a2a33', '#b57d58', '#2a2a33'], ['#f0d46a', '#f2c9a5', '#2a8f5a'], ['#7a4fd0', '#d9a07a', '#e0574f'], ['#c0c7d0', '#8a5a3c', '#2f6fd6']];
export const HGRID = ['hhhhhhhh', 'hhhhhhhh', 'hssssssh', 'swbssbws', 'ssssssss', 'sssnnsss', 'sssmmsss', 'hssssssh'];

export const loaderName = (l) => ({ fabric: 'Fabric', neoforge: 'NeoForge', forge: 'Forge', quilt: 'Quilt' }[l] || String(l));
