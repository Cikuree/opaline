package fr.classroomrun.net;

import io.netty.buffer.ByteBuf;
import java.util.UUID;
import net.minecraft.core.UUIDUtil;
import net.minecraft.network.codec.ByteBufCodecs;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.common.custom.CustomPacketPayload;

/**
 * A player was healed (potion, golden apple...). Natural regeneration is not reported.
 *
 * @param player UUID of the player whose heal triggered the shared heal
 * @param hearts health actually gained, in hearts (amount / 2), &gt; 0
 */
public record HealPayload(UUID player, float hearts) implements CustomPacketPayload {
	public static final Type<HealPayload> TYPE = new Type<>(ClassroomRunNetworking.id("heal"));
	public static final StreamCodec<ByteBuf, HealPayload> CODEC = StreamCodec.composite(
			UUIDUtil.STREAM_CODEC, HealPayload::player,
			ByteBufCodecs.FLOAT, HealPayload::hearts,
			HealPayload::new);

	@Override
	public Type<HealPayload> type() {
		return TYPE;
	}
}
