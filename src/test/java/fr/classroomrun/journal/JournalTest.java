package fr.classroomrun.journal;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Instant;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.Set;
import java.util.UUID;

import static org.junit.jupiter.api.Assertions.*;

class JournalTest {
	private static final Instant AT = Instant.parse("2026-10-30T18:00:00Z");

	// ---------------------------------------------------------------- HealthBuffer

	@Test
	void healthBufferStoresChangesOnlyAndKeepsTenSeconds() {
		HealthBuffer b = new HealthBuffer();
		UUID id = UUID.randomUUID();
		for (long t = 0; t <= 30_000; t += 50) {
			b.record(id, "Alex", t, t < 25_000 ? 20.0F : 6.0F); // one change, at t = 25 000
		}
		List<HealthBuffer.PlayerSamples> snap = b.snapshot(30_000, 0.0F);
		assertEquals(1, snap.size());
		List<HealthBuffer.Point> pts = snap.getFirst().samples();
		// value at the window edge (20.0, clamped to -10 000), the change (-5 000 ms before the end), the death point
		assertEquals(3, pts.size());
		assertEquals(new HealthBuffer.Point(-10_000, 20.0F), pts.get(0));
		assertEquals(new HealthBuffer.Point(-5_000, 6.0F), pts.get(1));
		assertEquals(new HealthBuffer.Point(0, 0.0F), pts.getLast());
		assertTrue(pts.stream().allMatch(p -> p.relMs() <= 0 && p.relMs() >= -10_000));
	}

	@Test
	void healthBufferPrunesOldSamplesAndForgetsPlayers() {
		HealthBuffer b = new HealthBuffer();
		UUID a = UUID.randomUUID();
		UUID c = UUID.randomUUID();
		float hp = 20F;
		for (long t = 0; t < 100_000; t += 100) {
			b.record(a, "A", t, hp -= 0.001F); // changes every call: worst case
		}
		b.record(c, "C", 99_900, 10F);
		b.remove(c);
		List<HealthBuffer.PlayerSamples> snap = b.snapshot(99_900, null);
		assertEquals(1, snap.size());
		assertTrue(snap.getFirst().samples().size() <= 103, "window bounded: " + snap.getFirst().samples().size());
	}

	@Test
	void healthBufferRepeatsLastValueWithoutEndValue() {
		HealthBuffer b = new HealthBuffer();
		b.record(UUID.randomUUID(), "A", 1000, 14F);
		List<HealthBuffer.Point> pts = b.snapshot(4000, null).getFirst().samples();
		assertEquals(List.of(new HealthBuffer.Point(-3000, 14F), new HealthBuffer.Point(0, 14F)), pts);
	}

	// ---------------------------------------------------------------- journal write / read

	private static JsonObject deathLine(long t, String replayPath) {
		JsonObject o = JournalEvents.event(JournalEvents.DEATH, AT.plusSeconds(t / 1000), t);
		o.addProperty("player", "uuid");
		o.addProperty("name", "Alex");
		o.addProperty("cause", "minecraft:fall");
		o.addProperty("message", "Alex fell from a high place");
		o.add("attacker", com.google.gson.JsonNull.INSTANCE);
		o.addProperty("fatalBy", "Alex");
		o.add("pos", JournalEvents.pos(12, 64, -30));
		o.addProperty("dimension", "classroomrun:run_20_overworld");
		if (replayPath != null) {
			o.add("replay", JournalEvents.replay(replayPath, t));
		} else {
			o.add("replay", com.google.gson.JsonNull.INSTANCE);
		}
		HealthBuffer b = new HealthBuffer();
		b.record(UUID.randomUUID(), "Alex", 0, 20F);
		o.add("health", JournalEvents.health(b.snapshot(500, 0.0F)));
		return o;
	}

