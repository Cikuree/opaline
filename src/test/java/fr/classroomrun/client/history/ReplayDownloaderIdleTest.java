package fr.classroomrun.client.history;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.IOException;
import java.net.ServerSocket;
import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.*;

class ReplayDownloaderIdleTest {
	@Test
	void silentServerFailsInsteadOfBlocking(@TempDir Path tmp) throws Exception {
		try (ServerSocket silent = new ServerSocket(0, 5, java.net.InetAddress.getLoopbackAddress())) {
			Path target = tmp.resolve("x.mcpr");
			long start = System.nanoTime();
			assertThrows(IOException.class, () -> ReplayDownloader.download(
					ReplayDownloader.url("127.0.0.1", silent.getLocalPort(), "a"), target, 2));
			assertTrue((System.nanoTime() - start) / 1_000_000_000L < 20, "bounded wait");
			assertFalse(Files.exists(target));
			assertFalse(Files.exists(tmp.resolve("x.mcpr.part")));
		}
	}
}
