package fr.classroomrun.client.clip;

import com.mojang.authlib.GameProfile;
import fr.classroomrun.net.DeathClipPayload;
import fr.classroomrun.net.DeathClipPayload.Actor;
import java.nio.charset.StandardCharsets;
import java.util.UUID;
import net.minecraft.client.CameraType;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.Identifier;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntitySpawnReason;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.EntityTypes;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.Pose;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.component.SwingAnimation;

/**
 * The fake entities of a playing clip plus the orbit camera. Fakes live in the {@link ClientLevel} with negative ids, and exist
 * only while their actor is present. Main thread only. {@link #update} allocates nothing while actors stay present.
 */
final class ClipStage {
	private static final int ID_BASE = -1_000_000;
	private static final int ID_RANGE = 10_000;
	/** Distance (frames) under which a flag edge is treated as normal forward playback, not a seek. */
	private static final int MAX_STEP = 4;

	/** The stage being shown, read by the mixins that hide the real entities. */
	static ClipStage active;

	private static int nextId = ID_BASE;

	private final ClientLevel level;
	private final DeathClipPayload clip;
	private final Fake[] fakes;
	private final Fake victim;
	private final OrbitCamera orbit;
	private final ClipTerrain terrain;
	private final Entity camera;
	private final float[] tmp = new float[ClipMath.OUT];
	private final CameraType savedCameraType;
	private final Entity savedCamera;
	private double targetX, targetY, targetZ;

	private static final class Fake {
		final Actor actor;
		Entity entity;
		int lastFrame = -1;
		double tickX = Double.NaN, tickZ;

		Fake(Actor actor) {
			this.actor = actor;
		}
	}

	/** @return null when the camera entity cannot be created (the screen then shows the UI only) */
	static ClipStage open(Minecraft mc, ClientLevel level, DeathClipPayload clip) {
		Entity camera = EntityTypes.MARKER.create(level, EntitySpawnReason.LOAD);
		if (camera == null) {
			return null;
		}
		return new ClipStage(mc, level, clip, camera);
	}

	private ClipStage(Minecraft mc, ClientLevel level, DeathClipPayload clip, Entity camera) {
		this.level = level;
		this.clip = clip;
		this.camera = camera;
		this.fakes = new Fake[clip.actors().size()];
		Fake v = null;
		for (int i = 0; i < this.fakes.length; i++) {
			this.fakes[i] = new Fake(clip.actors().get(i));
			if (v == null || this.fakes[i].actor.id() == 0) {
				v = this.fakes[i];
			}
		}
		this.victim = v;
		float startYaw = 0f;
		if (v != null && v.actor.frames() > 0) {
			startYaw = ClipMath.unpackAngle(v.actor.rot()[3 * (v.actor.frames() - 1)]);
		}
		this.orbit = new OrbitCamera(startYaw);
		this.terrain = new ClipTerrain(level, clip);
		this.targetX = clip.anchorX();
		this.targetY = clip.anchorY();
		this.targetZ = clip.anchorZ();
		this.savedCameraType = mc.options.getCameraType();
		this.savedCamera = mc.getCameraEntity();
		mc.options.setCameraType(CameraType.FIRST_PERSON);
		mc.setCameraEntity(camera);
		active = this;
	}

	OrbitCamera orbit() {
		return this.orbit;
	}

	ClientLevel level() {
		return this.level;
	}

	static boolean isFake(Entity entity) {
		int id = entity.getId();
		return id <= ID_BASE && id > ID_BASE - ID_RANGE;
	}

	/** Applies the state at the timeline position to every fake and to the camera. */
	void update(ClipTimeline timeline) {
		double framePos = timeline.framePos();
		int frame = (int) Math.floor(framePos);
		for (Fake f : this.fakes) {
			Actor a = f.actor;
			if (!ClipMath.sample(a.pos(), a.rot(), a.firstFrame(), a.frames(), framePos, this.tmp)) {
				despawn(f);
				continue;
			}
			if (f.entity == null && !spawn(f)) {
				continue;
			}
			apply(f, frame);
			if (f == this.victim) {
				this.targetX = this.clip.anchorX() + this.tmp[0];
				this.targetY = this.clip.anchorY() + this.tmp[1] + 1.0;
				this.targetZ = this.clip.anchorZ() + this.tmp[2];
			}
		}
		this.terrain.update(frame);
		this.orbit.update(this.targetX, this.targetY, this.targetZ);
		this.camera.snapTo(this.orbit.x, this.orbit.y, this.orbit.z, this.orbit.yRot, this.orbit.xRot);
		// a ClientboundSetCameraPacket may have replaced the camera
		Minecraft mc = Minecraft.getInstance();
		if (mc.getCameraEntity() != this.camera) {
			mc.setCameraEntity(this.camera);
		}
	}

