package fr.classroomrun.client.history;

import fr.classroomrun.client.history.HistoryModel.Death;
import fr.classroomrun.http.StatsHttpServer;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.IOException;
import java.net.URI;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Optional;

import static org.junit.jupiter.api.Assertions.*;

class HistoryClientLogicTest {
	private static final String DETAIL = """
			{"run": 20, "startedAt": "2026-10-30T18:00:00Z",
			 "deaths": [{
			   "at": "2026-10-30T18:59:00Z", "player": "uuid", "name": "Alex",
			   "cause": "minecraft:fall", "message": "Alex fell from a high place",
			   "attacker": null, "fatalBy": "Alex",
			   "pos": [12, 64, -30], "dimension": "classroomrun:run_20_overworld",
			   "replay": {"path": "classroomrun/run_20/u/a.mcpr", "offsetMs": 3540000},
			   "health": [{"name": "Alex", "samples": [[-10000, 20.0], [-500, 6.0], [0, 0.0]]}]
			 }, {"name": "Bob", "replay": null}]}
			""";

	@Test
	void parsesTheContractDetail() {
		HistoryModel.RunDetail d = HistoryModel.parseDetail(DETAIL);
		assertEquals(20, d.run());
		assertEquals(2, d.deaths().size());
		Death death = d.deaths().getFirst();
		assertEquals("Alex", death.name());
		assertNull(death.attacker());
		assertEquals("Alex", death.fatalBy());
		assertEquals(-30, death.z());
		assertEquals(new HistoryModel.ReplayRef("classroomrun/run_20/u/a.mcpr", 3_540_000L), death.replay());
		assertEquals(3, death.health().getFirst().samples().size());
		assertEquals(new HistoryModel.Sample(-500, 6.0F), death.health().getFirst().samples().get(1));
		assertNull(d.deaths().get(1).replay(), "partial entries get neutral defaults");
		assertEquals("", d.deaths().get(1).message());
	}

	@Test
	void parsesTheContractIndexAndSurvivesGarbage() {
		var rows = HistoryModel.parseIndex("""
				{"runs": [{"run": 20, "startedAt": "2026-10-30T18:00:00Z", "durationMs": 3600000,
				  "deaths": 1, "bestSplit": "ENDER_EYE", "finished": false}, {"run": 19, "bestSplit": null}, 7]}""");
		assertEquals(2, rows.size());
		assertEquals(new HistoryModel.RunRow(20, "2026-10-30T18:00:00Z", 3_600_000L, 1, "ENDER_EYE", false), rows.getFirst());
		assertNull(rows.get(1).bestSplit());
		assertTrue(HistoryModel.parseIndex("not json").isEmpty());
		assertTrue(HistoryModel.parseIndex("[1]").isEmpty());
		assertTrue(HistoryModel.parseDetail("{").deaths().isEmpty());
	}

	@Test
	void formatsAndExtractsTheHost() {
		assertEquals("1:00:05", HistoryModel.duration(3_605_000));
		assertEquals("not-a-date", HistoryModel.when("not-a-date"));
		assertEquals("play.example.com", HistoryModel.host("play.example.com"));
		assertEquals("play.example.com", HistoryModel.host("play.example.com:25565"));
		assertEquals("::1", HistoryModel.host("[::1]:25565"));
		assertEquals("2001:db8::1", HistoryModel.host("2001:db8::1"));
	}

	@Test
	void buildsUrlsAndSafeFileNames() {
		assertEquals(URI.create("http://host:25580/replay?path=classroomrun%2Frun_1%2Fa+b.mcpr"),
				ReplayDownloader.url("host", 25580, "classroomrun/run_1/a b.mcpr"));
		assertEquals("http://[::1]:25580/replay?path=x", ReplayDownloader.url("::1", 25580, "x").toString());
		assertEquals("classroomrun_run_1_a_b.mcpr", ReplayDownloader.fileName("classroomrun/run_1/a b.mcpr"));
		assertEquals("_.._x.mcpr", ReplayDownloader.fileName("/../x"));
	}

	@Test
	void downloadsThroughAPartFileAndRenames(@TempDir Path tmp) throws Exception {
		Path served = tmp.resolve("served.mcpr");
		byte[] content = new byte[200_000];
		new java.util.Random(7).nextBytes(content);
		Files.write(served, content);
		StatsHttpServer http = new StatsHttpServer("127.0.0.1", 0);
		http.setReplayResolver(raw -> "ok".equals(raw) ? Optional.of(served) : Optional.empty());
		http.start();
		try {
			Path target = tmp.resolve("replay_recordings/classroomrun/got.mcpr");
			ReplayDownloader.download(ReplayDownloader.url("127.0.0.1", http.port(), "ok"), target);
			assertArrayEquals(content, Files.readAllBytes(target));
			assertFalse(Files.exists(target.resolveSibling("got.mcpr.part")));

			Path missing = tmp.resolve("replay_recordings/classroomrun/missing.mcpr");
			assertThrows(IOException.class,
					() -> ReplayDownloader.download(ReplayDownloader.url("127.0.0.1", http.port(), "nope"), missing));
			assertFalse(Files.exists(missing));
			assertFalse(Files.exists(missing.resolveSibling("missing.mcpr.part")));
		} finally {
			http.stop();
		}
	}
}
