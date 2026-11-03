package fr.classroomrun.client.hud;

/** Pure text formatting for the HUD (no Minecraft dependency). */
public final class HudFormat {
	private HudFormat() {
	}

	/** Hearts rounded to the nearest half heart, like the vanilla health bar: "1", "2.5"; never "0" for a real change. */
	public static String hearts(float hearts) {
		int halves = Math.max(1, Math.round(Math.abs(hearts) * 2f));
		return halves % 2 == 0 ? Integer.toString(halves / 2) : (halves / 2) + ".5";
	}

	/** Total run time as h:mm:ss. */
	public static String clock(long ms) {
		long seconds = Math.max(0, ms) / 1000;
		long h = seconds / 3600;
		long m = seconds / 60 % 60;
		long s = seconds % 60;
		return h + ":" + (m < 10 ? "0" : "") + m + ":" + (s < 10 ? "0" : "") + s;
	}

	/**
	 * Top y of the splits panel: just under the boss bars vanilla draws (BossHealthOverlay, 26.3).
	 * Vanilla starts at y=12, steps 19 per bar and stops stacking once y >= guiHeight / 3.
	 */
	public static int splitsTop(int bossBars, int guiHeight, int top) {
		if (bossBars <= 0) {
			return top;
		}
		int drawn = Math.min(bossBars, Math.max(1, Math.ceilDiv(guiHeight / 3 - 12, 19)));
		return 12 + 19 * (drawn - 1) + 5 + 3;
	}
}
