package fr.classroomrun.net;

import io.netty.buffer.ByteBuf;
import net.minecraft.network.codec.ByteBufCodecs;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.common.custom.CustomPacketPayload;

/**
 * Server → client while the team is dead: how many members clicked « Prêt » and the time left before the reset is forced.
 *
 * {@code (0, 0, 0)} (total == 0 and secondsLeft == 0) is the explicit "clip over" signal, sent at every reset:
 * the client closes the clip screen.
 *
 * @param ready     members who clicked « Prêt »
 * @param total       online team members
 * @param secondsLeft seconds before the reset (forced at the max wait, or short once everybody is ready)
 */
public record ReadyStatePayload(int ready, int total, int secondsLeft) implements CustomPacketPayload {
	public static final Type<ReadyStatePayload> TYPE = new Type<>(ClassroomRunNetworking.id("ready_state"));
	public static final StreamCodec<ByteBuf, ReadyStatePayload> CODEC = StreamCodec.composite(
			ByteBufCodecs.VAR_INT, ReadyStatePayload::ready,
			ByteBufCodecs.VAR_INT, ReadyStatePayload::total,
			ByteBufCodecs.VAR_INT, ReadyStatePayload::secondsLeft,
			ReadyStatePayload::new);

	@Override
	public Type<ReadyStatePayload> type() {
		return TYPE;
	}
}
