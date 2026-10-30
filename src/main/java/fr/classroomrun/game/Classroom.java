package fr.classroomrun.game;

import fr.classroomrun.ClassroomRun;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Vec3i;
import net.minecraft.core.registries.Registries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.NbtAccounter;
import net.minecraft.nbt.NbtIo;
import net.minecraft.network.chat.Component;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.Display;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.entity.EntityTypeTest;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.level.levelgen.structure.templatesystem.StructurePlaceSettings;
import net.minecraft.world.level.levelgen.structure.templatesystem.StructureTemplate;
import net.minecraft.world.phys.Vec3;
import org.jspecify.annotations.Nullable;

import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.UUID;

/**
 * Builds the classroom in the run Overworld. Template source, by priority: {@code config/classroomrun/classroom.nbt},
 * then the jar resource {@code data/classroomrun/structure/classroom.nbt}, then a fallback platform.
 *
 * <p>Markers in the template: an entity {@code minecraft:marker} tagged {@code classroomrun:spawn} (player spawn,
 * removed after placement) and a {@code minecraft:text_display} tagged {@code classroomrun:board} (run board).
 */
public final class Classroom {
	public static final String SPAWN_TAG = "classroomrun:spawn";
	public static final String BOARD_TAG = "classroomrun:board";
	private static final String RESOURCE = "/data/classroomrun/structure/classroom.nbt";
	private static final int FALLBACK_SIZE = 15;
	/** Spawn used when the template has no spawn marker, relative to the template origin. */
	private static final Vec3 DEFAULT_SPAWN_OFFSET = new Vec3(7.5, 1.0, 6.5);
	private static final int MAX_FOUNDATION_DEPTH = 48;

	/** @param spawn spawn position of the players; @param boardId UUID of the board text_display, if any */
	public record Placed(ServerLevel level, Vec3 spawn, @Nullable UUID boardId) {
	}

	private Classroom() {
	}

	public static Placed build(MinecraftServer server, ServerLevel level) {
		StructureTemplate template = loadTemplate(server);
		BlockPos spawnXZ = level.getRespawnData().pos();
		Vec3i size = template != null ? template.getSize() : new Vec3i(FALLBACK_SIZE, 4, FALLBACK_SIZE);
		int ox = spawnXZ.getX() - size.getX() / 2;
		int oz = spawnXZ.getZ() - size.getZ() / 2;

		// Generate the chunks under the footprint, then find the ground height with the heightmap.
		for (int cx = ox >> 4; cx <= (ox + size.getX() - 1) >> 4; cx++) {
			for (int cz = oz >> 4; cz <= (oz + size.getZ() - 1) >> 4; cz++) {
				level.getChunk(cx, cz);
			}
		}
		int oy = level.getMinY() + 1;
		for (int x = ox; x < ox + size.getX(); x++) {
			for (int z = oz; z < oz + size.getZ(); z++) {
				oy = Math.max(oy, level.getHeight(Heightmap.Types.MOTION_BLOCKING_NO_LEAVES, x, z));
			}
		}
		oy = Math.min(oy, level.getMaxY() - size.getY());
		BlockPos origin = new BlockPos(ox, oy, oz);

		fillFoundation(level, ox, oy, oz, size);
		clearVolume(level, origin, size);

		if (template == null) {
			buildFallbackPlatform(level, origin);
			return new Placed(level, Vec3.atBottomCenterOf(origin.offset(FALLBACK_SIZE / 2, 1, FALLBACK_SIZE / 2)), null);
		}

		StructurePlaceSettings settings = new StructurePlaceSettings().setIgnoreEntities(false);
		template.placeInWorld(level, origin, origin, settings, level.getRandom(), Block.UPDATE_CLIENTS);

		Vec3 spawn = Vec3.atLowerCornerOf(origin).add(DEFAULT_SPAWN_OFFSET);
		List<? extends Entity> markers = level.getEntities(EntityTypeTest.forClass(Entity.class), e -> e.entityTags().contains(SPAWN_TAG));
		if (markers.isEmpty()) {
			ClassroomRun.LOGGER.warn("Classroom template has no entity tagged {}, using the default spawn", SPAWN_TAG);
		} else {
			Entity marker = markers.getFirst();
			spawn = marker.position();
			markers.forEach(Entity::discard);
		}
		UUID boardId = null;
		List<? extends Entity> boards = level.getEntities(EntityTypeTest.forClass(Entity.class), e -> e.entityTags().contains(BOARD_TAG));
		if (!boards.isEmpty()) {
			boardId = boards.getFirst().getUUID();
		} else {
			ClassroomRun.LOGGER.warn("Classroom template has no entity tagged {}", BOARD_TAG);
		}
		return new Placed(level, spawn, boardId);
	}

