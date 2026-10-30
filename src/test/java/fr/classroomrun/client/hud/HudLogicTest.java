package fr.classroomrun.client.hud;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.UUID;
import org.junit.jupiter.api.Test;

class HudLogicTest {
	private static final UUID A = new UUID(0, 1);
	private static final UUID B = new UUID(0, 2);

	@Test
	void formatsHearts() {
		assertEquals("1", HudFormat.hearts(1f));
		assertEquals("2", HudFormat.hearts(2f));
		assertEquals("0.5", HudFormat.hearts(0.5f));
		assertEquals("1.5", HudFormat.hearts(1.5f));
		assertEquals("3", HudFormat.hearts(2.96f));
	}

	@Test
	void formatsClock() {
		assertEquals("0:00:00", HudFormat.clock(0));
		assertEquals("0:01:05", HudFormat.clock(65_999));
		assertEquals("2:14:00", HudFormat.clock((2 * 60 + 14) * 60_000L));
		assertEquals("0:00:00", HudFormat.clock(-5));
	}

	@Test
	void mergesQuickRehitAndRestartsAnimation() {
		DamageFeed f = new DamageFeed();
		f.add(A, 1f, false, 1000);
		f.add(A, 1f, false, 1400);
		DamageFeed.Entry[] out = new DamageFeed.Entry[DamageFeed.CAPACITY];
		assertEquals(1, f.ordered(out));
		assertEquals("-2", out[0].text);
		assertEquals(1400, out[0].startMs);
	}

	@Test
	void doesNotMergeAfterWindowOrAcrossPlayersOrKinds() {
		DamageFeed f = new DamageFeed();
		DamageFeed.Entry[] out = new DamageFeed.Entry[DamageFeed.CAPACITY];
		f.add(A, 1f, false, 1000);
		f.add(A, 1f, false, 1500);
		f.add(B, 1f, false, 1500);
		f.add(A, 1f, true, 1500);
		assertEquals(4, f.ordered(out));
		assertEquals("+1", out[3].text);
	}

	@Test
	void expires() {
		DamageFeed f = new DamageFeed();
		DamageFeed.Entry[] out = new DamageFeed.Entry[DamageFeed.CAPACITY];
		f.add(A, 1f, false, 1000);
		f.expire(2499);
		assertEquals(1, f.ordered(out));
		f.expire(2500);
		assertEquals(0, f.ordered(out));
	}

	@Test
	void recyclesOldestWhenFull() {
		DamageFeed f = new DamageFeed();
		DamageFeed.Entry[] out = new DamageFeed.Entry[DamageFeed.CAPACITY];
		for (int i = 0; i < DamageFeed.CAPACITY + 1; i++) {
			f.add(new UUID(0, 100 + i), 1f, false, 1000);
		}
		assertEquals(DamageFeed.CAPACITY, f.ordered(out));
		assertFalse(out[0].player.equals(new UUID(0, 100)));
		assertEquals(new UUID(0, 100 + DamageFeed.CAPACITY), out[DamageFeed.CAPACITY - 1].player);
	}

	@Test
	void fadeAndRise() {
		assertEquals(1f, DamageFeed.alpha(0.2f));
		assertEquals(0f, DamageFeed.alpha(1f));
		assertTrue(DamageFeed.alpha(0.7f) < 1f);
		assertEquals(12f, DamageFeed.rise(1f, 12f));
	}

	@Test
	void runClockExtrapolatesAndCachesText() {
		ClientRunState s = new ClientRunState();
		assertFalse(s.present());
		s.update(10_000, true, 7, 3, 4, 0b1111, 5_000);
		assertTrue(s.present());
		assertTrue(s.reached(3));
		assertFalse(s.reached(4));
		assertEquals(12_500, s.elapsedMs(7_500));
		String t = s.clockText(7_500);
		assertEquals("0:00:12", t);
		assertSame(t, s.clockText(7_900));
		s.update(10_000, false, 7, 3, 4, 0b1111, 5_000);
		assertEquals(10_000, s.elapsedMs(99_999));
		s.reset();
		assertFalse(s.present());
	}
}
