package fr.classroomrun.clip;

import fr.classroomrun.clip.ClipBuffer.Entry;
import fr.classroomrun.clip.ClipBuffer.Info;
import fr.classroomrun.net.DeathClipPayload;
import fr.classroomrun.sync.TeamSync;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.Mob;
import net.minecraft.world.entity.item.PrimedTnt;
import net.minecraft.world.entity.monster.Creeper;
import net.minecraft.world.entity.projectile.ThrowableProjectile;
import net.minecraft.world.entity.projectile.arrow.AbstractArrow;
import net.minecraft.world.entity.projectile.hurtingprojectile.AbstractHurtingProjectile;
import net.minecraft.world.entity.player.Player;
import net.minecraft.util.Mth;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.Vec3;
import org.jspecify.annotations.Nullable;

import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import java.util.UUID;

/**
 * Records the last minute around the team (one sample every {@link #SAMPLE_TICKS} ticks) and builds the death clip.
 * Server thread only. The buffer logic lives in {@link ClipBuffer}.
 */
public final class ClipRecorder {
	public static final int SAMPLE_TICKS = 2;
	private static final int WINDOW_TICKS = 60 * 20;
	private static final double RECORD_RADIUS = 32.0;
	private static final double CLIP_RADIUS = 48.0;

	private static final int MAX_LOGGED_BLOCKS = 8192;
	private static final int MAX_LOGGED_EXPLOSIONS = 256;

	private final BlockLog blocks = new BlockLog(MAX_LOGGED_BLOCKS);
	private final ExplosionLog explosions = new ExplosionLog(MAX_LOGGED_EXPLOSIONS);
	/** True from the first sample of a contiguous recording until {@link #clear}; gates {@link #onBlockChange}. */
	private boolean recording;
	private List<ServerPlayer> members = List.of();
	private final ClipBuffer buffer = new ClipBuffer(Math.min(DeathClipPayload.MAX_FRAMES, WINDOW_TICKS / SAMPLE_TICKS));
	/** Entities swinging at the previous sample: a swing is "started" only on the transition. */
	private Set<UUID> swinging = new HashSet<>();
	private int ticks;

	/** Called every server tick while the run is active; samples every {@link #SAMPLE_TICKS} ticks. */
	public void tick(List<ServerPlayer> players) {
		this.recording = true;
		this.members = players;
		if (++this.ticks % SAMPLE_TICKS == 0) {
			sample(players);
		}
	}

	public void clear() {
		this.buffer.clear();
		this.blocks.clear();
		this.explosions.clear();
		this.recording = false;
		this.members = List.of();
		this.swinging = new HashSet<>();
	}

	/**
	 * Called by the block-set mixin for every successful server-side block change. Cheap when not recording; otherwise
	 * keeps it only within 32 blocks (horizontally) of a team member in that level.
	 */
	public void onBlockChange(ServerLevel level, BlockPos pos, BlockState oldState, BlockState newState) {
		if (!this.recording || oldState == newState) {
			return;
		}
		for (ServerPlayer m : this.members) {
			if (m.level() == level && TeamSync.isMember(m) && Math.abs(m.getX() - pos.getX()) <= RECORD_RADIUS
					&& Math.abs(m.getZ() - pos.getZ()) <= RECORD_RADIUS) {
				this.blocks.add(level.dimension().identifier().toString(), pos.getX(), pos.getY(), pos.getZ(),
						Block.getId(oldState), Block.getId(newState));
				return;
			}
		}
	}

	/** Called by the explosion mixin: kept when a team member of that level is within {@link DeathClipPayload#EXPLOSION_RADIUS}. */
	public void onExplosion(ServerLevel level, double x, double y, double z, float radius) {
		if (!this.recording || radius <= 0) {
			return;
		}
		double max = DeathClipPayload.EXPLOSION_RADIUS;
		for (ServerPlayer m : this.members) {
			if (m.level() == level && TeamSync.isMember(m) && m.distanceToSqr(x, y, z) <= max * max) {
				this.explosions.add(level.dimension().identifier().toString(), x, y, z, radius);
				return;
			}
		}
	}

	/** Arrows in flight, fireballs, wither skulls, thrown items and primed TNT: what the clip shows besides living entities. */
	private static boolean isHazard(Entity e) {
		if (!e.isAlive()) {
			return false;
		}
		if (e instanceof AbstractArrow) {
			return e.getDeltaMovement().lengthSqr() > 1.0E-7; // stuck in a block: not worth an actor for the whole minute
		}
		return e instanceof AbstractHurtingProjectile || e instanceof ThrowableProjectile || e instanceof PrimedTnt;
	}

