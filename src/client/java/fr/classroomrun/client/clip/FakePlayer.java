package fr.classroomrun.client.clip;

import com.mojang.authlib.GameProfile;
import java.util.UUID;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.client.multiplayer.PlayerInfo;
import net.minecraft.client.player.RemotePlayer;
import net.minecraft.client.resources.DefaultPlayerSkin;
import net.minecraft.world.entity.player.PlayerSkin;

/**
 * Client-only player of a clip. Its entity UUID is random (the real player with the same UUID is in the level, and the level
 * rejects duplicates); the skin is looked up in the tab list with {@code skinOwner}.
 */
final class FakePlayer extends RemotePlayer {
	private static final int FALL_FLYING_FLAG = 7;
	private static final int GLOWING_FLAG = 6;

	private final UUID skinOwner;

	FakePlayer(ClientLevel level, GameProfile randomProfile, UUID skinOwner) {
		super(level, randomProfile);
		this.skinOwner = skinOwner;
	}

	@Override
	public PlayerSkin getSkin() {
		ClientPacketListener connection = Minecraft.getInstance().getConnection();
		PlayerInfo info = connection == null ? null : connection.getPlayerInfo(this.skinOwner);
		return info == null ? DefaultPlayerSkin.get(this.skinOwner) : info.getSkin();
	}

	/** Client-side glowing outline: the flag is what {@code isCurrentlyGlowing} reads on the client, no mixin needed. */
	void setClipGlowing() {
		setSharedFlag(GLOWING_FLAG, true);
	}

	void setClipFallFlying(boolean fallFlying) {
		setSharedFlag(FALL_FLYING_FLAG, fallFlying);
	}
}
