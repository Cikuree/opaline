package fr.classroomrun.http;

import com.sun.net.httpserver.HttpExchange;
import com.sun.net.httpserver.HttpServer;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.net.URLDecoder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Optional;
import java.util.concurrent.Executors;
import java.util.function.Function;

/**
 * Read-only endpoints on the JDK HTTP server: {@code GET /stats} (JSON) and {@code GET /replay?path=...} (a .mcpr
 * file). The HTTP threads only read an immutable {@link StatsSnapshot} published by the server thread and the
 * replay resolver (journal files + disk); they never touch Minecraft state.
 */
public final class StatsHttpServer {
	private final HttpServer server;
	private volatile StatsSnapshot snapshot;
	/** Maps the {@code path} query parameter to a servable file; empty means 404. Null until the journal is attached. */
	private volatile Function<String, Optional<Path>> replayResolver = raw -> Optional.empty();

	public StatsHttpServer(String bind, int port) throws IOException {
		this.server = HttpServer.create(new InetSocketAddress(bind == null || bind.isBlank() ? "0.0.0.0" : bind, port), 0);
		this.server.createContext("/stats", this::handle);
		this.server.createContext("/replay", this::handleReplay);
		// a replay download is long: a small pool keeps /stats answering meanwhile
		this.server.setExecutor(Executors.newFixedThreadPool(4, r -> {
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

	public void setReplayResolver(Function<String, Optional<Path>> resolver) {
		this.replayResolver = resolver;
	}

	public void publish(StatsSnapshot snapshot) {
		this.snapshot = snapshot;
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

	private void handleReplay(HttpExchange exchange) throws IOException {
		try (exchange) {
			if (!"GET".equals(exchange.getRequestMethod())) {
				exchange.getResponseHeaders().set("Allow", "GET");
				exchange.sendResponseHeaders(405, -1);
				return;
			}
			Optional<Path> file = this.replayResolver.apply(queryParam(exchange.getRequestURI().getRawQuery(), "path"));
			if (file.isEmpty()) {
				exchange.sendResponseHeaders(404, -1);
				return;
			}
			exchange.getResponseHeaders().set("Content-Type", "application/octet-stream");
			exchange.sendResponseHeaders(200, Files.size(file.get()));
			// streamed in chunks, never loaded in memory
			try (InputStream in = Files.newInputStream(file.get()); OutputStream out = exchange.getResponseBody()) {
				in.transferTo(out);
			}
		}
	}

	/** Decoded value of a query parameter, or null. */
	static String queryParam(String rawQuery, String name) {
		if (rawQuery == null) {
			return null;
		}
		for (String pair : rawQuery.split("&")) {
			int eq = pair.indexOf('=');
			if (eq > 0 && pair.substring(0, eq).equals(name)) {
				try {
					return URLDecoder.decode(pair.substring(eq + 1), StandardCharsets.UTF_8);
				} catch (IllegalArgumentException e) {
					return null;
				}
			}
		}
		return null;
	}
}
