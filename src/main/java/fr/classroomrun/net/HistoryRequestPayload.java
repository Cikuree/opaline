package fr.classroomrun.net;

import io.netty.buffer.ByteBuf;
import net.minecraft.network.codec.ByteBufCodecs;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.common.custom.CustomPacketPayload;

/**
 * Client → server: asks for the death history. The server answers with a {@link HistoryPayload}.
 *
 * @param runNumber run to detail, or {@link #INDEX} for the list of runs
 */
public record HistoryRequestPayload(int runNumber) implements CustomPacketPayload {
	public static final int INDEX = -1;
	public static final Type<HistoryRequestPayload> TYPE = new Type<>(ClassroomRunNetworking.id("history_request"));
	public static final StreamCodec<ByteBuf, HistoryRequestPayload> CODEC = StreamCodec.composite(
			ByteBufCodecs.VAR_INT, HistoryRequestPayload::runNumber,
			HistoryRequestPayload::new);

	@Override
	public Type<HistoryRequestPayload> type() {
		return TYPE;
	}
}
