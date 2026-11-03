package fr.classroomrun.net;

import io.netty.buffer.ByteBuf;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.common.custom.CustomPacketPayload;

/** Client → server: the player clicked « Prêt » on the death clip (the client already deleted its copy). */
public record ClipReadyPayload() implements CustomPacketPayload {
	public static final ClipReadyPayload INSTANCE = new ClipReadyPayload();
	public static final Type<ClipReadyPayload> TYPE = new Type<>(ClassroomRunNetworking.id("clip_ready"));
	public static final StreamCodec<ByteBuf, ClipReadyPayload> CODEC = StreamCodec.unit(INSTANCE);

	@Override
	public Type<ClipReadyPayload> type() {
		return TYPE;
	}
}
