package fr.classroomrun.client.clip;

import fr.classroomrun.net.DeathClipPayload.BlockChange;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Pure replay logic of the block changes of a clip: for each changed position, the state to show at a given frame.
 * Changes must be ordered by time; a position's changes keep that order.
 */
final class ClipBlocks {
	/** Changes of one position, in time order. */
	record Pos(int dx, int dy, int dz, int[] frames, int[] olds, int[] news) {
		/** State shown at {@code frame}: the old state of the first change not yet happened, else the latest new state. */
		int stateAt(int frame) {
			for (int i = 0; i < this.frames.length; i++) {
				if (this.frames[i] > frame) {
					return this.olds[i];
				}
			}
			return this.news[this.news.length - 1];
		}
	}

	final Pos[] positions;

	ClipBlocks(List<BlockChange> changes) {
		Map<Long, List<BlockChange>> byPos = new HashMap<>();
		List<Long> order = new ArrayList<>();
		for (BlockChange c : changes) {
			long key = ((long) (c.dx() & 0xFFFF) << 32) | ((long) (c.dy() & 0xFFFF) << 16) | (c.dz() & 0xFFFF);
			List<BlockChange> list = byPos.get(key);
			if (list == null) {
				list = new ArrayList<>();
				byPos.put(key, list);
				order.add(key);
			}
			list.add(c);
		}
		this.positions = new Pos[order.size()];
		for (int i = 0; i < this.positions.length; i++) {
			List<BlockChange> l = byPos.get(order.get(i));
			int n = l.size();
			int[] f = new int[n], o = new int[n], w = new int[n];
			for (int j = 0; j < n; j++) {
				f[j] = l.get(j).frame();
				o[j] = l.get(j).oldState();
				w[j] = l.get(j).newState();
			}
			this.positions[i] = new Pos(l.get(0).dx(), l.get(0).dy(), l.get(0).dz(), f, o, w);
		}
	}
}
