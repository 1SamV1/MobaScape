package com.rs2.moba;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;
import java.io.PrintWriter;
import java.net.InetAddress;
import java.net.ServerSocket;
import java.net.Socket;
import java.nio.charset.StandardCharsets;

import com.rs2.game.players.Client;
import com.rs2.game.players.Player;
import com.rs2.game.players.PlayerHandler;
import com.rs2.world.clip.Region;

/**
 * MOBA dev console.
 *
 * A tiny line-oriented TCP console used by the dev MCP / automation harness so
 * that tests never depend on X11 chat input. Listens on 127.0.0.1 only.
 *
 * Protocol: one command per line, one response per command terminated by \n.
 *   ping
 *   mapdump <centerX> <centerY> [size]
 *   players
 *   cmd <playerName> <command...>   (runs a normal ::command as that player)
 */
public final class MobaDevConsole {

    public static final int PORT = 43596;

    private MobaDevConsole() {
    }

    public static void start() {
        Thread thread = new Thread(new Runnable() {
            @Override
            public void run() {
                try {
                    ServerSocket server = new ServerSocket(PORT, 8, InetAddress.getByName("127.0.0.1"));
                    log("MOBA dev console listening on 127.0.0.1:" + PORT);
                    while (!server.isClosed()) {
                        final Socket socket = server.accept();
                        Thread client = new Thread(new Runnable() {
                            @Override
                            public void run() {
                                handle(socket);
                            }
                        }, "moba-dev-console-client");
                        client.setDaemon(true);
                        client.start();
                    }
                } catch (IOException e) {
                    log("MOBA dev console failed to start: " + e);
                }
            }
        }, "moba-dev-console");
        thread.setDaemon(true);
        thread.start();
    }

    private static void handle(Socket socket) {
        try {
            socket.setSoTimeout(120_000);
            BufferedReader in = new BufferedReader(
                    new InputStreamReader(socket.getInputStream(), StandardCharsets.UTF_8));
            PrintWriter out = new PrintWriter(socket.getOutputStream(), true);
            String line;
            while ((line = in.readLine()) != null) {
                line = line.trim();
                if (line.isEmpty()) {
                    continue;
                }
                try {
                    out.print(dispatch(line));
                } catch (Exception e) {
                    out.print("ERROR " + e + "\n");
                }
                out.flush();
            }
        } catch (IOException ignored) {
        } finally {
            try {
                socket.close();
            } catch (IOException ignored) {
            }
        }
    }

    private static String dispatch(String line) {
        String[] parts = line.split("\\s+");
        String command = parts[0].toLowerCase();

        switch (command) {
            case "ping":
                return "PONG\n";
            case "mapdump":
                return mapdump(parts);
            case "players":
                return players();
            case "cmd":
                return runAsPlayer(parts);
            default:
                return "ERROR unknown command: " + command + "\n";
        }
    }

    private static String mapdump(String[] parts) {
        if (parts.length < 3) {
            return "ERROR usage: mapdump <centerX> <centerY> [size]\n";
        }
        int cx = Integer.parseInt(parts[1]);
        int cy = Integer.parseInt(parts[2]);
        int size = parts.length > 3 ? Integer.parseInt(parts[3]) : 104;
        int half = size / 2;
        StringBuilder sb = new StringBuilder();
        sb.append("MAPDUMP center=").append(cx).append(',').append(cy)
                .append(" size=").append(size).append('\n');
        for (int y = cy + half - 1; y >= cy - half; y--) {
            for (int x = cx - half; x < cx + half; x++) {
                sb.append(Region.getClipping(x, y, 0) == 0 ? '.' : '#');
            }
            sb.append('\n');
        }
        sb.append("END\n");
        return sb.toString();
    }

    private static String players() {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < PlayerHandler.players.length; i++) {
            Player p = PlayerHandler.players[i];
            if (p == null) {
                continue;
            }
            sb.append("PLAYER ").append(i)
                    .append(' ').append(p.playerName)
                    .append(" x=").append(p.absX)
                    .append(" y=").append(p.absY)
                    .append(" h=").append(p.heightLevel)
                    .append(" rights=").append(p.playerRights)
                    .append(" npc=").append(p.isNpc ? p.npcId2 : -1)
                    .append('\n');
        }
        sb.append("END\n");
        return sb.toString();
    }

    private static String runAsPlayer(String[] parts) {
        if (parts.length < 3) {
            return "ERROR usage: cmd <playerName> <command...>\n";
        }
        String name = parts[1];
        Player found = null;
        for (int i = 0; i < PlayerHandler.players.length; i++) {
            Player p = PlayerHandler.players[i];
            if (p != null && p.playerName != null && p.playerName.equalsIgnoreCase(name)) {
                found = p;
                break;
            }
        }
        if (found == null) {
            return "ERROR player not online: " + name + "\n";
        }
        StringBuilder command = new StringBuilder();
        for (int i = 2; i < parts.length; i++) {
            if (i > 2) {
                command.append(' ');
            }
            command.append(parts[i]);
        }
        String raw = command.toString();
        if (raw.startsWith("::")) {
            raw = raw.substring(2);
        }
        try {
            com.rs2.net.packets.impl.Commands.execute(found, raw);
            return "OK\n";
        } catch (Exception e) {
            return "ERROR " + e + "\n";
        }
    }

    private static void log(String message) {
        System.out.println(message);
    }
}
