package fr.classroomrun.client.hud;

import java.util.UUID;

/** Fixed pool of damage/heal feed entries. Pure logic: all times are supplied by the caller. */
public final class DamageFeed {
	public static final int CAPACITY = 8;
	public static final long MERGE_WINDOW_MS = 500;
	public static final long LIFETIME_MS = 1500;
	private static final float FADE_START = 0.4f;

	public static final class Entry {
		public UUID player;
		public boolean heal;
		public float amount;
		public long startMs;
		public long lastMs;
		public boolean active;
		public String text = "";
		/** Pixel width of {@link #text}, filled by the renderer; -1 means "not measured". */
		public int textWidth = -1;
		long seq;
	}

	private final Entry[] entries = new Entry[CAPACITY];
	private long nextSeq;

	public DamageFeed() {
		for (int i = 0; i < CAPACITY; i++) {
			entries[i] = new Entry();
		}
	}

	public void add(UUID player, float hearts, boolean heal, long nowMs) {
		Entry free = null;
		Entry oldest = null;
		for (Entry e : entries) {
			if (e.active) {
				if (e.heal == heal && e.player.equals(player) && nowMs - e.lastMs < MERGE_WINDOW_MS) {
					e.amount += hearts;
					e.startMs = nowMs;
					e.lastMs = nowMs;
					setText(e);
					return;
				}
				if (oldest == null || e.seq < oldest.seq) {
					oldest = e;
				}
			} else if (free == null) {
				free = e;
			}
		}
		Entry e = free != null ? free : oldest;
		e.active = true;
		e.player = player;
		e.heal = heal;
		e.amount = hearts;
		e.startMs = nowMs;
		e.lastMs = nowMs;
		e.seq = nextSeq++;
		setText(e);
	}

	private static void setText(Entry e) {
		e.text = (e.heal ? "+" : "-") + HudFormat.hearts(e.amount);
		e.textWidth = -1;
	}

	public void expire(long nowMs) {
		for (Entry e : entries) {
			if (e.active && nowMs - e.startMs >= LIFETIME_MS) {
				e.active = false;
			}
		}
	}

	public void clear() {
		for (Entry e : entries) {
			e.active = false;
		}
	}

	/** Writes the active entries into {@code out} (length >= CAPACITY), oldest first. Returns their count. */
	public int ordered(Entry[] out) {
		int n = 0;
		for (Entry e : entries) {
			if (!e.active) {
				continue;
			}
			int i = n++;
			while (i > 0 && out[i - 1].seq > e.seq) {
				out[i] = out[i - 1];
				i--;
			}
			out[i] = e;
		}
		return n;
	}

	public static float progress(Entry e, long nowMs) {
		return Math.min(1f, Math.max(0f, (nowMs - e.startMs) / (float) LIFETIME_MS));
	}

	/** Opaque for the first 40 %, then linear fade to 0. */
	public static float alpha(float progress) {
		return progress < FADE_START ? 1f : Math.max(0f, 1f - (progress - FADE_START) / (1f - FADE_START));
	}

	/** Ease-out rise in pixels. */
	public static float rise(float progress, float maxPixels) {
		float inv = 1f - progress;
		return maxPixels * (1f - inv * inv);
	}
}
