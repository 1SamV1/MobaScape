import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;
import java.io.PrintWriter;
import java.net.InetAddress;
import java.net.ServerSocket;
import java.net.Socket;
import java.nio.charset.StandardCharsets;

/**
 * MOBA dev: client-side TCP control channel (localhost only).
 *
 * X11 synthetic input is unreliable with this AWT frame, so the automation
 * harness talks to the client directly here. Intended for development only.
 *
 * Commands (one per line):
 *   ping
 *   cam                          report camera state
 *   cam <tileX> <tileY>          move camera target to a tile
 *   zoom <value>                 set zoom multiplier (0.55..1.65)
 *   yaw <0..2047>                set camera yaw
 *   pitch <128..383>             set camera pitch
 *   recentre
 *   pos                          report local player tile
 *   follow                       centre camera on the local player
 */
public final class MobaDevClient {

    public static final int PORT = 43597;

    private static volatile Game game;

    private MobaDevClient() {
    }

    public static void attach(Game g) {
        game = g;
    }

    public static void start() {
        Thread thread = new Thread(new Runnable() {
            @Override
            public void run() {
                try {
                    ServerSocket server = new ServerSocket(PORT, 8, InetAddress.getByName("127.0.0.1"));
                    System.out.println("MOBA client console listening on 127.0.0.1:" + PORT);
                    while (!server.isClosed()) {
                        final Socket socket = server.accept();
                        Thread client = new Thread(new Runnable() {
                            @Override
                            public void run() {
                                handle(socket);
                            }
                        }, "moba-client-console");
                        client.setDaemon(true);
                        client.start();
                    }
                } catch (IOException e) {
                    System.out.println("MOBA client console failed: " + e);
                }
            }
        }, "moba-client-console-accept");
        thread.setDaemon(true);
        thread.start();
    }

    private static void handle(Socket socket) {
        try {
            socket.setSoTimeout(300_000);
            BufferedReader in = new BufferedReader(
                    new InputStreamReader(socket.getInputStream(), StandardCharsets.UTF_8));
            PrintWriter out = new PrintWriter(socket.getOutputStream(), true);
            String line;
            while ((line = in.readLine()) != null) {
                line = line.trim();
                if (line.isEmpty()) {
                    continue;
                }
                out.print(dispatch(line));
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
        String[] p = line.split("\\s+");
        String cmd = p[0].toLowerCase();
        if ("ping".equals(cmd)) {
            return "PONG\n";
        }
        if ("keydown".equals(cmd) && p.length >= 2) {
            MobaCamera.keyDown(Integer.parseInt(p[1]));
            return "OK keyDowns=" + MobaCamera.keyDownCount + "\n";
        }
        if ("keyup".equals(cmd) && p.length >= 2) {
            MobaCamera.keyUp(Integer.parseInt(p[1]));
            return "OK\n";
        }
        if ("input".equals(cmd)) {
            Game g = game;
            return "INPUT mouse=" + (g != null ? g.mouseX + "," + g.mouseY : "?")
                    + " lastActivity=" + RSApplet.lastMouseActivity
                    + " keyDowns=" + MobaCamera.keyDownCount + "\n";
        }
        if ("cam".equals(cmd)) {
            if (p.length >= 3) {
                MobaCamera.setTargetTile(Integer.parseInt(p[1]), Integer.parseInt(p[2]));
            }
            return MobaCamera.status() + "\n";
        }
        if ("zoom".equals(cmd) && p.length >= 2) {
            MobaCamera.setZoom(Double.parseDouble(p[1]));
            return MobaCamera.status() + "\n";
        }
        if ("yaw".equals(cmd) && p.length >= 2) {
            MobaCamera.setYaw(Integer.parseInt(p[1]));
            return MobaCamera.status() + "\n";
        }
        if ("pitch".equals(cmd) && p.length >= 2) {
            MobaCamera.setPitch(Integer.parseInt(p[1]));
            return MobaCamera.status() + "\n";
        }
        if ("recentre".equals(cmd)) {
            MobaCamera.recentre();
            return MobaCamera.status() + "\n";
        }
        if ("pos".equals(cmd)) {
            Game g = game;
            if (g == null || g.myPlayer == null) {
                return "NOPLAYER\n";
            }
            return "PLAYER tile=" + (g.baseX + g.myPlayer.smallX[0]) + "," + (g.baseY + g.myPlayer.smallY[0])
                    + " local=" + g.myPlayer.x + "," + g.myPlayer.y
                    + " base=" + g.baseX + "," + g.baseY
                    + " scene=" + g.anInt1069 + "," + g.anInt1070
                    + " stage=" + g.loadingStage + " plane=" + g.plane
                    + " cam=" + g.xCameraPos + "," + g.yCameraPos + "," + g.zCameraPos
                    + " curve=" + g.xCameraCurve + "," + g.yCameraCurve
                    + " ground=" + (g.byteGroundArray != null ? g.byteGroundArray[0][52][52] : -999)
                    + "\n";
        }
        if ("follow".equals(cmd)) {
            Game g = game;
            if (g == null || g.myPlayer == null) {
                return "NOPLAYER\n";
            }
            MobaCamera.setTargetTile(g.baseX + g.myPlayer.smallX[0], g.baseY + g.myPlayer.smallY[0]);
            return MobaCamera.status() + "\n";
        }
        return "ERROR unknown command: " + cmd + "\n";
    }
}