	/** Sets the board text: "Run #N - M morts - meilleur split : name". No-op if the board is not loaded. */
	public static void updateBoard(@Nullable Placed placed, String text) {
		if (placed == null || placed.boardId() == null) {
			return;
		}
		if (placed.level().getEntity(placed.boardId()) instanceof Display.TextDisplay board) {
			board.setText(Component.literal(text));
		}
	}

	private static @Nullable StructureTemplate loadTemplate(MinecraftServer server) {
		Path external = FabricLoader.getInstance().getConfigDir().resolve("classroomrun").resolve("classroom.nbt");
		try {
			CompoundTag tag;
			if (Files.isRegularFile(external)) {
				try (InputStream in = Files.newInputStream(external)) {
					tag = NbtIo.readCompressed(in, NbtAccounter.unlimitedHeap());
				}
			} else {
				try (InputStream in = Classroom.class.getResourceAsStream(RESOURCE)) {
					if (in == null) {
						ClassroomRun.LOGGER.warn("No classroom template ({} or jar resource), using the fallback platform", external);
						return null;
					}
					tag = NbtIo.readCompressed(in, NbtAccounter.unlimitedHeap());
				}
			}
			StructureTemplate template = new StructureTemplate();
			template.load(server.registryAccess().lookupOrThrow(Registries.BLOCK), tag);
			return template;
		} catch (IOException | RuntimeException e) {
			ClassroomRun.LOGGER.warn("Cannot load the classroom template, using the fallback platform", e);
			return null;
		}
	}

	/** Fills (stone) the gap between the ground and the room floor so the room does not hover on slopes or water. */
	private static void fillFoundation(ServerLevel level, int ox, int oy, int oz, Vec3i size) {
		for (int x = ox; x < ox + size.getX(); x++) {
			for (int z = oz; z < oz + size.getZ(); z++) {
				int ground = level.getHeight(Heightmap.Types.OCEAN_FLOOR, x, z);
				for (int y = Math.max(ground, oy - MAX_FOUNDATION_DEPTH); y < oy; y++) {
					level.setBlock(new BlockPos(x, y, z), Blocks.STONE.defaultBlockState(), Block.UPDATE_CLIENTS);
				}
			}
		}
	}

	private static void clearVolume(ServerLevel level, BlockPos origin, Vec3i size) {
		BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
		for (int x = 0; x < size.getX(); x++) {
			for (int y = 0; y < size.getY(); y++) {
				for (int z = 0; z < size.getZ(); z++) {
					pos.set(origin.getX() + x, origin.getY() + y, origin.getZ() + z);
					if (!level.getBlockState(pos).isAir()) {
						level.setBlock(pos, Blocks.AIR.defaultBlockState(), Block.UPDATE_CLIENTS);
					}
				}
			}
		}
	}

	private static void buildFallbackPlatform(ServerLevel level, BlockPos origin) {
		for (int x = 0; x < FALLBACK_SIZE; x++) {
			for (int z = 0; z < FALLBACK_SIZE; z++) {
				level.setBlock(origin.offset(x, 0, z), Blocks.STONE_BRICKS.defaultBlockState(), Block.UPDATE_CLIENTS);
			}
		}
	}
}
