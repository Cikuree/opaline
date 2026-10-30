package fr.classroomrun.client.history;

import fr.classroomrun.client.history.HistoryModel.Death;
import fr.classroomrun.client.history.HistoryModel.RunDetail;
import fr.classroomrun.client.history.HistoryModel.RunRow;
import fr.classroomrun.client.history.HistoryModel.Sample;
import fr.classroomrun.client.history.HistoryModel.Series;
import fr.classroomrun.net.HistoryRequestPayload;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.multiplayer.ServerData;
import net.minecraft.network.chat.Component;

import java.net.InetSocketAddress;
import java.net.SocketAddress;
import java.nio.file.Path;
import java.util.List;

/**
 * "Historique des morts": list of runs, then the deaths of a run with their context (cause, message, attacker, position,
 * dimension, who dealt the fatal hit, health of the team over the last 10 s) and a "Voir le replay" button.
 * Data comes from {@link HistoryClient}; everything here runs on the client main thread except the download.
 */
public final class HistoryScreen extends Screen {
	private static final int[] PALETTE = {0xFF55FFFF, 0xFFFF5555, 0xFF55FF55, 0xFFFFFF55, 0xFFFF55FF, 0xFFFFAA00, 0xFFAAAAAA, 0xFF5555FF};
	private static final int WHITE = 0xFFFFFFFF;
	private static final int GRAY = 0xFFAAAAAA;
	private static final int ROW_H = 24;

	private List<RunRow> runs = List.of();
	private boolean indexLoaded;
	private int page;
	private int shownRun = -1;
	private RunDetail detail;
	private int selectedDeath;
	private volatile String replayStatus = "";
	private volatile boolean downloading;

	public HistoryScreen() {
		super(Component.literal("Historique des morts"));
	}

	@Override
	protected void init() {
		if (!this.indexLoaded) {
			HistoryClient.request(HistoryRequestPayload.INDEX);
		}
		int bottom = this.height - 28;
		int w = 90;
		int x = this.width / 2 - (3 * w + 2 * 6) / 2;
		if (this.detail != null) {
			addRenderableWidget(Button.builder(Component.literal("Retour"), b -> back()).bounds(x, bottom, w, 20).build());
		} else {
			Button prev = addRenderableWidget(Button.builder(Component.literal("<"), b -> setPage(this.page - 1)).bounds(x, bottom, w, 20).build());
			prev.active = this.page > 0;
		}
		addRenderableWidget(Button.builder(Component.literal("Actualiser"), b -> refresh()).bounds(x + w + 6, bottom, w, 20).build());
		if (this.detail == null) {
			Button next = addRenderableWidget(Button.builder(Component.literal(">"), b -> setPage(this.page + 1)).bounds(x + 2 * (w + 6), bottom, w, 20).build());
			next.active = this.page + 1 < pageCount();
		} else {
			addRenderableWidget(Button.builder(Component.literal("Fermer"), b -> onClose()).bounds(x + 2 * (w + 6), bottom, w, 20).build());
		}
		if (this.detail == null) {
			buildRunButtons();
		} else {
			buildDeathButtons();
		}
	}

	// ---------------------------------------------------------------- index view

	private int rowsPerPage() {
		return Math.max(1, (this.height - 70) / ROW_H);
	}

	private int pageCount() {
		return Math.max(1, (this.runs.size() + rowsPerPage() - 1) / rowsPerPage());
	}

	private void setPage(int p) {
		this.page = Math.max(0, Math.min(pageCount() - 1, p));
		rebuildWidgets();
	}

	private void buildRunButtons() {
		int w = Math.min(380, this.width - 40);
		int x = (this.width - w) / 2;
		int from = this.page * rowsPerPage();
		for (int i = from; i < Math.min(this.runs.size(), from + rowsPerPage()); i++) {
			RunRow r = this.runs.get(i);
			String text = "Run #" + r.run() + "  " + HistoryModel.duration(r.durationMs()) + "  " + r.deaths()
					+ (r.deaths() > 1 ? " morts" : " mort") + (r.bestSplit() != null ? "  " + r.bestSplit() : "")
					+ (r.finished() ? "  (terminee)" : "");
			addRenderableWidget(Button.builder(Component.literal(text), b -> openRun(r.run()))
					.bounds(x, 34 + (i - from) * ROW_H, w, 20).build());
		}
	}

	private void openRun(int run) {
		this.shownRun = run;
		this.selectedDeath = 0;
		this.replayStatus = "";
		HistoryClient.request(run);
	}

	private void back() {
		this.detail = null;
		this.shownRun = -1;
		rebuildWidgets();
	}

	private void refresh() {
		if (this.detail != null) {
			HistoryClient.request(this.detail.run());
		} else {
			HistoryClient.request(HistoryRequestPayload.INDEX);
		}
	}

