package fr.classroomrun.client.clip;

/** Pure helpers to read the per-frame arrays of a {@code DeathClipPayload.Actor}. No allocation. */
public final class ClipMath {
	/** Number of values written by {@link #sample}: x, y, z, yaw, pitch, head yaw. */
	public static final int OUT = 6;

	private ClipMath() {
	}

	/** Unpacks an angle stored in 1/256 turns; signed, so pitch keeps its sign. */
	public static float unpackAngle(byte b) {
		return b * (360f / 256f);
	}

	/** Interpolates along the shortest arc, degrees. */
	public static float lerpAngle(float a, float b, float t) {
		float d = ((b - a) % 360f + 540f) % 360f - 180f;
		return a + d * t;
	}

	/**
	 * Interpolated state of one actor at a fractional frame.
	 *
	 * @param first    first frame of the actor
	 * @param frames   number of frames of the actor
	 * @param framePos fractional frame index of the whole clip
	 * @param out      receives x, y, z (relative to the anchor), yaw, pitch, head yaw (degrees); length {@link #OUT}
	 * @return false when the actor is not present at that time (out untouched)
	 */
	public static boolean sample(float[] pos, byte[] rot, int first, int frames, double framePos, float[] out) {
		int frame = (int) Math.floor(framePos);
		int i = frame - first;
		if (frames <= 0 || i < 0 || i >= frames) {
			return false;
		}
		int j = Math.min(i + 1, frames - 1);
		float t = i == j ? 0f : (float) (framePos - frame);
		for (int k = 0; k < 3; k++) {
			float a = pos[3 * i + k];
			out[k] = a + (pos[3 * j + k] - a) * t;
			out[3 + k] = lerpAngle(unpackAngle(rot[3 * i + k]), unpackAngle(rot[3 * j + k]), t);
		}
		return true;
	}

	/** True when {@code mask} is set on any actor frame in {@code (fromExclusive, toInclusive]}. */
	public static boolean anyFlag(byte[] flags, int first, int frames, int fromExclusive, int toInclusive, int mask) {
		int from = Math.max(fromExclusive + 1, first);
		int to = Math.min(toInclusive, first + frames - 1);
		for (int f = from; f <= to; f++) {
			if ((flags[f - first] & mask) != 0) {
				return true;
			}
		}
		return false;
	}
}
