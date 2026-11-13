package com.rs2.moba;

import com.rs2.game.players.Player;

/**
 * Server-side MOBA match/bootstrap state (v1).
 *
 * One match per server process. For now this ensures every player is placed in
 * the Karamja arena on login so the total-conversion client always has a valid
 * local position inside the pinned 104x104 scene.
 */
public final class MobaManager {

    public static final int SPAWN_X = 2860;
    public static final int SPAWN_Y = 3060;
    public static final int SPAWN_H = 0;

    private static boolean initialized = false;

    private MobaManager() {
    }

    public static synchronized void init() {
        initialized = true;
    }

    public static boolean isInitialized() {
        return initialized;
    }

    public static void onLogin(Player player) {
        if (player == null) {
            return;
        }
        player.getPlayerAssistant().movePlayer(SPAWN_X, SPAWN_Y, SPAWN_H);
        player.getPacketSender().sendMessage("Welcome to the Karamja MOBA arena (dev build).");
    }

    public static void onTick() {
        // v1: match/lobby state, fog, camps, waves, etc. will live here.
    }
}
