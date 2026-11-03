package fr.classroomrun.game;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

class ClassroomGroundTest {
	@Test
	void floorSitsOnTheMedianGroundNotOnTheHighestTree() {
		// heightmap = first free block; one tree trunk (90) must not lift the room
		assertEquals(63, Classroom.medianGround(new int[] { 64, 64, 64, 65, 90 }));
		assertEquals(69, Classroom.medianGround(new int[] { 70 }));
	}
}
