package fr.classroomrun.command;

import com.mojang.brigadier.CommandDispatcher;
import fr.classroomrun.game.RunManager;
import net.minecraft.commands.CommandSourceStack;
import net.minecraft.commands.Commands;
import net.minecraft.network.chat.Component;

/** {@code /run start|reset|stats}, permission level 2. */
public final class RunCommand {
	private RunCommand() {
	}

	public static void register(CommandDispatcher<CommandSourceStack> dispatcher) {
		dispatcher.register(Commands.literal("run")
				.requires(Commands.hasPermission(Commands.LEVEL_GAMEMASTERS))
				.then(Commands.literal("start").executes(ctx -> {
					RunManager run = RunManager.get();
					if (run == null || !run.startRun()) {
						ctx.getSource().sendFailure(Component.literal("La run est deja demarree"));
						return 0;
					}
					ctx.getSource().sendSuccess(() -> Component.literal("Run demarree"), true);
					return 1;
				}))
				.then(Commands.literal("reset").executes(ctx -> {
					RunManager run = RunManager.get();
					if (run == null) {
						return 0;
					}
					run.reset();
					ctx.getSource().sendSuccess(() -> Component.literal("Run reinitialisee : " + run.describe()), true);
					return 1;
				}))
				.then(Commands.literal("stats").executes(ctx -> {
					RunManager run = RunManager.get();
					if (run == null) {
						return 0;
					}
					ctx.getSource().sendSuccess(() -> Component.literal(run.describe()), false);
					return 1;
				})));
	}
}
