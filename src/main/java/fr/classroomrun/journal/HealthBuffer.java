package fr.classroomrun.journal;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;

/**
 * Per-player health over the last {@link #WINDOW_MS}, stored on change only (a flat stretch costs one sample).
 * Pure logic, no Minecraft dependency. Times are caller-supplied milliseconds on any monotonic base.
 */
public final class HealthBuffer {
	public static final long WINDOW_MS = 10_000L;

	/** A point of the graph: time relative to the end ({@code <= 0}) and health points (20.0 = full). */
	public record Point(long relMs, float hp) {
	}

	public record PlayerSamples(String name, List<Point> samples) {
	}

	private record Sample(long t, float hp) {
	}

	private static final class Series {
		String name;
		final ArrayDeque<Sample> samples = new ArrayDeque<>();

		Series(String name) {
			this.name = name;
		}
	}

	private final Map<UUID, Series> series = new LinkedHashMap<>();

	public void record(UUID id, String name, long nowMs, float hp) {
		Series s = this.series.computeIfAbsent(id, k -> new Series(name));
		s.name = name;
		Sample last = s.samples.peekLast();
		if (last == null || last.hp() != hp) {
			s.samples.addLast(new Sample(nowMs, hp));
		}
		// keep a single sample at or before the window start: it carries the value at the window edge
		long edge = nowMs - WINDOW_MS;
		while (s.samples.size() >= 2) {
			Sample first = s.samples.pollFirst();
			if (s.samples.peekFirst().t() > edge) {
				s.samples.addFirst(first);
				break;
			}
		}
	}

	public void remove(UUID id) {
		this.series.remove(id);
	}

	public void clear() {
		this.series.clear();
	}

	/**
	 * @param endHp health to append at relative time 0 (e.g. 0.0 on death), or null to repeat the last known value
	 */
	public List<PlayerSamples> snapshot(long nowMs, Float endHp) {
		List<PlayerSamples> out = new ArrayList<>();
		for (Series s : this.series.values()) {
			if (s.samples.isEmpty()) {
				continue;
			}
			List<Point> points = new ArrayList<>();
			for (Sample sample : s.samples) {
				points.add(new Point(Math.max(-WINDOW_MS, Math.min(0L, sample.t() - nowMs)), sample.hp()));
			}
			float end = endHp != null ? endHp : s.samples.peekLast().hp();
			Point last = points.getLast();
			if (last.relMs() == 0L) {
				points.set(points.size() - 1, new Point(0L, end));
			} else {
				points.add(new Point(0L, end));
			}
			out.add(new PlayerSamples(s.name, points));
		}
		return out;
	}
}
