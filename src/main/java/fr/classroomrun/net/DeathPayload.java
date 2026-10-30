package fr.classroomrun.net;

import io.netty.buffer.ByteBuf;
import java.util.Optional;
import java.util.UUID;
import net.minecraft.core.BlockPos;
import net.minecraft.core.UUIDUtil;
import net.minecraft.core.registries.Registries;
import net.minecraft.network.codec.ByteBufCodecs;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.common.custom.CustomPacketPayload;
import net.minecraft.resources.ResourceKey;
import net.minecraft.world.level.Level;

/**
 * The team died: sent once, for the player whose death ended the run.
 *
 * @param player    UUID of the player who took the fatal hit
 * @param cause     damage type id, e.g. "minecraft:fall"
 * @param message   plain-text vanilla death message, e.g. "Alex fell from a high place"
 * @param attacker  name of the attacking entity (player name or entity type id), if any
 * @param pos       block position of the death
 * @param dimension dimension of the death (may be a Fantasy runtime dimension)
 */
public record DeathPayload(UUID player, String cause, String message, Optional<String> attacker, BlockPos pos,
		ResourceKey<Level> dimension) implements CustomPacketPayload {
	public static final Type<DeathPayload> TYPE = new Type<>(ClassroomRunNetworking.id("death"));
	public static final StreamCodec<ByteBuf, DeathPayload> CODEC = StreamCodec.composite(
			UUIDUtil.STREAM_CODEC, DeathPayload::player,
			ByteBufCodecs.STRING_UTF8, DeathPayload::cause,
			ByteBufCodecs.STRING_UTF8, DeathPayload::message,
			ByteBufCodecs.optional(ByteBufCodecs.STRING_UTF8), DeathPayload::attacker,
			BlockPos.STREAM_CODEC, DeathPayload::pos,
			ResourceKey.streamCodec(Registries.DIMENSION), DeathPayload::dimension,
			DeathPayload::new);

	@Override
	public Type<DeathPayload> type() {
		return TYPE;
	}
}
