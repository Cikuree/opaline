package fr.classroomrun.journal;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import fr.classroomrun.net.HistoryPayload;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.time.Instant;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;
import java.util.UUID;

import static org.junit.jupiter.api.Assertions.*;

/** Journal append-only behaviour and exact JSON schemas of CONTRACT.md ("Historique des morts"). */
class JournalContractTest {
	private static final Instant AT = Instant.parse("2026-10-30T18:00:00Z");

	private static JsonObject death(long t) {
		JsonObject o = JournalEvents.event(JournalEvents.DEATH, AT, t);
		o.addProperty("player", "uuid");
		o.addProperty("name", "Alex");
		o.addProperty("cause", "minecraft:fall");
		o.addProperty("message", "Alex fell from a high place");
		o.add("attacker", com.google.gson.JsonNull.INSTANCE);
		o.addProperty("fatalBy", "Alex");
		o.add("pos", JournalEvents.pos(12, 64, -30));
		o.addProperty("dimension", "classroomrun:run_20_overworld");
		HealthBuffer b = new HealthBuffer();
		b.record(UUID.randomUUID(), "Alex", 0, 20F);
		o.add("health", JournalEvents.health(b.snapshot(500, 0.0F)));
		return o;
	}

	private static Set<String> keys(JsonObject o) {
		return o.keySet();
	}

	@Test
	void reopeningKeepsExistingLinesAndAppendsAfterThem(@TempDir Path dir) throws Exception {
		Path f = dir.resolve("1/journal.jsonl");
		try (RunJournal j = new RunJournal(f)) {
			j.append(JournalEvents.event(JournalEvents.START, AT, 0));
		}
		String before = Files.readString(f);
		try (RunJournal j = new RunJournal(f)) {
			j.append(JournalEvents.event(JournalEvents.GO, AT, 1));
		}
		List<String> lines = Files.readAllLines(f);
		assertEquals(2, lines.size());
		assertTrue(Files.readString(f).startsWith(before), "existing bytes never rewritten");
		// several independent runs do not interfere
		for (int run = 2; run <= 4; run++) {
			try (RunJournal j = new RunJournal(dir.resolve(run + "/journal.jsonl"))) {
				j.append(JournalEvents.event(JournalEvents.START, AT, run));
			}
		}
		assertEquals(List.of(4, 3, 2, 1), new JournalStore(dir).summaries().stream().map(RunSummary::run).toList());
	}

	@Test
	void lineAfterATruncatedTailIsParsedCorrectly(@TempDir Path dir) throws Exception {
		Path f = dir.resolve("5/journal.jsonl");
		try (RunJournal j = new RunJournal(f)) {
			j.append(JournalEvents.event(JournalEvents.START, AT, 0));
		}
		Files.writeString(f, "{\"e\":\"death\",\"at\":\"2026", StandardCharsets.UTF_8, StandardOpenOption.APPEND);
		List<JsonObject> parsedBefore = Files.readAllLines(f).stream().map(JournalReader::parseLine).filter(java.util.Objects::nonNull).toList();
		assertEquals(1, parsedBefore.size(), "truncated tail ignored");
		try (RunJournal j = new RunJournal(f)) {
			j.append(death(7000));
		}
		List<String> lines = Files.readAllLines(f);
		assertEquals(3, lines.size());
		assertNull(JournalReader.parseLine(lines.get(1)), "garbage stays on its own line");
		JsonObject last = JournalReader.parseLine(lines.get(2));
		assertNotNull(last);
		assertEquals("death", last.get("e").getAsString());
		assertEquals(1, JournalReader.parse(5, lines).summary().deaths());
	}

	@Test
	void indexJsonHasExactKeysAndTypes() {
		var runs = List.of(new RunSummary(20, "2026-10-30T18:00:00Z", 3_600_000, 1, "ENDER_EYE", false, List.of()),
				new RunSummary(19, "2026-10-30T17:00:00Z", 0, 0, null, true, List.of()));
		JsonObject root = JsonParser.parseString(HistoryJson.index(runs, HistoryPayload.MAX_JSON)).getAsJsonObject();
		assertEquals(Set.of("runs"), keys(root));
		for (JsonElement e : root.getAsJsonArray("runs")) {
			JsonObject r = e.getAsJsonObject();
			assertEquals(Set.of("run", "startedAt", "durationMs", "deaths", "bestSplit", "finished", "victims"), keys(r));
			assertTrue(r.get("victims").isJsonArray());
			assertTrue(r.get("run").getAsJsonPrimitive().isNumber());
			assertTrue(r.get("startedAt").getAsJsonPrimitive().isString());
			assertTrue(r.get("durationMs").getAsJsonPrimitive().isNumber());
			assertTrue(r.get("deaths").getAsJsonPrimitive().isNumber());
			assertTrue(r.get("finished").getAsJsonPrimitive().isBoolean());
		}
		assertEquals("ENDER_EYE", root.getAsJsonArray("runs").get(0).getAsJsonObject().get("bestSplit").getAsString());
		assertTrue(root.getAsJsonArray("runs").get(1).getAsJsonObject().get("bestSplit").isJsonNull());
	}

