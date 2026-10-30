package fr.classroomrun.game;

import fr.classroomrun.net.Split;
import net.minecraft.advancements.AdvancementHolder;
import net.minecraft.resources.Identifier;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.Items;

import java.util.EnumMap;
import java.util.List;
import java.util.Map;

/**
 * Detects the splits by polling the players (no mixin). A split is shared by the whole team: it is reached as soon as
 * one player satisfies it. Choice per split:
 * <ul>
 *   <li>PICKAXE: advancement {@code story/mine_stone} (get a stone-tool material, requires the wooden pickaxe);</li>
 *   <li>IRON_INGOT: advancement {@code story/smelt_iron} (iron ingot in inventory);</li>
 *   <li>DIAMOND: advancement {@code story/mine_diamond};</li>
 *   <li>NETHER_PORTAL: the player is in the run Nether. The vanilla advancement {@code story/enter_the_nether} cannot
 *       be used: its {@code changed_dimension} criterion compares with the vanilla dimension key, never a Fantasy one;</li>
 *   <li>NETHER_WART: advancement {@code nether/find_fortress} (location criterion on the structure, independent of the
 *       dimension key) or a nether wart in the inventory;</li>
 *   <li>BLAZE_ROD: advancement {@code nether/obtain_blaze_rod} or a blaze rod in the inventory;</li>
 *   <li>ENDER_EYE: advancement {@code story/follow_ender_eye} (player inside a stronghold);</li>
 *   <li>ENTER_END: the player is in the run End (same reason as for the Nether);</li>
 *   <li>KILL_DRAGON: advancement {@code end/kill_dragon} (player_killed_entity, independent of the dimension).</li>
 * </ul>
 * Advancements are revoked from every player on each reset, so they only reflect the current run.
 */
final class SplitProbe {
	private static final Map<Split, String> ADVANCEMENTS = new EnumMap<>(Split.class);
	private static final Map<Split, Item> ITEMS = new EnumMap<>(Split.class);

	static {
		ADVANCEMENTS.put(Split.PICKAXE, "story/mine_stone");
		ADVANCEMENTS.put(Split.IRON_INGOT, "story/smelt_iron");
		ADVANCEMENTS.put(Split.DIAMOND, "story/mine_diamond");
		ADVANCEMENTS.put(Split.NETHER_WART, "nether/find_fortress");
		ADVANCEMENTS.put(Split.BLAZE_ROD, "nether/obtain_blaze_rod");
		ADVANCEMENTS.put(Split.ENDER_EYE, "story/follow_ender_eye");
		ADVANCEMENTS.put(Split.KILL_DRAGON, "end/kill_dragon");
		ITEMS.put(Split.NETHER_WART, Items.NETHER_WART);
		ITEMS.put(Split.BLAZE_ROD, Items.BLAZE_ROD);
	}

	private SplitProbe() {
	}

	/** @return the splits satisfied by at least one of the players among those not yet reached (bit mask) */
	static int probe(MinecraftServer server, List<ServerPlayer> players, RunWorlds worlds, int alreadyReached) {
		int found = 0;
		for (Split split : Split.values()) {
			if ((alreadyReached & split.bit()) != 0) {
				continue;
			}
			for (ServerPlayer player : players) {
				if (satisfied(server, player, worlds, split)) {
					found |= split.bit();
					break;
				}
			}
		}
		return found;
	}

	private static boolean satisfied(MinecraftServer server, ServerPlayer player, RunWorlds worlds, Split split) {
		if (split == Split.NETHER_PORTAL) {
			return player.level() == worlds.nether();
		}
		if (split == Split.ENTER_END) {
			return player.level() == worlds.end();
		}
		String advancement = ADVANCEMENTS.get(split);
		if (advancement != null) {
			AdvancementHolder holder = server.getAdvancements().get(Identifier.fromNamespaceAndPath("minecraft", advancement));
			if (holder != null && player.getAdvancements().getOrStartProgress(holder).isDone()) {
				return true;
			}
		}
		Item item = ITEMS.get(split);
		return item != null && player.getInventory().contains(stack -> stack.is(item));
	}
}
