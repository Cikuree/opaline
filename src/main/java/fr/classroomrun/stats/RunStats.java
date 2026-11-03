package fr.classroomrun.stats;

import fr.classroomrun.net.Split;

/**
 * Persistent run statistics (no Minecraft dependency). Serialised as JSON by {@link StatsStore}.
 * Public mutable fields on purpose: Gson reads and writes them directly.
 */
public final class RunStats {
	/** Cumulative chrono over all runs, in ms. */
	public long totalTimeMs;
	/** Team deaths over all runs. */
	public int deaths;
	/** Number of the current (or last started) run; 0 before the first run. */
	public int runNumber;
	/** Bitmask of the splits reached in the current run ({@link Split#bit()}). */
	public int reachedSplits;
	/** Highest split ordinal ever reached over all runs, -1 if none. */
	public int bestSplit = -1;

	/** Starts a new run: increments the number and forgets the splits of the previous one. */
	public void newRun() {
		this.runNumber++;
		this.reachedSplits = 0;
	}

	/** Forgets everything: chrono, deaths, run number, splits and best split. */
	public void wipe() {
		this.totalTimeMs = 0L;
		this.deaths = 0;
		this.runNumber = 0;
		this.reachedSplits = 0;
		this.bestSplit = -1;
	}

	public void recordDeath() {
		this.deaths++;
	}

	/** @return true if the split was not reached yet in this run */
	public boolean markSplit(Split split) {
		if ((this.reachedSplits & split.bit()) != 0) {
			return false;
		}
		this.reachedSplits |= split.bit();
		this.bestSplit = Math.max(this.bestSplit, split.ordinal());
		return true;
	}

	/** Clamps values read from disk so a hand-edited file cannot cause an out-of-bounds index later. */
	public void sanitize() {
		int n = Split.values().length;
		this.totalTimeMs = Math.max(0L, this.totalTimeMs);
		this.deaths = Math.max(0, this.deaths);
		this.runNumber = Math.max(0, this.runNumber);
		this.reachedSplits &= (1 << n) - 1;
		this.bestSplit = Math.max(-1, Math.min(n - 1, this.bestSplit));
	}

	/**
	 * Index of the first split not reached yet; {@code Split.values().length} when the run is finished, i.e. when
	 * KILL_DRAGON was reached (even if an earlier split was skipped).
	 */
	public int currentSplit() {
		if ((this.reachedSplits & Split.KILL_DRAGON.bit()) != 0) {
			return Split.values().length;
		}
		for (Split split : Split.values()) {
			if ((this.reachedSplits & split.bit()) == 0) {
				return split.ordinal();
			}
		}
		return Split.values().length;
	}

	public boolean finished() {
		return this.currentSplit() >= Split.values().length;
	}
}
