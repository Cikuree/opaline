package fr.classroomrun.client.clip;

import fr.classroomrun.net.DeathClipPayload;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.input.MouseButtonEvent;
import net.minecraft.network.chat.Component;
import net.minecraft.util.Util;

/**
 * Playback of the death clip: orbit camera around the victim, play/pause, seekable timeline, x1 / x0.5 and « Prêt ».
 * Without a stage (other dimension) only the message and « Prêt » are shown. Escape hides the screen but keeps the clip.
 */
public final class DeathClipScreen extends Screen {
	private static final Component PLAY = Component.literal("Lecture");
	private static final Component PAUSE = Component.literal("Pause");
	private static final Component REPLAY = Component.literal("Rejouer");
	private static final Component READY = Component.literal("Prêt");
	private static final Component SPEED_1 = Component.literal("x1");
	private static final Component SPEED_HALF = Component.literal("x0.5");
	private static final Component UNAVAILABLE = Component.literal("Clip indisponible ici");
	private static final Component DEATH_LABEL = Component.literal("Mort");
	private static final int WHITE = 0xFFFFFFFF;
	private static final int RED = 0xFFFF5555;
	private static final int PANEL = 0x90000000;
	private static final int TRACK = 0xFF404040;
	private static final int FILL = 0xFF55FFFF;
	private static final int BAR_H = 44;
	private static final long MAX_FRAME_MS = 250;

	private final DeathClipPayload clip;
	private ClipStage stage;
	private ClipTimeline timeline;
	private Button play;
	private int playLabel = -1;
	private boolean seeking;
	private long lastMs;
	private int trackX0, trackX1, trackY;
	private String timeText = "";
	private int timeSeconds = -1;

	public DeathClipScreen(DeathClipPayload clip) {
		super(Component.literal("Clip de mort"));
		this.clip = clip;
	}

	@Override
	public boolean isPauseScreen() {
		return false;
	}

	@Override
	protected void init() {
		Minecraft mc = this.minecraft;
		if (this.stage == null && mc.level != null && mc.level.dimension().equals(this.clip.dimension())) {
			this.stage = ClipStage.open(mc, mc.level, this.clip);
			if (this.stage != null) {
				this.timeline = new ClipTimeline(this.clip.frameCount(), this.clip.sampleTicks());
				this.lastMs = Util.getMillis();
			}
		}
		int y = this.height - 28;
		int margin = 10;
		int gap = 4;
		int readyW = 60;
		addRenderableWidget(Button.builder(READY, b -> ClipClient.ready())
				.bounds(this.width - margin - readyW, y, readyW, 20).build());
		if (this.stage == null) {
			return;
		}
		int playW = 56;
		int speedW = 40;
		this.play = addRenderableWidget(Button.builder(PAUSE, b -> {
			this.timeline.togglePlaying();
			refreshPlayLabel();
		}).bounds(margin, y, playW, 20).build());
		this.playLabel = -1;
		addRenderableWidget(Button.builder(this.timeline.speed() == 1f ? SPEED_1 : SPEED_HALF, b -> {
			this.timeline.setSpeed(this.timeline.speed() == 1f ? 0.5f : 1f);
			b.setMessage(this.timeline.speed() == 1f ? SPEED_1 : SPEED_HALF);
		}).bounds(margin + playW + gap, y, speedW, 20).build());
		this.trackX0 = margin + playW + speedW + 2 * gap + 4;
		this.trackX1 = this.width - margin - readyW - gap - 4;
		this.trackY = y + 10;
		refreshPlayLabel();
	}

	private void refreshPlayLabel() {
		int label = this.timeline.playing() ? 1 : this.timeline.atEnd() ? 2 : 0;
		if (label != this.playLabel) {
			this.playLabel = label;
			this.play.setMessage(label == 1 ? PAUSE : label == 2 ? REPLAY : PLAY);
		}
	}

