package fr.classroomrun.clip;

/**
 * Pure tick-based wait between a collective death and the reset. The deadline only moves closer: the max wait at
 * first, the short grace once every online member is ready, the minimum wait when nobody is online.
 */
public final class ClipWait {
	private final int minTicks;
	private final int graceTicks;
	private int deadline;
	private int elapsed;

	/**
	 * @param minTicks   fixed wait used when nobody is online
	 * @param maxTicks   forced reset (pass {@code minTicks} when there is no clip to watch)
	 * @param graceTicks delay after the last member is ready
	 */
	public ClipWait(int minTicks, int maxTicks, int graceTicks) {
		this.minTicks = Math.max(0, minTicks);
		this.graceTicks = Math.max(0, graceTicks);
		this.deadline = Math.max(0, maxTicks);
	}

	/** One server tick. @return true when the reset is due */
	public boolean tick(int online, int ready) {
		this.elapsed++;
		if (online == 0) {
			this.deadline = Math.min(this.deadline, this.minTicks);
		} else if (ready >= online) {
			this.deadline = Math.min(this.deadline, this.elapsed + this.graceTicks);
		}
		return this.elapsed >= this.deadline;
	}

	public int secondsLeft() {
		return Math.max(0, (this.deadline - this.elapsed + 19) / 20);
	}
}
