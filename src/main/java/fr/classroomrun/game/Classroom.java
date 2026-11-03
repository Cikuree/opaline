package fr.classroomrun.game;

import fr.classroomrun.ClassroomRun;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Vec3i;
import net.minecraft.core.registries.Registries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.NbtAccounter;
import net.minecraft.nbt.NbtIo;
import net.minecraft.nbt.DoubleTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.network.chat.Component;
import net.minecraft.util.ProblemReporter;
import net.minecraft.world.entity.EntitySpawnReason;
import net.minecraft.world.entity.EntitySpawnRequest;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.level.storage.TagValueInput;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.tags.BiomeTags;
import net.minecraft.world.entity.Display;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.level.levelgen.structure.templatesystem.StructurePlaceSettings;
import net.minecraft.world.level.levelgen.structure.templatesystem.StructureTemplate;
import net.minecraft.world.phys.Vec3;
import org.jspecify.annotations.Nullable;

import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Optional;

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
	private static final int DRY_SEARCH_RADIUS = 1024;
	/** Surface checks generate chunks: bounded so a reset never stalls the server for long. */
	private static final int MAX_DRY_CHECKS = 24;

	/**
	 * @param spawn   spawn position of the players
	 * @param board   the board text_display (kept loaded through forced chunks), or null
	 */
	public record Placed(ServerLevel level, Vec3 spawn, Display.@Nullable TextDisplay board) {
	}

	/** Template + what was read from its NBT before loading: spawn (relative), board entry (relative pos kept inside). */
	private record Loaded(StructureTemplate template, String source, @Nullable Vec3 spawn, @Nullable CompoundTag boardEntry) {
	}

	private Classroom() {
	}

	public static Placed build(MinecraftServer server, ServerLevel level) {
		Loaded loaded = loadTemplate(server);
		StructureTemplate template = loaded != null ? loaded.template() : null;
		Vec3i size = template != null ? template.getSize() : new Vec3i(FALLBACK_SIZE, 4, FALLBACK_SIZE);
		BlockPos center = dryCenter(level, level.getRespawnData().pos(), size);
		int ox = center.getX() - size.getX() / 2;
		int oz = center.getZ() - size.getZ() / 2;
		for (int cx = ox >> 4; cx <= (ox + size.getX() - 1) >> 4; cx++) {
			for (int cz = oz >> 4; cz <= (oz + size.getZ() - 1) >> 4; cz++) {
				level.getChunk(cx, cz); // generated before reading the heightmap
				level.setChunkForced(cx, cz, true); // board kept loaded
			}
		}
		// Floor flush with the ground: median height of the footprint (the volume above is cleared, the gaps below are filled).
		int[] heights = new int[size.getX() * size.getZ()];
		int n = 0;
		for (int x = ox; x < ox + size.getX(); x++) {
			for (int z = oz; z < oz + size.getZ(); z++) {
				heights[n++] = level.getHeight(Heightmap.Types.MOTION_BLOCKING_NO_LEAVES, x, z);
			}
		}
		int oy = Math.clamp(medianGround(heights), level.getMinY() + 1, level.getMaxY() - size.getY());
		BlockPos origin = new BlockPos(ox, oy, oz);

		fillFoundation(level, ox, oy, oz, size);
		clearVolume(level, origin, size);

		if (loaded == null) {
			buildFallbackPlatform(level, origin);
			ClassroomRun.LOGGER.info("Classroom placed at {} from source: fallback platform", origin);
			return new Placed(level, Vec3.atBottomCenterOf(origin.offset(FALLBACK_SIZE / 2, 1, FALLBACK_SIZE / 2)), null);
		}

		StructurePlaceSettings settings = new StructurePlaceSettings().setIgnoreEntities(false);
		boolean ok = template.placeInWorld(level, origin, origin, settings, level.getRandom(), Block.UPDATE_CLIENTS);
		int solid = 0;
		for (BlockPos p : BlockPos.betweenClosed(origin, origin.offset(size.getX() - 1, size.getY() - 1, size.getZ() - 1))) {
			if (!level.getBlockState(p).isAir()) {
				solid++;
			}
		}
		ClassroomRun.LOGGER.info("Classroom blocks: placeInWorld={}, {} non-air blocks in the volume", ok, solid);
		if (!ok || solid == 0) {
			ClassroomRun.LOGGER.error("Classroom placement produced no block (placeInWorld={})", ok);
		}

		Vec3 base = Vec3.atLowerCornerOf(origin);
		Vec3 spawn = base.add(loaded.spawn() != null ? loaded.spawn() : DEFAULT_SPAWN_OFFSET);
		if (loaded.spawn() == null) {
			ClassroomRun.LOGGER.warn("Classroom template has no entity tagged {}, using the default spawn", SPAWN_TAG);
		}
		Display.TextDisplay board = createBoard(level, loaded.boardEntry(), base);
		ClassroomRun.LOGGER.info("Classroom placed at {} from source: {} (spawn {}, board {})", origin, loaded.source(),
				spawn, board != null ? "yes" : "none");
		return new Placed(level, spawn, board);
	}

	/** Sets the board text (several lines, wrapped by the display's line_width). */
	public static void updateBoard(@Nullable Placed placed, String text) {
		if (placed == null || placed.board() == null || placed.board().isRemoved()) {
			return;
		}
		placed.board().setText(Component.literal(text));
		ClassroomRun.LOGGER.debug("Board updated: {}", text);
	}

	/** Creates the board ourselves: entities placed by the template are not reliably reachable right after placement. */
	private static Display.@Nullable TextDisplay createBoard(ServerLevel level, @Nullable CompoundTag entry, Vec3 base) {
		if (entry == null) {
			ClassroomRun.LOGGER.warn("Classroom template has no entity tagged {}", BOARD_TAG);
			return null;
		}
		try {
			CompoundTag nbt = entry.getCompoundOrEmpty("nbt").copy();
			Vec3 pos = base.add(readPos(entry));
			ListTag posTag = new ListTag();
			posTag.add(DoubleTag.valueOf(pos.x));
			posTag.add(DoubleTag.valueOf(pos.y));
			posTag.add(DoubleTag.valueOf(pos.z));
			nbt.put("Pos", posTag);
			nbt.remove("UUID");
			Optional<Entity> created = EntityType.create(TagValueInput.create(ProblemReporter.DISCARDING, level.registryAccess(), nbt),
					level, new EntitySpawnRequest(EntitySpawnReason.STRUCTURE, false));
			if (created.isPresent() && created.get() instanceof Display.TextDisplay board && level.addFreshEntity(board)) {
				return board;
			}
		} catch (RuntimeException e) {
			ClassroomRun.LOGGER.warn("Cannot create the classroom board", e);
		}
		return null;
	}

	private static Vec3 readPos(CompoundTag entry) {
		ListTag pos = entry.getListOrEmpty("pos");
		return new Vec3(pos.getDoubleOr(0, 0.0), pos.getDoubleOr(1, 0.0), pos.getDoubleOr(2, 0.0));
	}

	private static boolean hasTag(CompoundTag entry, String tag) {
		ListTag tags = entry.getCompoundOrEmpty("nbt").getListOrEmpty("Tags");
		for (int i = 0; i < tags.size(); i++) {
			if (tag.equals(tags.getStringOr(i, ""))) {
				return true;
			}
		}
		return false;
	}

	private static @Nullable Loaded loadTemplate(MinecraftServer server) {
		Path external = FabricLoader.getInstance().getConfigDir().resolve("classroomrun").resolve("classroom.nbt");
		try {
			CompoundTag tag;
			String source;
			if (Files.isRegularFile(external)) {
				source = "config " + external;
				try (InputStream in = Files.newInputStream(external)) {
					tag = NbtIo.readCompressed(in, NbtAccounter.unlimitedHeap());
				}
			} else {
				source = "jar resource";
				try (InputStream in = Classroom.class.getResourceAsStream(RESOURCE)) {
					if (in == null) {
						ClassroomRun.LOGGER.warn("No classroom template ({} or jar resource), using the fallback platform", external);
						return null;
					}
					tag = NbtIo.readCompressed(in, NbtAccounter.unlimitedHeap());
				}
			}
			// Spawn marker and board are taken out of the NBT: spawn is read directly, the board is created by us.
			Vec3 spawn = null;
			CompoundTag boardEntry = null;
			ListTag kept = new ListTag();
			ListTag entities = tag.getListOrEmpty("entities");
			for (int i = 0; i < entities.size(); i++) {
				CompoundTag entry = entities.getCompoundOrEmpty(i);
				if (spawn == null && hasTag(entry, SPAWN_TAG)) {
					spawn = readPos(entry);
				} else if (boardEntry == null && hasTag(entry, BOARD_TAG)) {
					boardEntry = entry.copy();
				} else {
					kept.add(entry);
				}
			}
			tag.put("entities", kept);
			upgradePalette(tag);
			StructureTemplate template = new StructureTemplate();
			template.load(server.registryAccess().lookupOrThrow(Registries.BLOCK), tag);
			return new Loaded(template, source, spawn, boardEntry);
		} catch (IOException | RuntimeException e) {
			ClassroomRun.LOGGER.warn("Cannot load the classroom template, using the fallback platform", e);
			return null;
		}
	}

	/**
	 * Since 26.x {@code NbtUtils.readBlockState} reads {@code id}/{@code properties}; the legacy structure format uses
	 * {@code Name}/{@code Properties} and would silently load every block as air.
	 */
	static void upgradePalette(CompoundTag tag) {
		ListTag palette = tag.getListOrEmpty("palette");
		for (int i = 0; i < palette.size(); i++) {
			CompoundTag entry = palette.getCompoundOrEmpty(i);
			if (!entry.contains("id") && entry.contains("Name")) {
				entry.put("id", entry.get("Name"));
				entry.remove("Name");
			}
			if (!entry.contains("properties") && entry.contains("Properties")) {
				entry.put("properties", entry.get("Properties"));
				entry.remove("Properties");
			}
		}
	}

	/** Median of the ground heights, minus one so the floor replaces the top block instead of resting on it. */
	static int medianGround(int[] heights) {
		int[] h = heights.clone();
		java.util.Arrays.sort(h);
		return h[h.length / 2] - 1;
	}

	/**
	 * Centre of a dry footprint near {@code start}: spirals outwards (16-block steps, up to {@link #DRY_SEARCH_RADIUS}),
	 * skips ocean and river biomes cheaply, then checks the generated surface for fluids. Falls back to {@code start}.
	 */
	private static BlockPos dryCenter(ServerLevel level, BlockPos start, Vec3i size) {
		int checks = 0;
		for (int ring = 0; ring * 16 <= DRY_SEARCH_RADIUS && checks < MAX_DRY_CHECKS; ring++) {
			for (int dx = -ring; dx <= ring; dx++) {
				for (int dz = -ring; dz <= ring; dz++) {
					if (Math.max(Math.abs(dx), Math.abs(dz)) != ring) {
						continue; // only the edge of the ring
					}
					BlockPos c = start.offset(dx * 16, 0, dz * 16);
					if (!landBiomes(level, c, size)) {
						continue;
					}
					if (checks++ >= MAX_DRY_CHECKS) {
						break;
					}
					if (dry(level, c, size)) {
						if (ring > 0) {
							ClassroomRun.LOGGER.info("Classroom moved {} blocks from the world spawn to stay out of water", ring * 16);
						}
						return c;
					}
				}
			}
		}
		ClassroomRun.LOGGER.warn("No dry ground found near {}, the classroom is placed there anyway", start);
		return start;
	}

	private static int[][] footprintSamples(BlockPos c, Vec3i size) {
		int hx = size.getX() / 2, hz = size.getZ() / 2;
		return new int[][] { { 0, 0 }, { -hx, -hz }, { hx, -hz }, { -hx, hz }, { hx, hz } };
	}

	private static boolean landBiomes(ServerLevel level, BlockPos c, Vec3i size) {
		int qy = level.getSeaLevel() >> 2;
		for (int[] o : footprintSamples(c, size)) {
			var biome = level.getUncachedNoiseBiome((c.getX() + o[0]) >> 2, qy, (c.getZ() + o[1]) >> 2);
			if (biome.is(BiomeTags.IS_OCEAN) || biome.is(BiomeTags.IS_DEEP_OCEAN) || biome.is(BiomeTags.IS_RIVER)) {
				return false;
			}
		}
		return true;
	}

	/** Generated surface without fluid (water, lava) at the centre and the corners of the footprint. */
	private static boolean dry(ServerLevel level, BlockPos c, Vec3i size) {
		for (int[] o : footprintSamples(c, size)) {
			int x = c.getX() + o[0], z = c.getZ() + o[1];
			level.getChunk(x >> 4, z >> 4); // generated before reading the heightmap
			int y = level.getHeight(Heightmap.Types.MOTION_BLOCKING_NO_LEAVES, x, z);
			if (!level.getFluidState(new BlockPos(x, y - 1, z)).isEmpty() || !level.getFluidState(new BlockPos(x, y, z)).isEmpty()) {
				return false;
			}
		}
		return true;
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
