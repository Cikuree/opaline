// Données statiques : libellés, noms de textures du client.jar (liste miroir : `UI_TEXTURES`, launcher/src/textures.rs), notes de version.
export const MOJANG_PENDING = 'Opaline attend la validation de Mojang. En attendant, tu joues via le Launcher Minecraft : ton compte et ton skin y sont gérés.';
export const SKIN_LOCKED = 'Changer de skin depuis Opaline sera possible quand Mojang aura validé l’app ; en attendant, minecraft.net ou le Launcher Minecraft.';
export const SPLITS = [
  { k: 'PICKAXE', label: 'Pioche', tex: 'wooden_pickaxe' },
  { k: 'IRON_INGOT', label: 'Lingot de fer', tex: 'iron_ingot' },
  { k: 'DIAMOND', label: 'Diamant', tex: 'diamond' },
  { k: 'NETHER_PORTAL', label: 'Portail du Nether', tex: 'flint_and_steel' },
  { k: 'NETHER_WART', label: 'Forteresse', tex: 'nether_wart' },
  { k: 'BLAZE_ROD', label: 'Bâton de blaze', tex: 'blaze_rod' },
  { k: 'ENDER_EYE', label: 'Œil de l’Ender', tex: 'ender_eye' },
  { k: 'ENTER_END', label: 'Entrée dans l’End', tex: 'end_stone' },
  { k: 'KILL_DRAGON', label: 'Ender Dragon', tex: 'end_crystal' },
];
export const splitByKey = (k) => SPLITS.find((s) => s.k === k);

export const MODTEX = { 'Fabric API': 'string', 'Classroom Run': 'book', Sodium: 'redstone', Lithium: 'lapis_lazuli', 'Iris Shaders': 'spyglass', 'Sodium Extra': 'glowstone_dust', 'Mod Menu': 'crafting_table_front', Fantasy: 'ender_pearl', ServerReplay: 'clock_00', 'Replay Mod': 'filled_map', Jade: 'emerald', AppleSkin: 'golden_apple' };

// cause de mort (partie après le « : ») -> [libellé, texture]
export const CAUSES = {
  fall: ['Chute de haut', 'feather'], lava: ['Tombé dans la lave', 'lava_bucket'], in_fire: ['Brûlé', 'flint_and_steel'], on_fire: ['Brûlé', 'flint_and_steel'],
  explosion: ['Explosion', 'gunpowder'], player_explosion: ['Explosion', 'gunpowder'], arrow: ['Flèche', 'arrow'], mob_attack: ['Attaque de monstre', 'iron_sword'],
  player_attack: ['Attaque de joueur', 'iron_sword'], fireball: ['Boule de feu', 'fire_charge'], drown: ['Noyade', 'water_bucket'], starve: ['Faim', 'rotten_flesh'],
  magic: ['Magie', 'splash_potion'], out_of_world: ['Tombé dans le vide', 'ender_pearl'], generic: ['Dégâts', 'totem_of_undying'],
};

export const HELP = {
  tailscaleNotInstalled: ['Tailscale non installé', 'Tailscale n’est pas installé sur ce PC.'],
  tailscaleDisconnected: ['Tailscale non connecté', 'Tailscale n’est pas connecté : ouvre Tailscale et connecte-toi.'],
  noAccess: ['Accès refusé', 'Tu n’as pas accès à ce serveur : demande à l’hôte de t’autoriser sur le réseau.'],
  serverOffline: ['Serveur hors ligne', 'Serveur hors ligne ou injoignable.'],
  unexpectedServer: ['Serveur inattendu', 'Le serveur joint ne correspond pas à ce pack.'],
};

export const STAGES = { java: 'Installation de Java', minecraft: 'Téléchargement de Minecraft', loader: 'Installation du chargeur', mods: 'Téléchargement des mods', overrides: 'Téléchargement des fichiers du pack', assets: 'Téléchargement des ressources', textures: 'Extraction des textures' };
export const STAGE_ORDER = ['java', 'minecraft', 'assets', 'loader', 'mods', 'overrides', 'textures']; // ordre émis par le backend (contrat §6)

/** Catégories de la copie de réglages entre packs (identifiants du contrat). */
export const COPY_CATS = [['keys', 'Touches'], ['mouse', 'Souris et sensibilité'], ['fov', 'Champ de vision (FOV)'], ['sound', 'Son'], ['language', 'Langue'], ['accessibility', 'Accessibilité']];
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

export const loaderName = (l) => ({ fabric: 'Fabric', neoforge: 'NeoForge', forge: 'Forge', quilt: 'Quilt' }[l] || String(l));
