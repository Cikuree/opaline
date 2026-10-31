package fr.classroomrun.client.history;

import fr.classroomrun.client.history.HistoryModel.Death;
import org.junit.jupiter.api.Test;


import static org.junit.jupiter.api.Assertions.*;

class HistoryClientLogicTest {
	private static final String DETAIL = """
			{"run": 20, "startedAt": "2026-10-30T18:00:00Z",
			 "deaths": [{
			   "at": "2026-10-30T18:59:00Z", "player": "uuid", "name": "Alex",
			   "cause": "minecraft:fall", "message": "Alex fell from a high place",
			   "attacker": null, "fatalBy": "Alex",
			   "pos": [12, 64, -30], "dimension": "classroomrun:run_20_overworld",
			   "health": [{"name": "Alex", "samples": [[-10000, 20.0], [-500, 6.0], [0, 0.0]]}]
			 }, {"name": "Bob"}]}
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
		assertEquals(3, death.health().getFirst().samples().size());
		assertEquals(new HistoryModel.Sample(-500, 6.0F), death.health().getFirst().samples().get(1));
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
	void formatsDurationAndDate() {
		assertEquals("1:00:05", HistoryModel.duration(3_605_000));
		assertEquals("not-a-date", HistoryModel.when("not-a-date"));
	}
}
