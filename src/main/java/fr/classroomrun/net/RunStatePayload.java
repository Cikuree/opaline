package fr.classroomrun.net;

import io.netty.buffer.ByteBuf;
import net.minecraft.network.codec.ByteBufCodecs;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.common.custom.CustomPacketPayload;

/**
 * Full run state, sent on join, on every split, death, run start/reset, and every 5 s while running.
 *
 * @param totalTimeMs  cumulative time over all runs, in ms, at the moment of sending
 * @param running      true while the timer is ticking (client extrapolates with its own clock)
 * @param deaths       total team deaths over all runs
 * @param runNumber    current run number (starts at 1)
 * @param currentSplit index in {@link Split} of the next split to reach; Split.values().length when finished
 * @param reachedSplits bitmask of reached splits ({@link Split#bit()})
 */
public record RunStatePayload(long totalTimeMs, boolean running, int deaths, int runNumber, int currentSplit,
		int reachedSplits) implements CustomPacketPayload {
	public static final Type<RunStatePayload> TYPE = new Type<>(ClassroomRunNetworking.id("run_state"));
	public static final StreamCodec<ByteBuf, RunStatePayload> CODEC = StreamCodec.composite(
			ByteBufCodecs.VAR_LONG, RunStatePayload::totalTimeMs,
			ByteBufCodecs.BOOL, RunStatePayload::running,
			ByteBufCodecs.VAR_INT, RunStatePayload::deaths,
			ByteBufCodecs.VAR_INT, RunStatePayload::runNumber,
			ByteBufCodecs.VAR_INT, RunStatePayload::currentSplit,
			ByteBufCodecs.VAR_INT, RunStatePayload::reachedSplits,
			RunStatePayload::new);

	public boolean reached(Split split) {
		return (reachedSplits & split.bit()) != 0;
	}

	@Override
	public Type<RunStatePayload> type() {
		return TYPE;
	}
}
