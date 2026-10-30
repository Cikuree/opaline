package fr.classroomrun.net;

import io.netty.buffer.ByteBuf;
import net.minecraft.network.codec.ByteBufCodecs;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.common.custom.CustomPacketPayload;

/**
 * Server → client: answer to a {@link HistoryRequestPayload}. The JSON schemas are defined in CONTRACT.md.
 *
 * @param runNumber the requested run, or {@link HistoryRequestPayload#INDEX} for the run list
 * @param json      index JSON or run detail JSON (UTF-8, at most {@link #MAX_JSON} chars)
 * @param httpPort  port of the server's HTTP endpoint for replay downloads, 0 if disabled
 */
public record HistoryPayload(int runNumber, String json, int httpPort) implements CustomPacketPayload {
	public static final int MAX_JSON = 262_144;
	public static final Type<HistoryPayload> TYPE = new Type<>(ClassroomRunNetworking.id("history"));
	public static final StreamCodec<ByteBuf, HistoryPayload> CODEC = StreamCodec.composite(
			ByteBufCodecs.VAR_INT, HistoryPayload::runNumber,
			ByteBufCodecs.stringUtf8(MAX_JSON), HistoryPayload::json,
			ByteBufCodecs.VAR_INT, HistoryPayload::httpPort,
			HistoryPayload::new);

	@Override
	public Type<HistoryPayload> type() {
		return TYPE;
	}
}