	/** Called by {@link HistoryClient} on the main thread. */
	void onAnswer(int runNumber, String json) {
		if (runNumber == HistoryRequestPayload.INDEX) {
			this.runs = HistoryModel.parseIndex(json);
			this.indexLoaded = true;
			this.page = Math.min(this.page, pageCount() - 1);
			if (this.detail == null) {
				rebuildWidgets();
			}
		} else if (runNumber == this.shownRun || (this.detail != null && runNumber == this.detail.run())) {
			this.detail = HistoryModel.parseDetail(json);
			this.selectedDeath = Math.min(this.selectedDeath, Math.max(0, this.detail.deaths().size() - 1));
			rebuildWidgets();
		}
	}

	// ---------------------------------------------------------------- run view

	private void buildDeathButtons() {
		List<Death> deaths = this.detail.deaths();
		if (deaths.size() > 1) {
			int w = 70;
			int total = deaths.size() * (w + 4);
			int x = Math.max(10, (this.width - total) / 2);
			for (int i = 0; i < deaths.size(); i++) {
				int index = i;
				Button b = addRenderableWidget(Button.builder(Component.literal("Mort " + (i + 1)), btn -> {
					this.selectedDeath = index;
					this.replayStatus = "";
					rebuildWidgets();
				}).bounds(x + i * (w + 4), 34, w, 20).build());
				b.active = i != this.selectedDeath;
			}
		}
		Death death = selected();
		if (death != null && death.replay() != null && HistoryClient.replayModLoaded() && HistoryClient.httpPort() > 0) {
			Button b = addRenderableWidget(Button.builder(Component.literal("Voir le replay"), btn -> startReplay(death))
					.bounds(this.width / 2 - 75, this.height - 56, 150, 20).build());
			b.active = !this.downloading;
		}
	}

	private Death selected() {
		return this.detail != null && this.selectedDeath < this.detail.deaths().size()
				? this.detail.deaths().get(this.selectedDeath) : null;
	}

	// ---------------------------------------------------------------- replay

	private String serverHost() {
		ServerData data = this.minecraft.getCurrentServer();
		if (data != null) {
			return HistoryModel.host(data.ip);
		}
		if (this.minecraft.getConnection() != null) {
			SocketAddress a = this.minecraft.getConnection().getConnection().getRemoteAddress();
			if (a instanceof InetSocketAddress inet) {
				return inet.getHostString();
			}
		}
		return "localhost"; // integrated server
	}

	private void startReplay(Death death) {
		if (this.downloading || death.replay() == null) {
			return;
		}
		this.downloading = true;
		this.replayStatus = "Telechargement du replay...";
		rebuildWidgets();
		HistoryModel.ReplayRef ref = death.replay();
		var url = ReplayDownloader.url(serverHost(), HistoryClient.httpPort(), ref.path());
		Path target = this.minecraft.gameDirectory.toPath().resolve("replay_recordings").resolve("classroomrun")
				.resolve(ReplayDownloader.fileName(ref.path()));
		Thread t = new Thread(() -> {
			try {
				Path file = ReplayDownloader.download(url, target);
				this.minecraft.execute(() -> {
					try {
						ReplayModBridge.open(file, ref.offsetMs());
						this.replayStatus = "";
					} catch (Exception | LinkageError e) {
						this.replayStatus = "Ouverture impossible : " + e.getMessage();
					}
					finishDownload();
				});
			} catch (Exception e) {
				this.replayStatus = "Echec du telechargement : " + e.getMessage();
				this.minecraft.execute(this::finishDownload);
			}
		}, "classroomrun-replay-download");
		t.setDaemon(true);
		t.start();
	}

	private void finishDownload() {
		this.downloading = false;
		if (this.minecraft.gui.screen() == this) {
			rebuildWidgets();
		}
	}

	// ---------------------------------------------------------------- rendering

