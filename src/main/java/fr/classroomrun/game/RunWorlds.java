package fr.classroomrun.game;

import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.level.Level;
import org.jspecify.annotations.Nullable;
import xyz.nucleoid.fantasy.Fantasy;
import xyz.nucleoid.fantasy.RuntimeLevelConfig;
import xyz.nucleoid.fantasy.RuntimeLevelHandle;

/**
 * The Overworld + Nether + End trio of one run. Fantasy does not link dimensions, so the links are made by the
 * portal mixins through {@link #nether()} / {@link #end()} / {@link #overworld()}.
 * Dimension types and chunk generators are the ones of the vanilla levels, only the seed is new.
 */
public record RunWorlds(RuntimeLevelHandle overworldHandle, RuntimeLevelHandle netherHandle,
		RuntimeLevelHandle endHandle) {

	public static RunWorlds open(MinecraftServer server, long seed) {
		Fantasy fantasy = Fantasy.get(server);
		return new RunWorlds(
				open(server, fantasy, Level.OVERWORLD, seed),
				open(server, fantasy, Level.NETHER, seed),
				open(server, fantasy, Level.END, seed));
	}

	private static RuntimeLevelHandle open(MinecraftServer server, Fantasy fantasy, net.minecraft.resources.ResourceKey<Level> vanillaKey, long seed) {
		ServerLevel vanilla = server.getLevel(vanillaKey);
		if (vanilla == null) {
			throw new IllegalStateException("Vanilla level " + vanillaKey.identifier() + " is not loaded");
		}
		RuntimeLevelConfig config = new RuntimeLevelConfig()
				.setSeed(seed)
				.setDimensionType(vanilla.dimensionTypeRegistration())
				.setGenerator(vanilla.getChunkSource().getGenerator())
				.setShouldTickTime(true)
				.mirrorOverworldGameState();
		return fantasy.openTemporaryLevel(config);
	}

	public ServerLevel overworld() {
		return this.overworldHandle.asLevel();
	}

	public ServerLevel nether() {
		return this.netherHandle.asLevel();
	}

	public ServerLevel end() {
		return this.endHandle.asLevel();
	}

	public boolean contains(@Nullable Level level) {
		return level != null && (level == overworld() || level == nether() || level == end());
	}

	/** Deletes the three levels. Every player must have been moved out beforehand. */
	public void delete() {
		this.overworldHandle.delete();
		this.netherHandle.delete();
		this.endHandle.delete();
	}
}
