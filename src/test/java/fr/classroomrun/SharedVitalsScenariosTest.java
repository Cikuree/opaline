package fr.classroomrun;

import fr.classroomrun.sync.SharedVitals;
import fr.classroomrun.sync.SharedVitals.Vitals;
import org.junit.jupiter.api.Test;

import java.util.Collections;
import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/** Multi-source scenarios on the pure team-vitals logic (damage, regeneration, totem aftermath, propagation). */
class SharedVitalsScenariosTest {
	private static Vitals v(float health) {
		return new Vitals(health, 0, 20, 5);
	}

	private static SharedVitals sharedAt(float health) {
		SharedVitals s = new SharedVitals();
		s.reset(v(health));
		return s;
	}

	/** Everybody takes the shared value, as TeamSync.tick does after resolve. */
	private static List<Vitals> after(Vitals shared, int players) {
		return Collections.nCopies(players, shared);
	}

	@Test
	void twoPlayersHitSameTickKeepOnlyTheBiggestLoss() {
		SharedVitals s = sharedAt(20);
		assertEquals(14f, s.resolve(List.of(v(14), v(17), v(20), v(20))).health());
	}

	@Test
	void manySourcesSameTickStillMinNotSum() {
		SharedVitals s = sharedAt(20);
		assertEquals(10f, s.resolve(List.of(v(18), v(16), v(14), v(12), v(10), v(20))).health());
	}

	@Test
	void hitsOnConsecutiveTicksDoAccumulate() {
		SharedVitals s = sharedAt(20);
		Vitals t1 = s.resolve(List.of(v(17), v(20)));
		Vitals t2 = s.resolve(List.of(v(t1.health() - 2), v(t1.health())));
		assertEquals(15f, t2.health());
	}

	@Test
	void lossAndGainSameTickLossWinsAndGainIsLost() {
		SharedVitals s = sharedAt(10);
		Vitals r = s.resolve(List.of(v(13), v(9), v(10)));
		assertEquals(9f, r.health());
		// next tick everybody is at 9: the +3 of the first player is gone, not applied later
		assertEquals(9f, s.resolve(after(r, 3)).health());
	}

	@Test
	void lossOnHealthAndGainOnAbsorptionSameTickAreIndependent() {
		SharedVitals s = sharedAt(20);
		Vitals r = s.resolve(List.of(new Vitals(16, 0, 20, 5), new Vitals(20, 4, 20, 5)));
		assertEquals(16f, r.health());
		assertEquals(4f, r.absorption());
	}

	@Test
	void lossOnFoodAndGainOnHealthSameTickAreIndependent() {
		SharedVitals s = sharedAt(10);
		Vitals r = s.resolve(List.of(new Vitals(12, 0, 20, 5), new Vitals(10, 0, 14, 5)));
		assertEquals(12f, r.health());
		assertEquals(14f, r.food());
	}

	@Test
	void nPlayersRegeneratingHealOncePerTick() {
		for (int n : new int[] {2, 5, 20}) {
			SharedVitals s = sharedAt(10);
			assertEquals(11f, s.resolve(Collections.nCopies(n, v(11))).health(), "n=" + n);
		}
	}

	@Test
	void nPlayersRegeneratingOverManyTicksMatchesOnePlayer() {
		SharedVitals team = sharedAt(4);
		float shared = 4;
		for (int tick = 0; tick < 6; tick++) {
			shared = team.resolve(Collections.nCopies(8, v(shared + 1))).health();
		}
		assertEquals(10f, shared);
	}

	@Test
	void unevenRegenTakesTheMaxNotTheSum() {
		assertEquals(13f, sharedAt(10).resolve(List.of(v(11), v(13), v(12), v(10))).health());
	}

	@Test
	void propagationDoesNotRetrigger() {
		SharedVitals s = sharedAt(20);
		Vitals r = s.resolve(List.of(v(15), v(20), v(20)));
		for (int tick = 0; tick < 50; tick++) {
			r = s.resolve(after(r, 3));
			assertEquals(15f, r.health());
		}
	}

	@Test
	void joinerSetToBaselineDoesNotChangeTheTeam() {
		SharedVitals s = sharedAt(12);
		// TeamSync.onJoin applies the baseline to the joiner before the next resolve
		assertEquals(12f, s.resolve(List.of(v(12), v(12), s.baseline())).health());
	}

	@Test
	void totemAftermathTeamGoesToHalfHeartAndSharesAbsorption() {
		SharedVitals s = sharedAt(20);
		// victim consumed a totem: 1 HP + absorption 8, the others are untouched
		Vitals r = s.resolve(List.of(new Vitals(1, 8, 20, 5), v(20), v(20)));
		assertEquals(1f, r.health());
		assertEquals(8f, r.absorption());
		assertEquals(r, s.resolve(after(r, 3)));
	}

	@Test
	void totemVictimAndTeammateHitSameTickMinWins() {
		SharedVitals s = sharedAt(20);
		assertEquals(1f, s.resolve(List.of(new Vitals(1, 8, 20, 5), v(5))).health());
	}

	@Test
	void hungerSharedAsMin() {
		SharedVitals s = sharedAt(20);
		Vitals r = s.resolve(List.of(new Vitals(20, 0, 18, 5), new Vitals(20, 0, 19, 5)));
		assertEquals(18f, r.food());
	}

	@Test
	void changesBelowEpsilonAreIgnored() {
		SharedVitals s = sharedAt(20);
		assertEquals(20f, s.resolve(List.of(v(19.9995f), v(20))).health());
	}

	@Test
	void firstResolveWithoutBaselineAdoptsFirstPlayerThenMerges() {
		SharedVitals s = new SharedVitals();
		assertNull(s.baseline());
		assertEquals(8f, s.resolve(List.of(v(8), v(8), v(8))).health());
		assertEquals(6f, s.resolve(List.of(v(6), v(8))).health());
	}

	@Test
	void resetAndClear() {
		SharedVitals s = sharedAt(5);
		s.reset(v(20));
		assertEquals(20f, s.baseline().health());
		s.clear();
		assertNull(s.baseline());
	}

	@Test
	void emptyTeamIsRejected() {
		assertThrows(IllegalArgumentException.class, () -> sharedAt(20).resolve(List.of()));
	}
}