	@Override
	public void tick() {
		if (ClipClient.clip() != this.clip || this.stage != null && this.minecraft.level != this.stage.level()) {
			onClose();
		}
	}

	@Override
	public void removed() {
		if (this.stage != null) {
			this.stage.close(this.minecraft);
			this.stage = null;
		}
	}

	/** No dimming: the world is the content. */
	@Override
	public void extractBackground(GuiGraphicsExtractor g, int mouseX, int mouseY, float a) {
	}

	@Override
	public void extractRenderState(GuiGraphicsExtractor g, int mouseX, int mouseY, float a) {
		if (this.stage != null) {
			long now = Util.getMillis();
			this.timeline.advance(Math.min(now - this.lastMs, MAX_FRAME_MS));
			this.lastMs = now;
			this.stage.update(this.timeline);
			refreshPlayLabel();
		}
		g.fill(0, this.height - BAR_H, this.width, this.height, PANEL);
		super.extractRenderState(g, mouseX, mouseY, a);
		String ready = ClipClient.readyText();
		if (ready != null) {
			g.centeredText(this.font, ready, this.width / 2, this.height - BAR_H - 12, WHITE);
		}
		if (this.stage == null) {
			g.centeredText(this.font, UNAVAILABLE, this.width / 2, this.height / 2, WHITE);
			return;
		}
		extractTimeline(g);
	}

	private void extractTimeline(GuiGraphicsExtractor g) {
		int x0 = this.trackX0;
		int x1 = this.trackX1;
		int y = this.trackY;
		g.fill(x0, y - 3, x1, y + 3, TRACK);
		int filled = x0 + (int) ((x1 - x0) * this.timeline.fraction());
		g.fill(x0, y - 3, filled, y + 3, FILL);
		g.fill(filled - 1, y - 6, filled + 1, y + 6, WHITE);
		// the last frame is the death
		g.fill(x1, y - 7, x1 + 2, y + 7, RED);
		int seconds = this.timeline.elapsedSeconds();
		if (seconds != this.timeSeconds) {
			this.timeSeconds = seconds;
			this.timeText = seconds + " s / " + this.timeline.totalSeconds() + " s";
		}
		g.text(this.font, this.timeText, x0, y - 6 - this.font.lineHeight - 2, WHITE);
		g.text(this.font, DEATH_LABEL, x1 - this.font.width(DEATH_LABEL), y - 6 - this.font.lineHeight - 2, RED);
	}

	private boolean onTrack(double x, double y) {
		return this.stage != null && x >= this.trackX0 - 3 && x <= this.trackX1 + 3 && Math.abs(y - this.trackY) <= 8;
	}

	private void seek(double x) {
		this.timeline.seekFraction((x - this.trackX0) / (this.trackX1 - this.trackX0));
	}

	@Override
	public boolean mouseClicked(MouseButtonEvent event, boolean doubleClick) {
		if (super.mouseClicked(event, doubleClick)) {
			return true;
		}
		if (event.button() == 0 && onTrack(event.x(), event.y())) {
			this.seeking = true;
			seek(event.x());
			return true;
		}
		return false;
	}

	@Override
	public boolean mouseDragged(MouseButtonEvent event, double dx, double dy) {
		if (this.seeking) {
			seek(event.x());
			return true;
		}
		if (super.mouseDragged(event, dx, dy)) {
			return true;
		}
		if (this.stage != null && event.button() == 0 && event.y() < this.height - BAR_H) {
			this.stage.orbit().rotate(dx, dy);
			return true;
		}
		return false;
	}

	@Override
	public boolean mouseReleased(MouseButtonEvent event) {
		this.seeking = false;
		return super.mouseReleased(event);
	}

	@Override
	public boolean mouseScrolled(double x, double y, double scrollX, double scrollY) {
		if (this.stage != null) {
			this.stage.orbit().zoom(scrollY);
			return true;
		}
		return super.mouseScrolled(x, y, scrollX, scrollY);
	}
}
