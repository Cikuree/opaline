package fr.classroomrun.sync;

import fr.classroomrun.net.DamagePayload;
import fr.classroomrun.net.HealPayload;
import fr.classroomrun.game.RunManager;
import fr.classroomrun.sync.SharedVitals.Vitals;
import net.fabricmc.fabric.api.entity.event.v1.ServerLivingEntityEvents;
import net.fabricmc.fabric.api.networking.v1.ServerPlayNetworking;
import net.minecraft.core.component.DataComponents;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.tags.DamageTypeTags;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.damagesource.DamageSource;
import net.minecraft.world.entity.LivingEntity;

import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import org.jspecify.annotations.Nullable;
import java.util.UUID;

/**
 * Minecraft glue of the shared life. The team is every online survival/adventure player. Rules (see also
 * {@link SharedVitals}):
 * <ul>
 *   <li><b>Propagation</b>: once per server tick, at the end of the tick, the shared value of health, absorption, food
 *       and saturation is computed with {@link SharedVitals#resolve} and written to the players with silent setters.
 *       Nothing is hooked on the propagation, so it can never re-trigger itself, and two players hit in the same tick
 *       give a shared loss of the biggest one only (shared life = min of the lives after the tick).</li>
 *   <li><b>Damage payload</b>: {@code ALLOW_DAMAGE} records health + absorption of the player about to be hit,
 *       {@code AFTER_DAMAGE} sends a {@link DamagePayload} with the real loss (armour, absorption, enchantments
 *       included) to everyone, once per hit, for the hit player only.</li>
 *   <li><b>Totem</b>: Fabric's {@code ALLOW_DEATH} is called inside {@code LivingEntity.hurtServer} <i>before</i>
 *       {@code checkTotemDeathProtection} (verified in the Fabric mixin: it replaces the second
 *       {@code isDeadOrDying()} call, so returning false skips both the totem and {@code die}). When the victim holds
 *       a death-protection item (and the damage does not bypass invulnerability) we let vanilla run: the totem is
 *       consumed, the victim is set to 1 HP and the tick propagation brings the whole team to 1 HP. Only the victim's
 *       own totem counts; a teammate's totem is not used.</li>
 *   <li><b>Heal payload</b>: no Fabric heal event, so a mixin on {@code LivingEntity.heal} calls {@link #onHeal};
 *       natural regeneration is excluded with the {@link #naturalRegeneration} flag raised by two small mixins.</li>
 * </ul>
 */
public final class TeamSync {
	/** True while vanilla natural regeneration is running (set by mixins, server thread only). */
	public static boolean naturalRegeneration;

	private static final float EPSILON = 1.0E-3F;
	private static final Vitals FULL = new Vitals(20.0F, 0.0F, 20.0F, 5.0F);

	private final SharedVitals vitals = new SharedVitals();
	private final Map<UUID, Float> beforeHit = new HashMap<>();
	/** Synced members of the previous tick: anybody else is a newcomer and does not take part in the merge. */
	private Set<UUID> previousTeam = Set.of();
	private @Nullable UUID carrier;
	private @Nullable ServerPlayer carrierPlayer;

	/** Registers the Fabric events; everything is routed to the current {@link RunManager}. */
	public static void registerEvents() {
		ServerLivingEntityEvents.ALLOW_DAMAGE.register((entity, source, amount) -> {
			RunManager run = RunManager.get();
			return run == null || !(entity instanceof ServerPlayer player) || run.team().allowDamage(run, player);
		});
		ServerLivingEntityEvents.AFTER_DAMAGE.register((entity, source, base, taken, blocked) -> {
			RunManager run = RunManager.get();
			if (run != null && entity instanceof ServerPlayer player) {
				run.team().afterDamage(run, player);
			}
		});
		ServerLivingEntityEvents.ALLOW_DEATH.register((entity, source, amount) -> {
			RunManager run = RunManager.get();
			return run == null || !(entity instanceof ServerPlayer player) || run.team().allowDeath(run, player, source);
		});
	}

	/** Team member: any player that can really take damage (not spectator, not creative). */
	public static boolean isMember(ServerPlayer player) {
		return !player.isSpectator() && !player.isCreative();
	}

	public static boolean isSynced(ServerPlayer player) {
		return player.isAlive() && isMember(player);
	}

	public List<ServerPlayer> team(MinecraftServer server) {
		return server.getPlayerList().getPlayers().stream().filter(TeamSync::isSynced).toList();
	}

	public void resetVitals() {
		this.vitals.reset(FULL);
		this.beforeHit.clear();
	}

	/** A player joins: it takes the common value, or defines it when the team was empty. */
	public void onJoin(MinecraftServer server, ServerPlayer player) {
		if (!isSynced(player)) {
			return;
		}
		boolean alone = team(server).stream().noneMatch(p -> p != player);
		if (alone || this.vitals.baseline() == null) {
			this.vitals.reset(read(player));
		} else {
			apply(player, this.vitals.baseline());
		}
		java.util.HashSet<UUID> known = new java.util.HashSet<>(this.previousTeam);
		known.add(player.getUUID());
		this.previousTeam = known;
	}

