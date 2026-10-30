package fr.classroomrun.journal;

import java.util.HashMap;
import java.util.Map;
import java.util.UUID;

/** At most one accepted call per key and interval. Server thread only. */
public final class RateLimiter {
	private final long intervalMs;
	private final Map<UUID, Long> last = new HashMap<>();

	public RateLimiter(long intervalMs) {
		this.intervalMs = intervalMs;
	}

	public boolean tryAcquire(UUID key, long nowMs) {
		Long previous = this.last.get(key);
		if (previous != null && nowMs - previous < this.intervalMs) {
			return false;
		}
		this.last.put(key, nowMs);
		return true;
	}

	public void forget(UUID key) {
		this.last.remove(key);
	}
}
