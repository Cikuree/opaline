package fr.classroomrun;

import fr.classroomrun.stats.RunStats;
import fr.classroomrun.stats.StatsStore;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import org.slf4j.Logger;

import java.lang.reflect.Proxy;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class StatsStoreCorruptionTest {
	/** Minimal Logger recording the warn(...) format strings (slf4j-api has no test implementation here). */
	private static Logger recording(List<String> warns) {
		return (Logger) Proxy.newProxyInstance(Logger.class.getClassLoader(), new Class<?>[] {Logger.class}, (p, m, a) -> {
			if (m.getName().equals("warn") && a != null && a.length > 0 && a[0] instanceof String) {
				warns.add((String) a[0]);
			}
			return null;
		});
	}

	@Test
	void corruptedFileIsBackedUpWarnedAndReset(@TempDir Path dir) throws Exception {
		Path file = dir.resolve("stats.json");
		Files.writeString(file, "{not json at all");
		List<String> warns = new ArrayList<>();
		RunStats s = StatsStore.load(file, recording(warns));
		assertEquals(0, s.deaths);
		assertEquals(0, s.runNumber);
		assertEquals(0, s.totalTimeMs);
		assertEquals(-1, s.bestSplit);
		assertFalse(Files.exists(file));
		assertEquals("{not json at all", Files.readString(dir.resolve("stats.json.bak")));
		assertEquals(1, warns.size());
		assertTrue(warns.getFirst().contains("unreadable"), warns.getFirst());
	}

	@Test
	void emptyNullAndWrongShapeFilesAreTreatedAsCorrupted(@TempDir Path dir) throws Exception {
		for (String content : new String[] {"", "null", "[1,2]", "\"text\"", "{\"deaths\":\"many\"}"}) {
			Path file = dir.resolve("stats.json");
			Files.writeString(file, content);
			List<String> warns = new ArrayList<>();
			RunStats s = StatsStore.load(file, recording(warns));
			assertEquals(0, s.deaths, content);
			assertFalse(warns.isEmpty(), "no WARN for: " + content);
			assertTrue(Files.exists(dir.resolve("stats.json.bak")), content);
		}
	}

	@Test
	void secondCorruptionReplacesTheBackup(@TempDir Path dir) throws Exception {
		Path file = dir.resolve("stats.json");
		Files.writeString(file, "{one");
		StatsStore.load(file, recording(new ArrayList<>()));
		Files.writeString(file, "{two");
		StatsStore.load(file, recording(new ArrayList<>()));
		assertEquals("{two", Files.readString(dir.resolve("stats.json.bak")));
	}

	@Test
	void missingFileGivesFreshStatsWithoutWarnOrBackup(@TempDir Path dir) {
		List<String> warns = new ArrayList<>();
		RunStats s = StatsStore.load(dir.resolve("stats.json"), recording(warns));
		assertEquals(0, s.runNumber);
		assertTrue(warns.isEmpty());
		assertFalse(Files.exists(dir.resolve("stats.json.bak")));
	}

	@Test
	void partialJsonKeepsDefaults(@TempDir Path dir) throws Exception {
		Path file = dir.resolve("stats.json");
		Files.writeString(file, "{\"deaths\":3}");
		RunStats s = StatsStore.load(file, recording(new ArrayList<>()));
		assertEquals(3, s.deaths);
		assertEquals(-1, s.bestSplit);
	}

	@Test
	void saveCreatesParentAndLeavesNoTmpFile(@TempDir Path dir) throws Exception {
		Path file = dir.resolve("a/b/stats.json");
		StatsStore.save(file, new RunStats());
		assertTrue(Files.exists(file));
		assertFalse(Files.exists(file.resolveSibling("stats.json.tmp")));
	}

	@Test
	void recoveredStoreCanBeSavedAgain(@TempDir Path dir) throws Exception {
		Path file = dir.resolve("stats.json");
		Files.writeString(file, "garbage");
		RunStats s = StatsStore.load(file, recording(new ArrayList<>()));
		s.recordDeath("Alex");
		StatsStore.save(file, s);
		assertEquals(1, StatsStore.load(file, recording(new ArrayList<>())).deaths);
	}
}
