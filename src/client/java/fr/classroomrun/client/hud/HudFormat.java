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
}
