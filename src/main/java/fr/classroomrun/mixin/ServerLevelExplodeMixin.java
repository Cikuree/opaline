package fr.classroomrun.mixin;

import fr.classroomrun.game.RunManager;
import net.minecraft.core.Holder;
import net.minecraft.core.particles.ExplosionParticleInfo;
import net.minecraft.core.particles.ParticleOptions;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.sounds.SoundEvent;
import net.minecraft.util.random.WeightedList;
import net.minecraft.world.damagesource.DamageSource;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.level.ExplosionDamageCalculator;
import net.minecraft.world.level.Level;
import org.jspecify.annotations.Nullable;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Why a mixin: the death clip replays explosions (particles and sound), and Fabric API has no explosion event.
 * {@code ServerLevel.explode} is where creepers, TNT, beds, crystals and wither skulls all end up ({@code Level.explode}
 * overloads delegate to it); only the centre and power are read, before the explosion changes anything.
 */
@Mixin(ServerLevel.class)
public abstract class ServerLevelExplodeMixin {
	@Inject(method = "explode", at = @At("HEAD"))
	private void classroomrun$recordExplosion(@Nullable Entity source, @Nullable DamageSource damageSource,
			@Nullable ExplosionDamageCalculator calculator, double x, double y, double z, float radius, boolean fire,
			Level.ExplosionInteraction interaction, ParticleOptions smallParticles, ParticleOptions largeParticles,
			WeightedList<ExplosionParticleInfo> blockParticles, Holder<SoundEvent> sound, CallbackInfo ci) {
		RunManager run = RunManager.get();
		if (run != null) {
			run.clips().onExplosion((ServerLevel) (Object) this, x, y, z, radius);
		}
	}
}
