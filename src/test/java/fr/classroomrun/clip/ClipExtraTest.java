package fr.classroomrun.clip;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

class ClipExtraTest {
	@Test
	void swellRoundTripsToTheCreeperTickCount() {
		for (int ticks = 0; ticks <= ClipExtra.SWELL_TICKS; ticks++) {
			assertEquals(ticks, ClipExtra.swellTicks(ClipExtra.swellLevel(ticks / (float) ClipExtra.SWELL_TICKS)));
		}
		assertEquals(0, ClipExtra.swellLevel(-1f));
		assertEquals(127, ClipExtra.swellLevel(50f));
	}

	@Test
	void levelAndHandShareOneByte() {
		int packed = ClipExtra.pack(100, true);
		assertEquals(100, ClipExtra.level((byte) packed)); // a sign-extended byte still decodes
		assertTrue(ClipExtra.offHand((byte) packed));
		assertFalse(ClipExtra.offHand(ClipExtra.pack(100, false)));
		assertEquals(127, ClipExtra.pack(1000, false)); // masked, never spills into the hand bit
		assertEquals(80, ClipExtra.fuseLevel(80));
		assertEquals(127, ClipExtra.fuseLevel(500));
	}
}
