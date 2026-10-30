package fr.classroomrun.client.hud;

/** Latest run state received from the server. Pure logic: the clock is extrapolated from a caller-supplied time. */
public final class ClientRunState {
	private boolean present;
	private long totalTimeMs;
	private boolean running;
	private long receivedAtMs;
	private int deaths;
	private int runNumber;
	private int currentSplit;
	private int reachedSplits;

	private long clockSeconds = -1;
	private String clockText = "0:00:00";
	private String deathsText = "0";

	public void update(long totalTimeMs, boolean running, int deaths, int runNumber, int currentSplit,
			int reachedSplits, long nowMs) {
		boolean deathsChanged = !present || deaths != this.deaths;
		this.present = true;
		this.totalTimeMs = totalTimeMs;
		this.running = running;
		this.receivedAtMs = nowMs;
		this.deaths = deaths;
		this.runNumber = runNumber;
		this.currentSplit = currentSplit;
		this.reachedSplits = reachedSplits;
		if (deathsChanged) {
			deathsText = Integer.toString(deaths);
		}
		clockSeconds = -1;
	}

	public void reset() {
		present = false;
		running = false;
		totalTimeMs = 0;
		deaths = 0;
		reachedSplits = 0;
		currentSplit = 0;
		clockSeconds = -1;
	}

	public boolean present() {
		return present;
	}

	public int runNumber() {
		return runNumber;
	}

	public int currentSplit() {
		return currentSplit;
	}

	public boolean reached(int split) {
		return (reachedSplits & (1 << split)) != 0;
	}

	public long elapsedMs(long nowMs) {
		return running ? totalTimeMs + Math.max(0, nowMs - receivedAtMs) : totalTimeMs;
	}

	/** Same String instance until the displayed second changes. */
	public String clockText(long nowMs) {
		long seconds = elapsedMs(nowMs) / 1000;
		if (seconds != clockSeconds) {
			clockSeconds = seconds;
			clockText = HudFormat.clock(seconds * 1000);
		}
		return clockText;
	}

	/** Same String instance until the death count changes. */
	public String deathsText() {
		return deathsText;
	}
}