	@Override
	public void extractRenderState(GuiGraphicsExtractor g, int mouseX, int mouseY, float a) {
		super.extractRenderState(g, mouseX, mouseY, a);
		g.centeredText(this.font, this.title, this.width / 2, 12, WHITE);
		if (this.detail == null) {
			if (this.shownRun >= 0) {
				g.centeredText(this.font, "Chargement de la run #" + this.shownRun + "...", this.width / 2, 40, GRAY);
			} else if (!this.indexLoaded) {
				g.centeredText(this.font, "Chargement...", this.width / 2, 40, GRAY);
			} else if (this.runs.isEmpty()) {
				g.centeredText(this.font, "Aucune run enregistree.", this.width / 2, 40, GRAY);
			} else {
				g.centeredText(this.font, "Page " + (this.page + 1) + "/" + pageCount(), this.width / 2, this.height - 42, GRAY);
			}
			return;
		}
		int x = Math.max(12, this.width / 2 - 180);
		int y = this.detail.deaths().size() > 1 ? 60 : 34;
		g.text(this.font, "Run #" + this.detail.run() + "  -  " + HistoryModel.when(this.detail.startedAt()), x, y, GRAY);
		y += 14;
		Death d = selected();
		if (d == null) {
			g.text(this.font, "Aucune mort enregistree pour cette run.", x, y, WHITE);
			return;
		}
		g.text(this.font, d.message(), x, y, 0xFFFF5555);
		y += 12;
		y = line(g, x, y, "Joueur", d.name() + "   (coup fatal : " + d.fatalBy() + ")");
		y = line(g, x, y, "Cause", d.cause() + (d.attacker() != null ? "   attaquant : " + d.attacker() : ""));
		y = line(g, x, y, "Position", d.x() + " " + d.y() + " " + d.z() + "   " + d.dimension());
		y = line(g, x, y, "Heure", HistoryModel.when(d.at()));
		int graphW = Math.min(360, this.width - 24);
		int graphH = Math.max(40, Math.min(80, this.height - y - 100));
		drawGraph(g, x, y + 14, graphW, graphH, d.health());
		replayText(g, d);
	}

	private int line(GuiGraphicsExtractor g, int x, int y, String label, String value) {
		g.text(this.font, label + " : " + value, x, y, WHITE);
		return y + 11;
	}

	private void replayText(GuiGraphicsExtractor g, Death d) {
		int cx = this.width / 2;
		if (d.replay() == null) {
			g.centeredText(this.font, "Pas d'enregistrement pour cette mort (ServerReplay absent ou run sans replay).", cx, this.height - 50, GRAY);
		} else if (!HistoryClient.replayModLoaded()) {
			g.centeredText(this.font, "ReplayMod est necessaire pour revoir cette mort (installez-le cote client).", cx, this.height - 50, 0xFFFFAA00);
		} else if (HistoryClient.httpPort() <= 0) {
			g.centeredText(this.font, "Endpoint HTTP du serveur desactive : telechargement impossible.", cx, this.height - 50, 0xFFFFAA00);
		}
		if (!this.replayStatus.isEmpty()) {
			g.centeredText(this.font, this.replayStatus, cx, this.height - 70, 0xFFFFFF55);
		}
	}

	/** Step graph of the health of each player over the 10 s before the death: x = time (-10 s .. 0), y = health points. */
	private void drawGraph(GuiGraphicsExtractor g, int x, int y, int w, int h, List<Series> series) {
		g.fill(x, y, x + w, y + h, 0xA0000000);
		g.outline(x - 1, y - 1, w + 2, h + 2, 0xFF808080);
		g.text(this.font, "-10 s", x, y + h + 3, GRAY);
		g.text(this.font, "0", x + w - 6, y + h + 3, GRAY);
		g.text(this.font, "vie (20 = pleine)", x + w / 2 - 30, y + h + 3, GRAY);
		float max = 20.0F;
		for (Series s : series) {
			for (Sample p : s.samples()) {
				max = Math.max(max, p.hp());
			}
		}
		List<Series> list = series;
		int legendX = x + w + 8;
		for (int i = 0; i < list.size(); i++) {
			int color = PALETTE[i % PALETTE.length];
			Series s = list.get(i);
			if (legendX + 60 < this.width) {
				g.text(this.font, s.name(), legendX, y + i * 11, color);
			}
			Sample prev = null;
			for (Sample p : s.samples()) {
				if (prev != null) {
					int xa = px(x, w, prev.relMs());
					int xb = px(x, w, p.relMs());
					int ya = py(y, h, prev.hp(), max);
					int yb = py(y, h, p.hp(), max);
					g.fill(xa, ya, Math.max(xa + 1, xb), ya + 1, color); // value held until the next change
					g.fill(xb, Math.min(ya, yb), xb + 1, Math.max(ya, yb) + 1, color);
				}
				prev = p;
			}
			if (prev != null) {
				int xa = px(x, w, prev.relMs());
				int ya = py(y, h, prev.hp(), max);
				g.fill(xa, ya, x + w, ya + 1, color);
			}
		}
	}

	private static int px(int x, int w, long relMs) {
		long clamped = Math.max(-10_000L, Math.min(0L, relMs));
		return x + (int) ((clamped + 10_000L) * (w - 1) / 10_000L);
	}

	private static int py(int y, int h, float hp, float max) {
		return y + h - 2 - (int) (Math.max(0F, Math.min(max, hp)) / max * (h - 3));
	}

	@Override
	public boolean isPauseScreen() {
		return false;
	}
}