	@Test
	void detailJsonHasExactKeysAndTypes() {
		JsonObject root = JsonParser.parseString(HistoryJson.detail(20, "2026-10-30T18:00:00Z",
				List.of(death(5000), death(6000)), HistoryPayload.MAX_JSON)).getAsJsonObject();
		assertEquals(Set.of("run", "startedAt", "deaths"), keys(root));
		assertTrue(root.get("run").getAsJsonPrimitive().isNumber());
		JsonObject d = root.getAsJsonArray("deaths").get(0).getAsJsonObject();
		assertEquals(Set.of("at", "player", "name", "cause", "message", "attacker", "fatalBy", "pos", "dimension",
				"health"), keys(d));
		assertTrue(d.get("attacker").isJsonNull());
		JsonArray pos = d.getAsJsonArray("pos");
		assertEquals(3, pos.size());
		assertTrue(pos.get(0).getAsJsonPrimitive().isNumber());
		JsonObject h = d.getAsJsonArray("health").get(0).getAsJsonObject();
		assertEquals(Set.of("name", "samples"), keys(h));
		for (JsonElement s : h.getAsJsonArray("samples")) {
			JsonArray pair = s.getAsJsonArray();
			assertEquals(2, pair.size());
			assertTrue(pair.get(0).getAsDouble() <= 0);
		}
	}

	@Test
	void legacyJournalWithReplayFieldStillReadsAndTheFieldIsNotSent() {
		JsonObject legacy = death(5000);
		JsonObject replay = new JsonObject();
		replay.addProperty("path", "classroomrun/run_20/u/a.mcpr");
		replay.addProperty("offsetMs", 5000);
		legacy.add("replay", replay);
		var parsed = JournalReader.parse(20, List.of(JournalEvents.event(JournalEvents.START, AT, 0).toString(), legacy.toString()));
		assertEquals(1, parsed.summary().deaths());
		JsonObject d = JsonParser.parseString(HistoryJson.detail(20, "s", parsed.deaths(), HistoryPayload.MAX_JSON))
				.getAsJsonObject().getAsJsonArray("deaths").get(0).getAsJsonObject();
		assertFalse(d.has("replay"));
	}

	@Test
	void payloadJsonIsCappedAtMaxJson() {
		assertEquals(262_144, HistoryPayload.MAX_JSON);
		List<RunSummary> runs = new ArrayList<>();
		for (int i = 500; i >= 1; i--) {
			runs.add(new RunSummary(i, "2026-10-30T18:00:00Z", 1, 1, "ENDER_EYE", false, List.of()));
		}
		assertTrue(HistoryJson.index(runs, HistoryPayload.MAX_JSON).length() <= HistoryPayload.MAX_JSON);
		List<JsonObject> deaths = new ArrayList<>();
		for (int i = 0; i < 3000; i++) {
			JsonObject d = death(i);
			d.addProperty("message", "x".repeat(100) + i);
			deaths.add(d);
		}
		String json = HistoryJson.detail(20, "s", deaths, HistoryPayload.MAX_JSON);
		assertTrue(json.length() <= HistoryPayload.MAX_JSON, "len " + json.length());
		JsonArray kept = JsonParser.parseString(json).getAsJsonObject().getAsJsonArray("deaths");
		assertTrue(kept.size() > 0 && kept.size() < 3000);
		assertTrue(kept.get(kept.size() - 1).getAsJsonObject().get("message").getAsString().endsWith("2999"), "newest death kept");
	}

	@Test
	void rateLimiterBoundaryAndRefusedCallsDoNotExtendTheWindow() {
		RateLimiter r = new RateLimiter(1000);
		UUID a = UUID.randomUUID();
		assertTrue(r.tryAcquire(a, 5000));
		assertFalse(r.tryAcquire(a, 5500));
		assertFalse(r.tryAcquire(a, 5999));
		assertTrue(r.tryAcquire(a, 6000), "refused calls must not reset the window");
	}
}
