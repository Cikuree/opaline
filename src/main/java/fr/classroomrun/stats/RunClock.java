package fr.classroomrun.stats;

/**
 * Cumulative chrono over all runs (no Minecraft dependency). Fed with a monotonic clock in nanoseconds; time only
 * accumulates between two consecutive ticks both flagged as running, so a pause never counts.
 */
public final class RunClock {
	private long totalNanos;
	private long lastNanos;
	private boolean wasRunning;

	public RunClock(long totalTimeMs) {
		this.totalNanos = totalTimeMs * 1_000_000L;
	}

	public void tick(long nowNanos, boolean running) {
		if (running && this.wasRunning) {
			this.totalNanos += Math.max(0L, nowNanos - this.lastNanos);
		}
		this.lastNanos = nowNanos;
		this.wasRunning = running;
	}

	/** Back to 0:00:00 (full wipe). */
	public void clear() {
		this.totalNanos = 0L;
	}

	public long totalTimeMs() {
		return this.totalNanos / 1_000_000L;
	}
}
