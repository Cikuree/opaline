package fr.classroomrun.client.clip;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;

class ClipLogicTest {
	@Test
	void unpacksAnglesSigned() {
		assertEquals(0f, ClipMath.unpackAngle((byte) 0), 1e-4f);
		assertEquals(90f, ClipMath.unpackAngle((byte) 64), 1e-4f);
		assertEquals(-90f, ClipMath.unpackAngle((byte) -64), 1e-4f);
		assertEquals(-180f, ClipMath.unpackAngle((byte) 128), 1e-4f);
	}

	@Test
	void lerpsAnglesAcrossTheWrap() {
		assertEquals(185f, ClipMath.lerpAngle(170f, -160f, 0.5f), 1e-3f); // 170 -> 200 by the short way
		assertEquals(10f, ClipMath.lerpAngle(0f, 20f, 0.5f), 1e-4f);
	}

	@Test
	void samplesAndInterpolatesPositions() {
		float[] pos = {0, 0, 0, 2, 4, 6};
		byte[] rot = {0, 0, 0, 64, 32, 64};
		float[] out = new float[ClipMath.OUT];
		assertTrue(ClipMath.sample(pos, rot, 3, 2, 3.5, out));
		assertEquals(1f, out[0], 1e-5f);
		assertEquals(2f, out[1], 1e-5f);
		assertEquals(3f, out[2], 1e-5f);
		assertEquals(45f, out[3], 1e-3f);
		assertEquals(22.5f, out[4], 1e-3f); // pitch 0 -> 45 deg, halfway
		assertTrue(ClipMath.sample(pos, rot, 3, 2, 4.0, out)); // last frame: held
		assertEquals(6f, out[2], 1e-5f);
	}

	@Test
	void actorIsAbsentOutsideItsFrames() {
		float[] pos = new float[6];
		byte[] rot = new byte[6];
		float[] out = new float[ClipMath.OUT];
		assertFalse(ClipMath.sample(pos, rot, 3, 2, 2.9, out));
		assertFalse(ClipMath.sample(pos, rot, 3, 2, 5.0, out));
		assertFalse(ClipMath.sample(pos, rot, 3, 0, 3.0, out));
	}

	@Test
	void detectsFlagEdgesInRange() {
		byte[] flags = {0, 0, 8, 0}; // SWING at local frame 2 = clip frame 12 (first = 10)
		assertTrue(ClipMath.anyFlag(flags, 10, 4, 10, 12, 8));
		assertFalse(ClipMath.anyFlag(flags, 10, 4, 12, 13, 8)); // exclusive lower bound
		assertFalse(ClipMath.anyFlag(flags, 10, 4, 0, 11, 8));
		assertTrue(ClipMath.anyFlag(flags, 10, 4, 0, 99, 8)); // clamped to the actor
	}

	@Test
	void timelineMapsTimeToFramesAndStopsOnTheDeathFrame() {
		ClipTimeline t = new ClipTimeline(11, 2); // 10 frames/s, 1 s long
		assertEquals(0.0, t.framePos(), 1e-9);
		t.advance(500);
		assertEquals(5.0, t.framePos(), 1e-9);
		assertEquals(0.5, t.fraction(), 1e-9);
		t.advance(2000);
		assertEquals(10.0, t.framePos(), 1e-9);
		assertTrue(t.atEnd());
		assertFalse(t.playing());
		assertEquals(1, t.totalSeconds());
	}

	@Test
	void timelineSpeedPauseSeekAndReplay() {
		ClipTimeline t = new ClipTimeline(11, 2);
		t.setSpeed(0.5f);
		t.advance(400);
		assertEquals(2.0, t.framePos(), 1e-9);
		t.togglePlaying();
		t.advance(400);
		assertEquals(2.0, t.framePos(), 1e-9); // paused
		t.seekFraction(2.0); // clamped
		assertTrue(t.atEnd());
		t.togglePlaying(); // at the end: replays from the start
		assertEquals(0.0, t.framePos(), 1e-9);
		assertTrue(t.playing());
	}

	@Test
	void singleFrameClipIsAlreadyAtItsEnd() {
		ClipTimeline t = new ClipTimeline(1, 2);
		assertEquals(1.0, t.fraction(), 1e-9);
		t.togglePlaying();
		t.advance(100);
		assertEquals(0.0, t.framePos(), 1e-9);
	}

	@Test
	void orbitLooksAtTheTargetFromBehind() {
		OrbitCamera o = new OrbitCamera(0f); // yaw 0 looks toward +z
		o.rotate(0, -25f / 0.4f); // pitch 25 -> 0
		o.update(10, 5, 20);
		assertEquals(10, o.x, 1e-6);
		assertEquals(5, o.y, 1e-6);
		assertEquals(20 - o.distance(), o.z, 1e-6);
		assertEquals(0f, o.xRot, 1e-4f);
	}

	@Test
	void orbitClampsZoomAndPitchAndStaysAtDistance() {
		OrbitCamera o = new OrbitCamera(37f);
		o.zoom(100);
		assertEquals(OrbitCamera.MIN_DISTANCE, o.distance());
		o.zoom(-100);
		assertEquals(OrbitCamera.MAX_DISTANCE, o.distance());
		o.rotate(0, 10_000);
		o.update(1, 2, 3);
		assertEquals(85f, o.xRot, 1e-4f);
		double d = Math.sqrt(Math.pow(o.x - 1, 2) + Math.pow(o.y - 2, 2) + Math.pow(o.z - 3, 2));
		assertEquals(OrbitCamera.MAX_DISTANCE, d, 1e-4);
		assertTrue(o.y > 2); // positive pitch: camera above the target
	}
}
