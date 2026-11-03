package fr.classroomrun.client.clip;

/** Real-time playback position of a clip: time, play/pause, speed and the mapping to fractional frames. Pure logic. */
public final class ClipTimeline {
	private final long frameMs;
	private final long durationMs;
	private double posMs;
	private boolean playing = true;
	private float speed = 1f;

	public ClipTimeline(int frameCount, int sampleTicks) {
		this.frameMs = sampleTicks * 50L;
		this.durationMs = Math.max(0, frameCount - 1) * this.frameMs;
	}

	/** Moves forward by real elapsed time; stops on the last (death) frame. */
	public void advance(long deltaMs) {
		if (!this.playing) {
			return;
		}
		this.posMs += deltaMs * this.speed;
		if (this.posMs >= this.durationMs) {
			this.posMs = this.durationMs;
			this.playing = false;
		}
	}

	/** Pause/resume; resuming at the end replays from the start. */
	public void togglePlaying() {
		if (this.playing) {
			this.playing = false;
			return;
		}
		if (atEnd()) {
			this.posMs = 0;
		}
		this.playing = this.durationMs > 0;
	}

	public void seekFraction(double fraction) {
		this.posMs = Math.max(0, Math.min(1, fraction)) * this.durationMs;
		if (atEnd()) {
			this.playing = false;
		}
	}

	public void setSpeed(float speed) {
		this.speed = speed;
	}

	public float speed() {
		return this.speed;
	}

	public boolean playing() {
		return this.playing;
	}

	public boolean atEnd() {
		return this.posMs >= this.durationMs;
	}

	/** Fractional frame index, in {@code [0, frameCount - 1]}. */
	public double framePos() {
		return this.posMs / this.frameMs;
	}

	/** Position in {@code [0, 1]} (1 when the clip is a single frame). */
	public double fraction() {
		return this.durationMs == 0 ? 1.0 : this.posMs / this.durationMs;
	}

	public int elapsedSeconds() {
		return (int) (this.posMs / 1000);
	}

	public int totalSeconds() {
		return (int) (this.durationMs / 1000);
	}
}