	/** Records every living entity and hazard within 32 blocks of a team member (spectators excluded). */
	private void sample(List<ServerPlayer> players) {
		Map<UUID, Entry> frame = new HashMap<>();
		Set<UUID> nowSwinging = new HashSet<>();
		for (ServerPlayer member : players) {
			if (!TeamSync.isMember(member)) {
				continue;
			}
			ServerLevel level = member.level();
			String dimension = level.dimension().identifier().toString();
			var box = member.getBoundingBox().inflate(RECORD_RADIUS);
			for (LivingEntity e : level.getEntitiesOfClass(LivingEntity.class, box, x -> !x.isSpectator() && x.isAlive())) {
				if (frame.containsKey(e.getUUID())) {
					continue;
				}
				boolean swinging = e.isSwinging();
				if (swinging) {
					nowSwinging.add(e.getUUID());
				}
				// hurtTime counts down from hurtDuration each tick: the first samples after a hit see it near the top
				int flags = (e.onGround() ? DeathClipPayload.ON_GROUND : 0)
						| (e.isCrouching() ? DeathClipPayload.CROUCHING : 0)
						| (e.isSprinting() ? DeathClipPayload.SPRINTING : 0)
						| (swinging && !this.swinging.contains(e.getUUID()) ? DeathClipPayload.SWING : 0)
						| (e.hurtDuration > 0 && e.hurtTime >= e.hurtDuration - SAMPLE_TICKS + 1 ? DeathClipPayload.HURT : 0)
						| (e.isFallFlying() ? DeathClipPayload.FALL_FLYING : 0)
						| (e.isUsingItem() ? DeathClipPayload.USING_ITEM : 0)
						| (e instanceof Mob mob && mob.isAggressive() ? DeathClipPayload.AGGRESSIVE : 0);
				int swell = e instanceof Creeper creeper ? ClipExtra.swellLevel(creeper.getSwelling(1.0F)) : 0;
				int extra = ClipExtra.pack(swell, e.isUsingItem() && e.getUsedItemHand() == InteractionHand.OFF_HAND);
				frame.put(e.getUUID(), new Entry(dimension, info(e), e.getX(), e.getY(), e.getZ(), e.getYRot(),
						e.getXRot(), e.getYHeadRot(), flags, extra));
			}
			for (Entity e : level.getEntitiesOfClass(Entity.class, box, ClipRecorder::isHazard)) {
				if (!frame.containsKey(e.getUUID())) {
					int extra = e instanceof PrimedTnt tnt ? ClipExtra.pack(ClipExtra.fuseLevel(tnt.getFuse()), false) : 0;
					frame.put(e.getUUID(), new Entry(dimension, new Info(typeId(e), Optional.empty(), "", "", ""), e.getX(),
							e.getY(), e.getZ(), e.getYRot(), e.getXRot(), e.getYRot(), 0, extra));
				}
			}
		}
		this.swinging = nowSwinging;
		this.buffer.add(frame);
		this.blocks.nextSample();
		this.explosions.nextSample();
	}

	private static Info info(LivingEntity e) {
		Optional<UUID> profile = e instanceof Player p ? Optional.of(p.getUUID()) : Optional.empty();
		String name = e instanceof Player p ? p.getGameProfile().name()
				: e.hasCustomName() ? e.getCustomName().getString() : "";
		if (name.length() > 64) {
			name = name.substring(0, 64); // a huge custom name must not blow the payload limit
		}
		return new Info(typeId(e), profile, name, itemId(e.getMainHandItem()), itemId(e.getOffhandItem()));
	}

	private static String typeId(Entity e) {
		return BuiltInRegistries.ENTITY_TYPE.getKey(e.getType()).toString();
	}

	private static String itemId(ItemStack stack) {
		return stack.isEmpty() ? "" : BuiltInRegistries.ITEM.getKey(stack.getItem()).toString();
	}

	/**
	 * Records a final sample (the death), builds the clip and clears the buffer.
	 *
	 * @return null when there is nothing to play
	 */
	public @Nullable DeathClipPayload build(ServerPlayer victim, Vec3 deathPos, ResourceKey<Level> dimension,
			List<ServerPlayer> players) {
		sample(players);
		ClipBuffer.Clip clip = this.buffer.build(victim.getUUID(), dimension.identifier().toString(), deathPos.x,
				deathPos.y, deathPos.z, CLIP_RADIUS);
		List<DeathClipPayload.BlockChange> changes = clip == null ? List.of()
				: this.blocks.build(dimension.identifier().toString(), Mth.floor(deathPos.x), Mth.floor(deathPos.y),
						Mth.floor(deathPos.z), DeathClipPayload.BLOCK_RADIUS, clip.frameCount(),
						DeathClipPayload.MAX_BLOCK_CHANGES);
		List<DeathClipPayload.Explosion> booms = clip == null ? List.of()
				: this.explosions.build(dimension.identifier().toString(), deathPos.x, deathPos.y, deathPos.z,
						DeathClipPayload.EXPLOSION_RADIUS, clip.frameCount(), DeathClipPayload.MAX_EXPLOSIONS);
		clear();
		return clip == null ? null : new DeathClipPayload(victim.getUUID(), dimension, deathPos.x, deathPos.y, deathPos.z,
				SAMPLE_TICKS, clip.frameCount(), clip.actors(), changes, booms);
	}
}
