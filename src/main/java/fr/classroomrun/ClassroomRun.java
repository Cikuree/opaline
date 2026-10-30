package fr.classroomrun;

import fr.classroomrun.net.ClassroomRunNetworking;
import fr.classroomrun.game.RunManager;
import fr.classroomrun.journal.JournalService;
import net.fabricmc.api.ModInitializer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

public class ClassroomRun implements ModInitializer {
	public static final String MOD_ID = "classroomrun";
	public static final Logger LOGGER = LoggerFactory.getLogger(MOD_ID);

	@Override
	public void onInitialize() {
		ClassroomRunNetworking.registerPayloads();
		RunManager.register();
		JournalService.register();
		LOGGER.info("Classroom Run initialised");
	}
}
