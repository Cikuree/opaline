package fr.classroomrun.client.hud;

/** Pure text formatting for the HUD (no Minecraft dependency). */
public final class HudFormat {
	private HudFormat() {
	}

	/** Hearts as "1", "2" or "0.5": integer when whole, otherwise one decimal. */
	public static String hearts(float hearts) {
		float rounded = Math.round(hearts * 10f) / 10f;
		return rounded == (int) rounded ? Integer.toString((int) rounded) : Float.toString(rounded);
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
