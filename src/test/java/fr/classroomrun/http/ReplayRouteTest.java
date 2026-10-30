package fr.classroomrun.http;

import fr.classroomrun.journal.ReplayPaths;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.*;

/** The real JDK HTTP server and the real whitelist, without Minecraft. */
class ReplayRouteTest {
	@Test
	void servesOnlyWhitelistedReplays(@TempDir Path tmp) throws Exception {
		Path root = tmp.resolve("recordings");
		Files.createDirectories(root.resolve("classroomrun/run_1"));
		byte[] content = new byte[300_000];
		new java.util.Random(1).nextBytes(content);
		Files.write(root.resolve("classroomrun/run_1/a.mcpr"), content);
		Files.writeString(root.resolve("classroomrun/run_1/other.mcpr"), "not in a journal");
		Files.writeString(tmp.resolve("secret.txt"), "secret");
		Set<String> whitelist = Set.of("classroomrun/run_1/a.mcpr", "../secret.txt");

		StatsHttpServer http = new StatsHttpServer("127.0.0.1", 0);
		http.setReplayResolver(raw -> ReplayPaths.resolve(whitelist, root, raw));
		http.start();
		try {
			HttpClient client = HttpClient.newHttpClient();
			String base = "http://127.0.0.1:" + http.port() + "/replay?path=";

			HttpResponse<byte[]> ok = get(client, base + "classroomrun%2Frun_1%2Fa.mcpr");
			assertEquals(200, ok.statusCode());
			assertEquals("application/octet-stream", ok.headers().firstValue("Content-Type").orElse(""));
			assertArrayEquals(content, ok.body());

			assertEquals(404, get(client, base + "..%2F..%2Fetc").statusCode());
			assertEquals(404, get(client, base + "../../etc").statusCode());
			assertEquals(404, get(client, base + "classroomrun%2Frun_1%2Fother.mcpr").statusCode(), "not in a journal");
			assertEquals(404, get(client, base + "..%2Fsecret.txt").statusCode(), "whitelisted but outside the directory");
			assertEquals(404, get(client, "http://127.0.0.1:" + http.port() + "/replay").statusCode(), "no path");
			assertEquals(405, client.send(HttpRequest.newBuilder(URI.create(base + "x")).POST(HttpRequest.BodyPublishers.noBody()).build(),
					HttpResponse.BodyHandlers.ofByteArray()).statusCode());
		} finally {
			http.stop();
		}
	}

	private static HttpResponse<byte[]> get(HttpClient client, String url) throws Exception {
		return client.send(HttpRequest.newBuilder(URI.create(url)).GET().build(), HttpResponse.BodyHandlers.ofByteArray());
	}
}
