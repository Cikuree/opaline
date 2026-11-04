package fr.classroomrun.clip;

/**
 * Pure encoding of the per-frame {@code extra} byte of a clip actor. Bits 0-6: a level (creeper swell, TNT fuse); bit 7: the
 * used item is in the off hand. Shared by the recorder and the client.
 */
public final class ClipExtra {
	private static final int OFF_HAND = 0x80;
	private static final int LEVEL_MASK = 0x7F;
	/** {@code Creeper.getSwelling} reaches 1 at swell = maxSwell - 2 = 28 ticks. */
	public static final int SWELL_TICKS = 28;

	private ClipExtra() {
	}

	/** Creeper swell progress (1 = about to explode) as a level, 1/100 steps. */
	public static int swellLevel(float swelling) {
		return Math.max(0, Math.min(LEVEL_MASK, Math.round(swelling * 100f)));
	}

	/** Inverse of {@link #swellLevel}: the creeper {@code swell} tick count. */
	public static int swellTicks(int level) {
		return Math.round(level / 100f * SWELL_TICKS);
	}

	/** TNT fuse in ticks, capped to the level range (the vanilla fuse is 80). */
	public static int fuseLevel(int fuse) {
		return Math.max(0, Math.min(LEVEL_MASK, fuse));
	}

	public static int pack(int level, boolean offHand) {
		return Math.max(0, Math.min(LEVEL_MASK, level)) | (offHand ? OFF_HAND : 0);
	}

	/** @param extra the byte, signed or not */
	public static int level(int extra) {
		return extra & LEVEL_MASK;
	}

	public static boolean offHand(int extra) {
		return (extra & OFF_HAND) != 0;
	}
}