	@Test
	void journalIsAppendOnlyAndTruncatedLastLineIsIgnored(@TempDir Path dir) throws Exception {
		Path file = dir.resolve("runs/20/journal.jsonl");
		try (RunJournal j = new RunJournal(file)) {
			j.append(JournalEvents.event(JournalEvents.START, AT, 1000));
			j.append(JournalEvents.event(JournalEvents.GO, AT, 1000));
			JsonObject split = JournalEvents.event(JournalEvents.SPLIT, AT, 5000);
			split.addProperty("split", "DIAMOND");
			j.append(split);
			j.append(deathLine(61_000, "classroomrun/run_20/u/a.mcpr"));
		}
		// simulate a crash in the middle of the next line, then a restart that appends again
		Files.writeString(file, "{\"e\":\"damage\",\"at\":\"2026-10-3", StandardCharsets.UTF_8, java.nio.file.StandardOpenOption.APPEND);
		try (RunJournal j = new RunJournal(file)) {
			j.append(JournalEvents.event(JournalEvents.END, AT, 61_000));
		}
		List<String> lines = Files.readAllLines(file);
		assertEquals(6, lines.size());
		JournalReader.Parsed p = JournalReader.parse(20, lines);
		assertEquals(new RunSummary(20, "2026-10-30T18:00:00Z", 60_000, 1, "DIAMOND", false), p.summary());
		assertEquals(Set.of("classroomrun/run_20/u/a.mcpr"), p.replayPaths());
	}

	@Test
	void dragonEndMarksRunFinishedAndRunsWithoutGoHaveNoDuration() {
		JsonObject end = JournalEvents.event(JournalEvents.END, AT, 9000);
		end.addProperty("reason", "dragon");
		var p = JournalReader.parse(3, List.of(
				JournalEvents.event(JournalEvents.START, AT, 0).toString(), "garbage", "", "[1,2]", end.toString()));
		assertTrue(p.summary().finished());
		assertEquals(0, p.summary().durationMs());
		assertNull(p.summary().bestSplit());
	}

	@Test
	void storeBuildsIndexFromJournalsNewestFirstAndCollectsReplayPaths(@TempDir Path dir) throws Exception {
		for (int run : new int[] {2, 10, 9}) {
			try (RunJournal j = new RunJournal(dir.resolve(run + "/journal.jsonl"))) {
				j.append(JournalEvents.event(JournalEvents.START, AT, 0));
				if (run == 10) {
					j.append(deathLine(1000, "p/10.mcpr"));
				}
			}
		}
		Files.createDirectories(dir.resolve("not-a-run"));
		JournalStore store = new JournalStore(dir);
		assertEquals(List.of(10, 9, 2), store.summaries().stream().map(RunSummary::run).toList());
		assertEquals(Set.of("p/10.mcpr"), store.replayPaths());
		try (RunJournal j = new RunJournal(dir.resolve("9/journal.jsonl"))) { // cache invalidated by the new size
			j.append(deathLine(2000, "p/9.mcpr"));
		}
		assertEquals(Set.of("p/10.mcpr", "p/9.mcpr"), store.replayPaths());
	}

	// ---------------------------------------------------------------- JSON built for the payload

	@Test
	void indexIsCappedAndTruncatedFromTheOldestEnd() {
		List<RunSummary> runs = new ArrayList<>();
		for (int i = 300; i >= 1; i--) {
			runs.add(new RunSummary(i, "2026-10-30T18:00:00Z", 1000, 1, i % 2 == 0 ? "ENDER_EYE" : null, false));
		}
		JsonArray all = JsonParser.parseString(HistoryJson.index(runs, 262_144)).getAsJsonObject().getAsJsonArray("runs");
		assertEquals(200, all.size());
		assertEquals(300, all.get(0).getAsJsonObject().get("run").getAsInt());
		assertTrue(all.get(1).getAsJsonObject().get("bestSplit").isJsonNull());
		String small = HistoryJson.index(runs, 1000);
		assertTrue(small.length() <= 1000);
		JsonArray cut = JsonParser.parseString(small).getAsJsonObject().getAsJsonArray("runs");
		assertTrue(cut.size() > 0 && cut.size() < 200);
		assertEquals(300, cut.get(0).getAsJsonObject().get("run").getAsInt(), "the newest runs are kept");
	}

