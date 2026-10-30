package fr.classroomrun;

import fr.classroomrun.net.Split;
import fr.classroomrun.stats.RunClock;
import fr.classroomrun.stats.RunStats;
import fr.classroomrun.stats.StatsStore;
import fr.classroomrun.sync.SharedVitals;
import fr.classroomrun.sync.SharedVitals.Vitals;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import org.slf4j.LoggerFactory;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class LogicTest {
	private static Vitals v(float health) {
		return new Vitals(health, 0, 20, 5);
	}

	private static SharedVitals full() {
		SharedVitals s = new SharedVitals();
		s.reset(v(20));
		return s;
	}

	@Test
	void damageOnOnePlayerIsSharedOnce() {
		assertEquals(17f, full().resolve(List.of(v(17), v(20), v(20))).health());
	}

	@Test
	void simultaneousHitsDoNotStack() {
		assertEquals(17f, full().resolve(List.of(v(17), v(18), v(20))).health());
	}

	@Test
	void simultaneousRegenHealsOnce() {
		SharedVitals s = new SharedVitals();
		s.reset(v(10));
		assertEquals(11f, s.resolve(List.of(v(11), v(11), v(10))).health());
	}

	@Test
	void lossWinsOverGainInTheSameTick() {
		SharedVitals s = new SharedVitals();
		s.reset(v(10));
		assertEquals(8f, s.resolve(List.of(v(14), v(8))).health());
	}

	@Test
	void absorptionAndFoodAreSharedIndependently() {
		SharedVitals s = full();
		Vitals r = s.resolve(List.of(new Vitals(20, 4, 20, 5), new Vitals(20, 0, 17, 0)));
		assertEquals(4f, r.absorption());
		assertEquals(17f, r.food());
		assertEquals(0f, r.saturation());
	}

	@Test
	void stableWhenNothingChanges() {
		SharedVitals s = full();
		s.resolve(List.of(v(15), v(20)));
		assertEquals(15f, s.resolve(List.of(v(15), v(15))).health());
		assertEquals(15f, s.baseline().health());
	}

	@Test
	void clockOnlyCountsRunningIntervals() {
		RunClock c = new RunClock(1000);
		c.tick(0, false);
		c.tick(50_000_000L, true);
		c.tick(150_000_000L, true);
		c.tick(10_000_000_000L, false);
		c.tick(10_100_000_000L, false);
		assertEquals(1100, c.totalTimeMs());
	}

	@Test
	void statsSplitsAndRuns() {
		RunStats s = new RunStats();
		s.newRun();
		assertEquals(1, s.runNumber);
		assertEquals(0, s.currentSplit());
		assertTrue(s.markSplit(Split.IRON_INGOT));
		assertFalse(s.markSplit(Split.IRON_INGOT));
		assertEquals(0, s.currentSplit());
		s.markSplit(Split.PICKAXE);
		assertEquals(2, s.currentSplit());
		assertEquals(1, s.bestSplit);
		s.newRun();
		assertEquals(0, s.reachedSplits);
		assertEquals(1, s.bestSplit);
		for (Split sp : Split.values()) {
			s.markSplit(sp);
		}
		assertTrue(s.finished());
	}

	@Test
	void storeRoundTripAndCorruption(@TempDir Path dir) throws Exception {
		Path file = dir.resolve("stats.json");
		RunStats s = new RunStats();
		s.newRun();
		s.recordDeath();
		s.totalTimeMs = 4242;
		StatsStore.save(file, s);
		RunStats read = StatsStore.load(file, LoggerFactory.getLogger("test"));
		assertEquals(4242, read.totalTimeMs);
		assertEquals(1, read.deaths);

		Files.writeString(file, "{not json");
		RunStats fresh = StatsStore.load(file, LoggerFactory.getLogger("test"));
		assertEquals(0, fresh.deaths);
		assertTrue(Files.exists(dir.resolve("stats.json.bak")));
	}
}
