package fr.classroomrun.net;

import io.netty.buffer.ByteBuf;
import java.util.UUID;
import net.minecraft.core.UUIDUtil;
import net.minecraft.network.codec.ByteBufCodecs;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.common.custom.CustomPacketPayload;

/**
 * A player took damage. Sent to every player of the team, once per hit on the hit player
 * (not once per synced player).
 *
 * @param player UUID of the player who was hit
 * @param hearts damage actually taken, in hearts (damage / 2), &gt; 0
 */
public record DamagePayload(UUID player, float hearts) implements CustomPacketPayload {
	public static final Type<DamagePayload> TYPE = new Type<>(ClassroomRunNetworking.id("damage"));
	public static final StreamCodec<ByteBuf, DamagePayload> CODEC = StreamCodec.composite(
			UUIDUtil.STREAM_CODEC, DamagePayload::player,
			ByteBufCodecs.FLOAT, DamagePayload::hearts,
			DamagePayload::new);

	@Override
	public Type<DamagePayload> type() {
		return TYPE;
	}
}