	@Test
	void detailFollowsTheContractAndDropsTheOldestDeathsWhenTooBig() {
		List<JsonObject> deaths = List.of(deathLine(1000, null), deathLine(2000, "a.mcpr"), deathLine(3000, "b.mcpr"));
		JsonObject root = JsonParser.parseString(HistoryJson.detail(20, "2026-10-30T18:00:00Z", deaths, 262_144)).getAsJsonObject();
		assertEquals(20, root.get("run").getAsInt());
		JsonArray arr = root.getAsJsonArray("deaths");
		assertEquals(3, arr.size());
		JsonObject d = arr.get(1).getAsJsonObject();
		assertEquals("Alex", d.get("fatalBy").getAsString());
		assertEquals(2000, d.getAsJsonObject("replay").get("offsetMs").getAsLong());
		assertTrue(arr.get(0).getAsJsonObject().get("replay").isJsonNull());
		assertFalse(d.has("e") || d.has("t"), "journal-only fields are not sent");
		assertEquals(0.0, d.getAsJsonArray("health").get(0).getAsJsonObject().getAsJsonArray("samples").get(1).getAsJsonArray().get(1).getAsDouble());

		int oneDeath = HistoryJson.detail(20, "x", deaths.subList(2, 3), 262_144).length();
		String cut = HistoryJson.detail(20, "x", deaths, oneDeath + 20);
		assertTrue(cut.length() <= oneDeath + 20);
		JsonArray kept = JsonParser.parseString(cut).getAsJsonObject().getAsJsonArray("deaths");
		assertEquals(1, kept.size());
		assertEquals("b.mcpr", kept.get(0).getAsJsonObject().getAsJsonObject("replay").get("path").getAsString());
		// absurdly small limit: still valid JSON
		assertTrue(JsonParser.parseString(HistoryJson.detail(20, "x", deaths, 30)).isJsonObject());
	}

	// ---------------------------------------------------------------- whitelist and rate limit

	@Test
	void replayRouteServesOnlyWhitelistedFilesInsideTheRoot(@TempDir Path tmp) throws Exception {
		Path root = tmp.resolve("recordings");
		Files.createDirectories(root.resolve("classroomrun/run_1"));
		Files.writeString(root.resolve("classroomrun/run_1/a.mcpr"), "x");
		Files.writeString(root.resolve("classroomrun/run_1/b.mcpr"), "y");
		Files.writeString(tmp.resolve("secret.txt"), "s");
		Set<String> white = Set.of("classroomrun/run_1/a.mcpr", "../secret.txt", "classroomrun/run_1/missing.mcpr");

		assertTrue(ReplayPaths.resolve(white, root, "classroomrun/run_1/a.mcpr").isPresent());
		assertEquals(Optional.empty(), ReplayPaths.resolve(white, root, "classroomrun/run_1/b.mcpr"), "not in a journal");
		assertEquals(Optional.empty(), ReplayPaths.resolve(white, root, "../../etc"), "not in a journal");
		assertEquals(Optional.empty(), ReplayPaths.resolve(white, root, "../secret.txt"), "whitelisted but outside the root");
		assertEquals(Optional.empty(), ReplayPaths.resolve(white, root, "classroomrun/run_1/missing.mcpr"));
		assertEquals(Optional.empty(), ReplayPaths.resolve(white, root, ""));
		assertEquals(Optional.empty(), ReplayPaths.resolve(white, root, null));
		assertEquals(Optional.empty(), ReplayPaths.resolve(Set.of("a\0b"), root, "a\0b"));
	}

	@Test
	void rateLimiterAllowsOnePerSecondPerPlayer() {
		RateLimiter r = new RateLimiter(1000);
		UUID a = UUID.randomUUID();
		UUID b = UUID.randomUUID();
		assertTrue(r.tryAcquire(a, 0));
		assertFalse(r.tryAcquire(a, 999));
		assertTrue(r.tryAcquire(b, 999));
		assertTrue(r.tryAcquire(a, 1000));
		r.forget(a);
		assertTrue(r.tryAcquire(a, 1001));
	}
}
