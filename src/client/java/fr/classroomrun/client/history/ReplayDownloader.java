package fr.classroomrun.client.history;

import java.io.IOException;
import java.io.InputStream;
import java.net.URI;
import java.net.URLEncoder;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.time.Duration;

/** Downloads a replay from the server's HTTP endpoint. Blocking: call it off the render thread. */
public final class ReplayDownloader {
	private ReplayDownloader() {
	}

	public static URI url(String host, int port, String replayPath) {
		String h = host.contains(":") ? "[" + host + "]" : host; // IPv6 literal
		return URI.create("http://" + h + ":" + port + "/replay?path=" + URLEncoder.encode(replayPath, StandardCharsets.UTF_8));
	}

	/** A flat, safe file name for a server-side relative path ("a/b c.mcpr" gives "a_b_c.mcpr"). */
	public static String fileName(String replayPath) {
		String name = replayPath.replaceAll("[^A-Za-z0-9._-]", "_");
		return name.endsWith(".mcpr") ? name : name + ".mcpr";
	}

	/**
	 * Downloads to {@code <target>.part} then renames, so a half file is never seen as a replay.
	 *
	 * @return {@code target}
	 */
	public static Path download(URI url, Path target) throws IOException, InterruptedException {
		Files.createDirectories(target.toAbsolutePath().getParent());
		Path part = target.resolveSibling(target.getFileName() + ".part");
		HttpClient client = HttpClient.newBuilder().connectTimeout(Duration.ofSeconds(10)).build();
		HttpResponse<InputStream> response = client.send(HttpRequest.newBuilder(url).GET().build(),
				HttpResponse.BodyHandlers.ofInputStream());
		try (InputStream in = response.body()) {
			if (response.statusCode() != 200) {
				throw new IOException(response.statusCode() == 404 ? "replay introuvable sur le serveur (pas encore sauvegarde ?)"
						: "HTTP " + response.statusCode());
			}
			long expected = response.headers().firstValueAsLong("Content-Length").orElse(-1);
			long copied = Files.copy(in, part, StandardCopyOption.REPLACE_EXISTING);
			if (expected >= 0 && copied != expected) {
				Files.deleteIfExists(part);
				throw new IOException("telechargement incomplet (" + copied + "/" + expected + " octets)");
			}
		} catch (IOException e) {
			Files.deleteIfExists(part);
			throw e;
		}
		Files.move(part, target, StandardCopyOption.REPLACE_EXISTING);
		return target;
	}
}
