package fr.classroomrun.http;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import fr.classroomrun.journal.JournalEvents;
import fr.classroomrun.journal.JournalStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Instant;

import static org.junit.jupiter.api.Assertions.*;

/** {@code /history} routes on the real JDK HttpServer. */
class StatsHttpServerHistoryTest {
	@TempDir
	Path dir;
	StatsHttpServer server;
	HttpClient client = HttpClient.newHttpClient();

	@BeforeEach
	void start() throws Exception {
		Path run = dir.resolve("3");
		Files.createDirectories(run);
		JsonObject death = JournalEvents.event(JournalEvents.DEATH, Instant.parse("2026-10-30T18:00:00Z"), 1000);
		death.addProperty("name", "Alex");
		death.addProperty("cause", "minecraft:fall");
		Files.writeString(run.resolve(JournalStore.FILE), death + "\n", StandardCharsets.UTF_8);
		this.server = new StatsHttpServer("127.0.0.1", 0, new JournalStore(dir));
		this.server.start();
	}

	@AfterEach
	void stop() {
		this.server.stop();
	}

	private HttpResponse<String> send(String method, String path) throws Exception {
		HttpRequest.Builder b = HttpRequest.newBuilder(URI.create("http://127.0.0.1:" + this.server.port() + path));
		return this.client.send("POST".equals(method) ? b.POST(HttpRequest.BodyPublishers.noBody()).build() : b.GET().build(),
				HttpResponse.BodyHandlers.ofString());
	}

	@Test
	void index() throws Exception {
		HttpResponse<String> r = send("GET", "/history");
		assertEquals(200, r.statusCode());
		assertEquals("application/json; charset=utf-8", r.headers().firstValue("Content-Type").orElseThrow());
		JsonObject runs = JsonParser.parseString(r.body()).getAsJsonObject();
		assertEquals(3, runs.getAsJsonArray("runs").get(0).getAsJsonObject().get("run").getAsInt());
	}

	@Test
	void detail() throws Exception {
		JsonObject d = JsonParser.parseString(send("GET", "/history/3").body()).getAsJsonObject();
		assertEquals(1, d.getAsJsonArray("deaths").size());
		JsonObject unknown = JsonParser.parseString(send("GET", "/history/99").body()).getAsJsonObject();
		assertEquals(0, unknown.getAsJsonArray("deaths").size());
	}

	@Test
	void invalidPathsAre404() throws Exception {
		for (String p : new String[] {"/history/0", "/history/-1", "/history/abc", "/history/03", "/history/2147483648",
				"/history/3/x", "/historyx", "/history/"}) {
			assertEquals(404, send("GET", p).statusCode(), p);
		}
	}

	@Test
	void postIs405() throws Exception {
		HttpResponse<String> r = send("POST", "/history");
		assertEquals(405, r.statusCode());
		assertEquals("GET", r.headers().firstValue("Allow").orElseThrow());
	}
}
