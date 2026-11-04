package fr.classroomrun.clip;

import fr.classroomrun.net.DeathClipPayload.BlockChange;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.List;

/**
 * Pure (no Minecraft world) bounded log of block changes, stamped with the sample counter so they line up with the
 * {@link ClipBuffer} frames. Oldest changes are dropped beyond {@code capacity}.
 */
public final class BlockLog {
	private record Change(String dimension, int x, int y, int z, int oldState, int newState, long sample) {
	}

	private final int capacity;
	private final ArrayDeque<Change> changes = new ArrayDeque<>();
	private long samples;

	public BlockLog(int capacity) {
		this.capacity = capacity;
	}

	/** One more entity frame was recorded: later changes belong to the next frame. */
	public void nextSample() {
		this.samples++;
	}

	public void add(String dimension, int x, int y, int z, int oldState, int newState) {
		if (oldState == newState) {
			return;
		}
		if (this.changes.size() == this.capacity) {
			this.changes.removeFirst();
		}
		this.changes.addLast(new Change(dimension, x, y, z, oldState, newState, this.samples));
	}

	public int size() {
		return this.changes.size();
	}

	public void clear() {
		this.changes.clear();
		this.samples = 0;
	}

	/**
	 * Changes in {@code dimension} within {@code radius} (per axis) of the anchor block, for a clip of {@code frameCount}
	 * frames ending at the last recorded sample. A change recorded after {@code k} samples shows from frame
	 * {@code k - firstSample}; older than the clip it is dropped (the next change's old state keeps the replay right). When
	 * there are more than {@code max}, the oldest go for the same reason. Ordered by time.
	 */
	public List<BlockChange> build(String dimension, int ax, int ay, int az, int radius, int frameCount, int max) {
		long first = this.samples - frameCount;
		List<BlockChange> out = new ArrayList<>();
		for (Change c : this.changes) {
			long frame = c.sample() - first;
			if (frame >= 0 && frame <= frameCount && c.dimension().equals(dimension) && Math.abs(c.x() - ax) <= radius
					&& Math.abs(c.y() - ay) <= radius && Math.abs(c.z() - az) <= radius) {
				out.add(new BlockChange((int) frame, c.x() - ax, c.y() - ay, c.z() - az, c.oldState(), c.newState()));
			}
		}
		return out.size() <= max ? out : new ArrayList<>(out.subList(out.size() - max, out.size()));
	}
}
