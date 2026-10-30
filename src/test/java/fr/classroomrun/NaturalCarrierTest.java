package fr.classroomrun;

import fr.classroomrun.net.Split;
import fr.classroomrun.stats.RunStats;
import fr.classroomrun.sync.NaturalCarrier;
import org.junit.jupiter.api.Test;

import java.util.List;
import java.util.Set;
import java.util.UUID;

import static org.junit.jupiter.api.Assertions.*;

class NaturalCarrierTest {
	private static final UUID A = new UUID(0, 1);
	private static final UUID B = new UUID(0, 2);

	@Test
	void keepsCurrentWhileMember() {
		assertEquals(B, NaturalCarrier.select(Set.of(A, B), B));
	}

	@Test
	void fallsBackToSmallestUuid() {
		assertEquals(A, NaturalCarrier.select(Set.of(B, A), new UUID(9, 9)));
		assertEquals(A, NaturalCarrier.select(Set.of(B, A), null));
		assertNull(NaturalCarrier.select(List.of(), A));
	}

	@Test
	void exhaustionIsDividedByTeamSize() {
		assertEquals(0.5f, NaturalCarrier.share(2f, 4));
		assertEquals(2f, NaturalCarrier.share(2f, 0));
	}

	@Test
	void dragonKillFinishesTheRunEvenWithSkippedSplits() {
		RunStats s = new RunStats();
		s.markSplit(Split.KILL_DRAGON);
		assertTrue(s.finished());
		assertEquals(Split.values().length, s.currentSplit());
	}

	@Test
	void sanitizeClampsOutOfRangeValues() {
		RunStats s = new RunStats();
		s.bestSplit = 99;
		s.reachedSplits = -1;
		s.deaths = -4;
		s.sanitize();
		assertEquals(Split.values().length - 1, s.bestSplit);
		assertEquals((1 << Split.values().length) - 1, s.reachedSplits);
		assertEquals(0, s.deaths);
	}
}
