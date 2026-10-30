package fr.classroomrun.client.history;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
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
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicLong;

/** Downloads a replay from the server's HTTP endpoint. Blocking: call it off the render thread. */
public final class ReplayDownloader {
	/** A server that stays silent this long (headers or body) fails the download instead of blocking the thread. */
	static final long IDLE_SECONDS = 30;

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
		return download(url, target, IDLE_SECONDS);
	}

	static Path download(URI url, Path target, long idleSeconds) throws IOException, InterruptedException {
		Files.createDirectories(target.toAbsolutePath().getParent());
		Path part = target.resolveSibling(target.getFileName() + ".part");
		AtomicLong lastProgress = new AtomicLong(System.nanoTime());
		ScheduledExecutorService watchdog = Executors.newSingleThreadScheduledExecutor(r -> {
			Thread t = new Thread(r, "classroomrun-replay-watchdog");
			t.setDaemon(true);
			return t;
		});
		try (HttpClient client = HttpClient.newBuilder().connectTimeout(Duration.ofSeconds(10)).build()) {
			HttpResponse<InputStream> response = client.send(
					HttpRequest.newBuilder(url).timeout(Duration.ofSeconds(idleSeconds)).GET().build(),
					HttpResponse.BodyHandlers.ofInputStream());
			try (InputStream in = response.body(); OutputStream out = Files.newOutputStream(part)) {
				int status = response.statusCode();
				if (status != 200) {
					throw new IOException(switch (status) {
						case 404 -> "replay introuvable sur le serveur";
						case 503 -> "replay en cours d'enregistrement ou de sauvegarde, reessaie dans quelques secondes";
						default -> "HTTP " + status;
					});
				}
				// a stalled body: closing the stream makes the blocked read fail
				watchdog.scheduleWithFixedDelay(() -> {
					if (System.nanoTime() - lastProgress.get() > TimeUnit.SECONDS.toNanos(idleSeconds)) {
						try {
							in.close();
						} catch (IOException ignored) {
							// nothing more to do
						}
					}
				}, 1, 1, TimeUnit.SECONDS);
				long expected = response.headers().firstValueAsLong("Content-Length").orElse(-1);
				byte[] buf = new byte[65536];
				long copied = 0;
				int n;
				while ((n = in.read(buf)) > 0) {
					out.write(buf, 0, n);
					copied += n;
					lastProgress.set(System.nanoTime());
				}
				if (expected >= 0 && copied != expected) {
					throw new IOException("telechargement incomplet (" + copied + "/" + expected + " octets)");
				}
			}
		} catch (IOException | InterruptedException | RuntimeException e) {
			Files.deleteIfExists(part);
			throw e;
		} finally {
			watchdog.shutdownNow();
		}
		Files.move(part, target, StandardCopyOption.REPLACE_EXISTING);
		return target;
	}
}
