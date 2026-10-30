package fr.classroomrun.net;

/**
 * Run splits, in display order. The ordinal is the split index used in {@link RunStatePayload}.
 * {@code icon} is the path of a vanilla item id (namespace minecraft) shown in the HUD.
 */
public enum Split {
	PICKAXE("wooden_pickaxe"),
	IRON_INGOT("iron_ingot"),
	DIAMOND("diamond"),
	NETHER_PORTAL("flint_and_steel"),
	NETHER_WART("nether_wart"),
	BLAZE_ROD("blaze_rod"),
	ENDER_EYE("ender_eye"),
	ENTER_END("end_stone"),
	KILL_DRAGON("dragon_head");

	public final String icon;

	Split(String icon) {
		this.icon = icon;
	}

	public int bit() {
		return 1 << ordinal();
	}
}