	/** End of server tick: propagates the shared values. Skipped while someone is dying. */
	public void tick(MinecraftServer server) {
		this.beforeHit.clear();
		List<ServerPlayer> team = team(server);
		if (team.isEmpty() || team.stream().anyMatch(LivingEntity::isDeadOrDying)) {
			return;
		}
		Set<UUID> ids = team.stream().map(ServerPlayer::getUUID).collect(java.util.stream.Collectors.toSet());
		List<ServerPlayer> established = team.stream().filter(p -> this.previousTeam.contains(p.getUUID())).toList();
		if (established.isEmpty()) {
			// nobody was synced before (first tick, or everybody was creative): the first player defines the value
			established = team.subList(0, 1);
			this.vitals.reset(read(established.getFirst()));
		}
		this.carrier = NaturalCarrier.select(established.stream().map(ServerPlayer::getUUID).toList(), this.carrier);
		this.carrierPlayer = established.stream().filter(p -> p.getUUID().equals(this.carrier)).findFirst().orElse(null);
		// newcomers (creative/spectator back to survival) are left out of the merge and receive the common value
		Vitals shared = this.vitals.resolve(established.stream().map(TeamSync::read).toList());
		for (ServerPlayer player : team) {
			apply(player, shared);
		}
		this.previousTeam = ids;
	}

	/** False for non-carrier members: their vanilla hunger and natural regeneration are skipped (see NaturalCarrier). */
	public static boolean mayRunNaturalLogic(ServerPlayer player) {
		return exhaustionTarget(player) == null;
	}

	/** The carrier to charge with this player's exhaustion, or null when the player is the carrier or not synced. */
	public static @Nullable ServerPlayer exhaustionTarget(ServerPlayer player) {
		RunManager run = RunManager.get();
		if (run == null || !isMember(player)) {
			return null;
		}
		TeamSync sync = run.team();
		ServerPlayer carrier = sync.carrierPlayer;
		if (carrier == null || carrier == player || carrier.hasDisconnected() || !isSynced(carrier)
				|| !sync.previousTeam.contains(player.getUUID())) {
			return null;
		}
		return carrier;
	}

	private boolean allowDamage(RunManager run, ServerPlayer player) {
		if (run.isEnding() || !run.isStarted()) {
			return false;
		}
		if (isMember(player)) {
			this.beforeHit.put(player.getUUID(), player.getHealth() + player.getAbsorptionAmount());
		}
		return true;
	}

	private void afterDamage(RunManager run, ServerPlayer player) {
		Float before = this.beforeHit.remove(player.getUUID());
		if (before == null) {
			return;
		}
		float taken = before - (player.getHealth() + player.getAbsorptionAmount());
		if (taken > EPSILON) {
			broadcast(run.server(), new DamagePayload(player.getUUID(), taken / 2.0F));
		}
	}

	private boolean allowDeath(RunManager run, ServerPlayer player, DamageSource source) {
		if (!isMember(player)) {
			return true;
		}
		if (run.isEnding()) {
			player.setHealth(player.getMaxHealth());
			return false;
		}
		if (hasDeathProtection(player, source)) {
			return true;
		}
		run.onCollectiveDeath(player, source);
		return false;
	}

	private static boolean hasDeathProtection(ServerPlayer player, DamageSource source) {
		if (source.is(DamageTypeTags.BYPASSES_INVULNERABILITY)) {
			return false;
		}
		for (InteractionHand hand : InteractionHand.values()) {
			if (player.getItemInHand(hand).get(DataComponents.DEATH_PROTECTION) != null) {
				return true;
			}
		}
		return false;
	}

	/** Called by the {@code LivingEntity.heal} mixin with the health really gained. */
	public static void onHeal(ServerPlayer player, float gained) {
		RunManager run = RunManager.get();
		if (run != null && !naturalRegeneration && !run.isEnding() && isMember(player) && gained > EPSILON) {
			broadcast(run.server(), new HealPayload(player.getUUID(), gained / 2.0F));
		}
	}

	private static void broadcast(MinecraftServer server, net.minecraft.network.protocol.common.custom.CustomPacketPayload payload) {
		for (ServerPlayer p : server.getPlayerList().getPlayers()) {
			ServerPlayNetworking.send(p, payload);
		}
	}

	private static Vitals read(ServerPlayer p) {
		return new Vitals(p.getHealth(), p.getAbsorptionAmount(), p.getFoodData().getFoodLevel(), p.getFoodData().getSaturationLevel());
	}

	private static void apply(ServerPlayer p, Vitals v) {
		if (Math.abs(p.getHealth() - v.health()) > EPSILON && v.health() > 0.0F) {
			p.setHealth(v.health());
		}
		if (Math.abs(p.getAbsorptionAmount() - v.absorption()) > EPSILON) {
			p.setAbsorptionAmount(v.absorption());
		}
		int food = Math.round(v.food());
		if (p.getFoodData().getFoodLevel() != food) {
			p.getFoodData().setFoodLevel(food);
		}
		float saturation = Math.min(v.saturation(), food);
		if (Math.abs(p.getFoodData().getSaturationLevel() - saturation) > EPSILON) {
			p.getFoodData().setSaturation(saturation);
		}
	}
}
