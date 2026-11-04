package fr.classroomrun.clip;

import fr.classroomrun.net.DeathClipPayload.Explosion;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.List;

/**
 * Pure (no Minecraft world) bounded log of explosions, stamped with the sample counter like {@link BlockLog} so they line up
 * with the {@link ClipBuffer} frames. Oldest explosions are dropped beyond {@code capacity}.
 */
public final class ExplosionLog {
	private record Blast(String dimension, double x, double y, double z, float radius, long sample) {
	}

	private final int capacity;
	private final ArrayDeque<Blast> blasts = new ArrayDeque<>();
	private long samples;

	public ExplosionLog(int capacity) {
		this.capacity = capacity;
	}

	/** One more entity frame was recorded: later explosions belong to the next frame. */
	public void nextSample() {
		this.samples++;
	}

	public void add(String dimension, double x, double y, double z, float radius) {
		if (this.blasts.size() == this.capacity) {
			this.blasts.removeFirst();
		}
		this.blasts.addLast(new Blast(dimension, x, y, z, radius, this.samples));
	}

	public int size() {
		return this.blasts.size();
	}

	public void clear() {
		this.blasts.clear();
		this.samples = 0;
	}

	/**
	 * Explosions in {@code dimension} within {@code maxDistance} of the anchor, for a clip of {@code frameCount} frames ending
	 * at the last recorded sample. Frame numbering is the one of {@link BlockLog#build}; one that happened after the last
	 * frame is shown on it, older than the clip it is dropped. Keeps the latest {@code max}, ordered by time.
	 */
	public List<Explosion> build(String dimension, double ax, double ay, double az, double maxDistance, int frameCount,
			int max) {
		long first = this.samples - frameCount;
		List<Explosion> out = new ArrayList<>();
		for (Blast b : this.blasts) {
			long frame = b.sample() - first;
			double dx = b.x() - ax, dy = b.y() - ay, dz = b.z() - az;
			if (frame >= 0 && b.dimension().equals(dimension) && dx * dx + dy * dy + dz * dz <= maxDistance * maxDistance) {
				out.add(new Explosion((int) Math.min(frame, frameCount - 1), (float) dx, (float) dy, (float) dz,
						Math.min(b.radius(), 64f)));
			}
		}
		return out.size() <= max ? out : new ArrayList<>(out.subList(out.size() - max, out.size()));
	}
}