	private void apply(Fake f, int frame) {
		Entity e = f.entity;
		Actor a = f.actor;
		e.snapTo(this.clip.anchorX() + this.tmp[0], this.clip.anchorY() + this.tmp[1], this.clip.anchorZ() + this.tmp[2],
				this.tmp[3], this.tmp[4]);
		int flags = a.flags()[frame - a.firstFrame()];
		boolean crouching = (flags & DeathClipPayload.CROUCHING) != 0;
		e.setOnGround((flags & DeathClipPayload.ON_GROUND) != 0);
		if (e instanceof LivingEntity living) {
			living.yBodyRot = this.tmp[3];
			living.yBodyRotO = this.tmp[3];
			living.yHeadRot = this.tmp[5];
			living.yHeadRotO = this.tmp[5];
			living.setSprinting((flags & DeathClipPayload.SPRINTING) != 0);
			living.setShiftKeyDown(crouching);
			if (living instanceof FakePlayer player) {
				boolean flying = (flags & DeathClipPayload.FALL_FLYING) != 0;
				player.setClipFallFlying(flying);
				player.setPose(flying ? Pose.FALL_FLYING : crouching ? Pose.CROUCHING : Pose.STANDING);
			}
			int step = frame - f.lastFrame;
			if (f.lastFrame >= 0 && step > 0 && step <= MAX_STEP) {
				if (ClipMath.anyFlag(a.flags(), a.firstFrame(), a.frames(), f.lastFrame, frame, DeathClipPayload.SWING)) {
					living.swing(InteractionHand.MAIN_HAND, SwingAnimation.DEFAULT, false);
				}
				if (ClipMath.anyFlag(a.flags(), a.firstFrame(), a.frames(), f.lastFrame, frame, DeathClipPayload.HURT)) {
					living.animateHurt(0f);
				}
			}
		}
		f.lastFrame = frame;
	}

	private boolean spawn(Fake f) {
		Actor a = f.actor;
		Entity e;
		if ("minecraft:player".equals(a.typeId())) {
			UUID skin = a.profile().orElseGet(() -> UUID.nameUUIDFromBytes(a.name().getBytes(StandardCharsets.UTF_8)));
			e = new FakePlayer(this.level, profile(a), skin);
		} else {
			Identifier id = Identifier.tryParse(a.typeId());
			EntityType<?> type = id == null ? null : BuiltInRegistries.ENTITY_TYPE.getOptional(id).orElse(null);
			e = type == null ? null : type.create(this.level, EntitySpawnReason.LOAD);
			if (e != null && !a.name().isEmpty()) {
				e.setCustomName(Component.literal(a.name()));
			}
		}
		if (e == null) {
			return false;
		}
		e.setId(nextId);
		nextId -= 16; // an ender_dragon assigns its parts to id+1..id+n
		if (nextId <= ID_BASE - ID_RANGE) {
			nextId = ID_BASE;
		}
		e.noPhysics = true;
		e.setNoGravity(true);
		e.setSilent(true);
		if (e instanceof LivingEntity living && !a.mainHand().isEmpty()) {
			Identifier item = Identifier.tryParse(a.mainHand());
			Item held = item == null ? null : BuiltInRegistries.ITEM.getOptional(item).orElse(null);
			if (held != null) {
				living.setItemSlot(EquipmentSlot.MAINHAND, new ItemStack(held));
			}
		}
		f.entity = e;
		f.lastFrame = -1;
		f.tickX = Double.NaN;
		// placed before it is added so the first extraction never sees it at the origin
		e.snapTo(this.clip.anchorX() + this.tmp[0], this.clip.anchorY() + this.tmp[1], this.clip.anchorZ() + this.tmp[2]);
		this.level.addEntity(e);
		return true;
	}

	private static GameProfile profile(Actor a) {
		String name = a.name().isEmpty() ? "Joueur" : a.name();
		if (name.length() > 16) {
			name = name.substring(0, 16);
		}
		try {
			return new GameProfile(UUID.randomUUID(), name);
		} catch (IllegalArgumentException e) {
			return new GameProfile(UUID.randomUUID(), "Joueur");
		}
	}

	private void despawn(Fake f) {
		if (f.entity != null) {
			this.level.removeEntity(f.entity.getId(), Entity.RemovalReason.DISCARDED);
			f.entity = null;
		}
	}

	/**
	 * Walk animation of a fake, from the distance it moved since the previous tick. Vanilla derives it from {@code xo}, which
	 * is always equal to the position here because the clip moves the entities between ticks.
	 *
	 * @return true when {@code living} is a fake (the vanilla computation is then skipped)
	 */
	static boolean animateWalk(LivingEntity living) {
		ClipStage stage = active;
		if (stage == null || !isFake(living)) {
			return false;
		}
		for (Fake f : stage.fakes) {
			if (f.entity == living) {
				boolean first = Double.isNaN(f.tickX);
				double dx = first ? 0 : living.getX() - f.tickX;
				double dz = first ? 0 : living.getZ() - f.tickZ;
				f.tickX = living.getX();
				f.tickZ = living.getZ();
				float target = Math.min((float) Math.sqrt(dx * dx + dz * dz) * 4f, 1f);
				living.walkAnimation.update(target, 0.4f, living.isBaby() ? 3f : 1f);
				return true;
			}
		}
		return false;
	}

	/** Removes the fakes and gives the camera back to the player. */
	void close(Minecraft mc) {
		this.terrain.restore();
		for (Fake f : this.fakes) {
			despawn(f);
		}
		if (active == this) {
			active = null;
		}
		mc.options.setCameraType(this.savedCameraType);
		Entity back = this.savedCamera;
		mc.setCameraEntity(back != null && !back.isRemoved() && back.level() == mc.level ? back : mc.player);
	}
}
