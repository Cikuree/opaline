package fr.classroomrun.http;

import com.sun.net.httpserver.HttpExchange;
import com.sun.net.httpserver.HttpServer;

import fr.classroomrun.journal.JournalStore;
import fr.classroomrun.net.HistoryPayload;

import java.io.IOException;
import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.Executors;

/**
 * Read-only JSON endpoint ({@code GET /stats}) on the JDK HTTP server. The HTTP threads only read an immutable
 * {@link StatsSnapshot} published by the server thread; they never touch Minecraft state.
 *
 * <p>Also {@code GET /history} (runs index) and {@code GET /history/<n>} (deaths of run n), the JSON of
 * {@code HistoryPayload} (CONTRACT.md). Journals are read on the HTTP thread through the synchronized
 * {@link JournalStore}, each body capped at {@link HistoryPayload#MAX_JSON} chars. Like {@code /stats}, no
 * authentication: player names and death causes (already visible in game) are readable by anyone who reaches the port.
 */
public final class StatsHttpServer {
	private final HttpServer server;
	private volatile StatsSnapshot snapshot;

	public StatsHttpServer(String bind, int port, JournalStore journals) throws IOException {
		this.server = HttpServer.create(new InetSocketAddress(bind == null || bind.isBlank() ? "0.0.0.0" : bind, port), 0);
		this.server.createContext("/stats", this::handle);
		this.server.createContext("/history", ex -> handleHistory(ex, journals));
		this.server.setExecutor(Executors.newSingleThreadExecutor(r -> {
			Thread t = new Thread(r, "classroomrun-http");
			t.setDaemon(true);
			return t;
		}));
	}

	public int port() {
		return this.server.getAddress().getPort();
	}

	public void start() {
		this.server.start();
	}

	public void stop() {
		this.server.stop(0);
		if (this.server.getExecutor() instanceof java.util.concurrent.ExecutorService executor) {
			executor.shutdownNow();
		}
	}

	public void publish(StatsSnapshot snapshot) {
		this.snapshot = snapshot;
	}

	private void handleHistory(HttpExchange exchange, JournalStore journals) throws IOException {
		try (exchange) {
			if (!"GET".equals(exchange.getRequestMethod())) {
				exchange.getResponseHeaders().set("Allow", "GET");
				exchange.sendResponseHeaders(405, -1);
				return;
			}
			String path = exchange.getRequestURI().getPath();
			String json;
			if (path.equals("/history")) {
				json = journals.indexJson(HistoryPayload.MAX_JSON);
			} else if (path.matches("/history/[1-9][0-9]{0,9}") && Long.parseLong(path.substring(9)) <= Integer.MAX_VALUE) {
				json = journals.detailJson(Integer.parseInt(path.substring(9)), HistoryPayload.MAX_JSON);
			} else {
				exchange.sendResponseHeaders(404, -1);
				return;
			}
			send(exchange, json);
		}
	}

	private static void send(HttpExchange exchange, String json) throws IOException {
		byte[] body = json.getBytes(StandardCharsets.UTF_8);
		exchange.getResponseHeaders().set("Content-Type", "application/json; charset=utf-8");
		exchange.getResponseHeaders().set("Access-Control-Allow-Origin", "*");
		exchange.sendResponseHeaders(200, body.length);
		try (OutputStream out = exchange.getResponseBody()) {
			out.write(body);
		}
	}

	private void handle(HttpExchange exchange) throws IOException {
		try (exchange) {
			StatsSnapshot current = this.snapshot;
			if (!"GET".equals(exchange.getRequestMethod())) {
				exchange.getResponseHeaders().set("Allow", "GET");
				exchange.sendResponseHeaders(405, -1);
				return;
			}
			if (current == null) {
				exchange.sendResponseHeaders(503, -1);
				return;
			}
			byte[] body = current.toJson().getBytes(StandardCharsets.UTF_8);
			exchange.getResponseHeaders().set("Content-Type", "application/json; charset=utf-8");
			exchange.getResponseHeaders().set("Access-Control-Allow-Origin", "*");
			exchange.sendResponseHeaders(200, body.length);
			try (OutputStream out = exchange.getResponseBody()) {
				out.write(body);
			}
		}
	}
}
