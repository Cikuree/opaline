package fr.classroomrun.stats;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.List;
import java.util.Map;
import org.junit.jupiter.api.Test;

class DeathsByPlayerTest {
	@Test
	void topDeathsMostFirstAndWipeClears() {
		RunStats s = new RunStats();
		for (String n : List.of("Sam", "Alex", "Alex", "Lou", "Alex", "Sam", "Max")) {
			s.recordDeath(n);
		}
		assertEquals(List.of(Map.entry("Alex", 3), Map.entry("Sam", 2), Map.entry("Lou", 1)), s.topDeaths(3));
		assertEquals(7, s.deaths);
		s.wipe();
		assertTrue(s.topDeaths(3).isEmpty());
	}

	@Test
	void oldStatsWithoutTheMapAreRepaired() {
		RunStats s = new RunStats();
		s.deathsByPlayer = null; // file written before the field existed
		s.sanitize();
		assertTrue(s.topDeaths(3).isEmpty());
	}
}
