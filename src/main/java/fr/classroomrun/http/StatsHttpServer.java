package fr.classroomrun.http;

import com.sun.net.httpserver.HttpExchange;
import com.sun.net.httpserver.HttpServer;

import java.io.IOException;
import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.Executors;

/**
 * Read-only JSON endpoint ({@code GET /stats}) on the JDK HTTP server. The HTTP threads only read an immutable
 * {@link StatsSnapshot} published by the server thread; they never touch Minecraft state.
 */
public final class StatsHttpServer {
	private final HttpServer server;
	private volatile StatsSnapshot snapshot;

	public StatsHttpServer(int port) throws IOException {
		this.server = HttpServer.create(new InetSocketAddress(port), 0);
		this.server.createContext("/stats", this::handle);
		this.server.setExecutor(Executors.newSingleThreadExecutor(r -> {
			Thread t = new Thread(r, "classroomrun-http");
			t.setDaemon(true);
			return t;
		}));
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
