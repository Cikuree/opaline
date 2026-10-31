package fr.classroomrun.game;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;

import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import org.junit.jupiter.api.Test;

class ClassroomPaletteTest {
	@Test
	void legacyKeysAreRenamed() {
		CompoundTag props = new CompoundTag();
		props.putString("axis", "y");
		CompoundTag entry = new CompoundTag();
		entry.putString("Name", "minecraft:oak_log");
		entry.put("Properties", props);
		ListTag palette = new ListTag();
		palette.add(entry);
		CompoundTag tag = new CompoundTag();
		tag.put("palette", palette);

		Classroom.upgradePalette(tag);

		CompoundTag out = tag.getListOrEmpty("palette").getCompoundOrEmpty(0);
		assertEquals("minecraft:oak_log", out.getStringOr("id", ""));
		assertEquals("y", out.getCompoundOrEmpty("properties").getStringOr("axis", ""));
		assertFalse(out.contains("Name"));
		assertFalse(out.contains("Properties"));
	}
}
