package fr.classroomrun.clip;

import fr.classroomrun.net.DeathClipPayload;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;

/**
 * Pure (no Minecraft world) ring buffer of the last {@code capacity} samples and the clip extraction at death.
 * Memory is bounded by the capacity: an entity not seen for the whole window disappears with its last frame.
 */
public final class ClipBuffer {
	/** Stable state of an entity, shared by all its entries. */
	public record Info(String typeId, Optional<UUID> profile, String name, String mainHand) {
	}

	/** One entity at one sample; {@code flags} are the {@link DeathClipPayload} bits. */
	public record Entry(String dimension, Info info, double x, double y, double z, float yaw, float pitch, float headYaw,
			int flags) {
	}

	/** The extracted clip: {@code frameCount} frames and the actors (victim first). */
	public record Clip(int frameCount, List<DeathClipPayload.Actor> actors) {
	}

	private record Cand(UUID id, int first, int last, double closest) {
	}

	private final int capacity;
	private final ArrayDeque<Map<UUID, Entry>> frames = new ArrayDeque<>();

	public ClipBuffer(int capacity) {
		this.capacity = capacity;
	}

	public void add(Map<UUID, Entry> frame) {
		if (this.frames.size() == this.capacity) {
			this.frames.removeFirst();
		}
		this.frames.addLast(frame);
	}

	public int size() {
		return this.frames.size();
	}

	public void clear() {
		this.frames.clear();
	}

	/** 1/256 turn, like vanilla angle packing. */
	public static byte packAngle(float degrees) {
		return (byte) (int) Math.floor(degrees * 256.0F / 360.0F);
	}

	/**
	 * Extracts the clip around {@code (ax, ay, az)} in {@code dimension}. Returns null when nothing was recorded or the
	 * victim never appears. An actor is an entity of that dimension within {@code radius} of the anchor at some frame;
	 * shortcut: if it disappears and reappears only its last contiguous window (the one ending nearest the death) is kept.
	 * The victim gets id 0, the others follow nearest first (closest approach to the anchor), capped at
	 * {@link DeathClipPayload#MAX_ACTORS}.
	 */
	public Clip build(UUID victim, String dimension, double ax, double ay, double az, double radius) {
		List<Map<UUID, Entry>> list = new ArrayList<>(this.frames);
		int n = list.size();
		if (n == 0) {
			return null;
		}
		Map<UUID, Integer> lastSeen = new HashMap<>();
		for (int f = 0; f < n; f++) {
			for (Map.Entry<UUID, Entry> e : list.get(f).entrySet()) {
				if (e.getValue().dimension().equals(dimension)) {
					lastSeen.put(e.getKey(), f);
				}
			}
		}
		List<Cand> cands = new ArrayList<>();
		for (Map.Entry<UUID, Integer> e : lastSeen.entrySet()) {
			int last = e.getValue();
			int first = last;
			while (first > 0 && present(list.get(first - 1), e.getKey(), dimension)) {
				first--;
			}
			double closest = Double.MAX_VALUE;
			for (int f = first; f <= last; f++) {
				Entry en = list.get(f).get(e.getKey());
				closest = Math.min(closest, Math.sqrt(sq(en.x() - ax) + sq(en.y() - ay) + sq(en.z() - az)));
			}
			if (e.getKey().equals(victim) || closest <= radius) {
				cands.add(new Cand(e.getKey(), first, last, closest));
			}
		}
		if (cands.stream().noneMatch(c -> c.id().equals(victim))) {
			return null;
		}
		cands.sort(Comparator.<Cand>comparingInt(c -> c.id().equals(victim) ? 0 : 1)
				.thenComparingDouble(Cand::closest).thenComparing(Cand::id));
		List<DeathClipPayload.Actor> actors = new ArrayList<>();
		for (Cand c : cands.subList(0, Math.min(cands.size(), DeathClipPayload.MAX_ACTORS))) {
			int len = c.last() - c.first() + 1;
			float[] pos = new float[3 * len];
			byte[] rot = new byte[3 * len];
			byte[] flags = new byte[len];
			for (int i = 0; i < len; i++) {
				Entry en = list.get(c.first() + i).get(c.id());
				pos[3 * i] = (float) (en.x() - ax);
				pos[3 * i + 1] = (float) (en.y() - ay);
				pos[3 * i + 2] = (float) (en.z() - az);
				rot[3 * i] = packAngle(en.yaw());
				rot[3 * i + 1] = packAngle(en.pitch());
				rot[3 * i + 2] = packAngle(en.headYaw());
				flags[i] = (byte) en.flags();
			}
			Info info = list.get(c.last()).get(c.id()).info();
			actors.add(new DeathClipPayload.Actor(actors.size(), info.typeId(), info.profile(), info.name(),
					info.mainHand(), c.first(), pos, rot, flags));
		}
		return new Clip(n, actors);
	}

	private static boolean present(Map<UUID, Entry> frame, UUID id, String dimension) {
		Entry e = frame.get(id);
		return e != null && e.dimension().equals(dimension);
	}

	private static double sq(double v) {
		return v * v;
	}
}
