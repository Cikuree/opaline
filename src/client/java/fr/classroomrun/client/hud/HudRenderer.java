package fr.classroomrun.client.hud;

import fr.classroomrun.client.hud.DamageFeed.Entry;
import fr.classroomrun.client.mixin.BossHealthOverlayAccessor;
import fr.classroomrun.net.Split;
import java.util.UUID;
import net.fabricmc.fabric.api.client.rendering.v1.hud.HudElement;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.PlayerFaceExtractor;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.client.multiplayer.PlayerInfo;
import net.minecraft.client.resources.DefaultPlayerSkin;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.world.entity.ai.attributes.Attributes;
import net.minecraft.world.entity.player.Player;
import net.minecraft.util.Mth;
import net.minecraft.util.Util;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;

/** Draws the split bar and the damage feed. No allocation in the render path once the item stacks are built. */
public final class HudRenderer {
	private static final int CELL = 20;
	private static final int PAD = 3;
	private static final int TOP = 3;
	private static final int PINK = 0xFFFF69B4;
	private static final int CYAN = 0xFF55FFFF;
	private static final int PANEL = 0x90000000;
	private static final int DIM = 0xB0101010;
	private static final int RED = 0xFF5555;
	private static final int GREEN = 0x55FF55;
	private static final int HEAD = 8;
	private static final int FEED_GAP = 6;
	private static final float FEED_RISE = 12f;

	private static ItemStack[] splitStacks;
	private static ItemStack skull;
	private static final Entry[] ORDERED = new Entry[DamageFeed.CAPACITY];

	private static String lastDeathsText;
	private static int deathsWidth;

	private HudRenderer() {
	}

	public static HudElement splitsElement() {
		return HudRenderer::extractSplits;
	}

	public static HudElement feedElement() {
		return HudRenderer::extractFeed;
	}

	private static void initStacks() {
		Split[] splits = Split.values();
		splitStacks = new ItemStack[splits.length];
		for (int i = 0; i < splits.length; i++) {
			Item item = BuiltInRegistries.ITEM.getValue(Identifier.fromNamespaceAndPath("minecraft", splits[i].icon));
			splitStacks[i] = new ItemStack(item);
		}
		skull = new ItemStack(Items.SKELETON_SKULL);
	}

	private static void extractSplits(GuiGraphicsExtractor g, DeltaTracker delta) {
		ClientRunState run = ClientState.RUN;
		if (!run.present()) {
			return;
		}
		if (splitStacks == null) {
			initStacks();
		}
		Font font = Minecraft.getInstance().font;
		long now = Util.getMillis();
		int n = splitStacks.length;
		int rowW = n * CELL;
		// Centered, just under the boss bars so it never covers them.
		int top = HudFormat.splitsTop(
				((BossHealthOverlayAccessor) Minecraft.getInstance().gui.hud.getBossOverlay()).classroomrun$events().size(),
				g.guiHeight(), TOP);
		int left = (g.guiWidth() - rowW) / 2;
		int right = left + rowW;
		int iconY = top + PAD;
		int rowY = iconY + CELL + 4;
		int bottom = rowY + 16 + PAD;
		int current = run.currentSplit();

		// Pass 1: panel and item icons.
		g.fill(left - PAD, top, right + PAD, bottom, PANEL);
		for (int i = 0; i < n; i++) {
			g.item(splitStacks[i], left + i * CELL + 2, iconY + 2);
		}
		g.nextStratum();

		// Pass 2 (above the items): dimming of future splits, frame, arrow, clock, deaths.
		for (int i = 0; i < n; i++) {
			int x = left + i * CELL;
			if (i != current && !run.reached(i)) {
				g.fill(x + 1, iconY + 1, x + CELL - 1, iconY + CELL - 1, DIM);
			}
		}
		if (current >= 0 && current < n) {
			int x = left + current * CELL;
			int y = iconY;
			g.fill(x, y, x + CELL, y + 1, PINK);
			g.fill(x, y + CELL - 1, x + CELL, y + CELL, PINK);
			g.fill(x, y + 1, x + 1, y + CELL - 1, PINK);
			g.fill(x + CELL - 1, y + 1, x + CELL, y + CELL - 1, PINK);
			int cx = x + CELL / 2;
			int ay = y + CELL + 1;
			g.fill(cx, ay, cx + 1, ay + 1, PINK);
			g.fill(cx - 1, ay + 1, cx + 2, ay + 2, PINK);
			g.fill(cx - 2, ay + 2, cx + 3, ay + 3, PINK);
		}
		int textY = rowY + (16 - font.lineHeight) / 2 + 1;
		g.text(font, run.clockText(now), left, textY, CYAN);
		g.item(skull, right - 16, rowY);
		String deaths = run.deathsText();
		if (deaths != lastDeathsText) {
			lastDeathsText = deaths;
			deathsWidth = font.width(deaths);
		}
		g.text(font, deaths, right - 18 - deathsWidth, textY, 0xFFFFFFFF);
	}

	/** Top y of the highest heart/armor row, mirroring Hud.extractPlayerHealth and extractArmor (26.3). */
	private static int topOfStatusRows(Minecraft mc, int guiHeight) {
		int base = guiHeight - 39;
		Player player = mc.player;
		if (player == null) {
			return base;
		}
		float maxHealth = (float) player.getAttributeValue(Attributes.MAX_HEALTH);
		int rows = Mth.ceil((Math.max(maxHealth, player.getHealth()) + Mth.ceil(player.getAbsorptionAmount())) / 20.0F);
		int rowHeight = Math.max(10 - (rows - 2), 3);
		int top = base - (rows - 1) * rowHeight;
		return player.getArmorValue() > 0 ? top - 10 : top;
	}

	private static void extractFeed(GuiGraphicsExtractor g, DeltaTracker delta) {
		DamageFeed feed = ClientState.FEED;
		long now = Util.getMillis();
		feed.expire(now);
		int count = feed.ordered(ORDERED);
		if (count == 0) {
			return;
		}
		Minecraft mc = Minecraft.getInstance();
		Font font = mc.font;
		ClientPacketListener conn = mc.getConnection();
		int baseY = topOfStatusRows(mc, g.guiHeight()) - 12;
		int x = g.guiWidth() / 2 - 91;
		for (int i = 0; i < count; i++) {
			Entry e = ORDERED[i];
			if (e.textWidth < 0) {
				e.textWidth = font.width(e.text);
			}
			float p = DamageFeed.progress(e, now);
			int a = (int) (DamageFeed.alpha(p) * 255f);
			if (a >= 8) {
				int y = baseY - (int) DamageFeed.rise(p, FEED_RISE);
				UUID id = e.player;
				PlayerInfo info = conn == null ? null : conn.getPlayerInfo(id);
				PlayerFaceExtractor.extractRenderState(g, info != null ? info.getSkin() : DefaultPlayerSkin.get(id), x, y,
						HEAD, (a << 24) | 0xFFFFFF);
				g.text(font, e.text, x + HEAD + 2, y, (a << 24) | (e.heal ? GREEN : RED));
			}
			x += HEAD + 2 + e.textWidth + FEED_GAP;
		}
	}
}
