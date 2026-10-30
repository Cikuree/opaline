package fr.classroomrun;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.*;

/**
 * The mod must load without ServerReplay (net.casual, kotlin) and the client without ReplayMod: only the bridge classes
 * may name those packages. Scans the constant pools of the compiled classes.
 */
class OptionalDependencyIsolationTest {
	private static List<Path> classes(String dir, String bridgeName) throws IOException {
		Path root = Path.of("build", "classes", "java", dir);
		assumeBuilt(root);
		try (Stream<Path> s = Files.walk(root)) {
			return s.filter(p -> p.toString().endsWith(".class"))
					.filter(p -> !p.getFileName().toString().startsWith(bridgeName))
					.toList();
		}
	}

	private static void assumeBuilt(Path root) {
		org.junit.jupiter.api.Assumptions.assumeTrue(Files.isDirectory(root), "classes not compiled: " + root);
	}

	private static void assertNoReference(List<Path> files, String... needles) throws IOException {
		assertFalse(files.isEmpty());
		for (Path f : files) {
			String bytes = new String(Files.readAllBytes(f), StandardCharsets.ISO_8859_1);
			for (String n : needles) {
				assertFalse(bytes.contains(n), f + " references " + n);
			}
		}
	}

	@Test
	void serverClassesNeverReferenceServerReplayOrKotlin() throws IOException {
		assertNoReference(classes("main", "ServerReplayHook"), "net/casual", "kotlin/");
	}

	@Test
	void clientClassesNeverReferenceReplayModOutsideTheBridge() throws IOException {
		assertNoReference(classes("client", "ReplayModBridge"), "com/replaymod");
	}
}
