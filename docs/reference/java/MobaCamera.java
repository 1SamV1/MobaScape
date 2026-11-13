
import java.awt.event.KeyEvent;

public final class MobaCamera {

    private static double targetX = MobaClient.ARENA_CENTER_X * 128 + 64;
    private static double targetY = MobaClient.ARENA_CENTER_Y * 128 + 64;

    private static int pitch = 300;
    private static int yaw = 0;
    private static double zoom = 1.0;

    public static int keyDownCount;
    private static final boolean[] held = new boolean[256];
    private static long lastTickNanos;

    private MobaCamera() {
    }

    public static boolean keyDown(int keyCode) {
        if (keyCode < 0 || keyCode >= held.length) {
            return false;
        }
        if (MobaClient.isCameraKey(keyCode)) {
            held[keyCode] = true;
            keyDownCount++;
            return true;
        }
        return false;
    }

    public static void keyUp(int keyCode) {
        if (keyCode >= 0 && keyCode < held.length) {
            held[keyCode] = false;
        }
    }

    public static void clearKeys() {
        for (int i = 0; i < held.length; i++) {
            held[i] = false;
        }
    }

    public static int getPitch() {
        return pitch;
    }

    public static double getZoom() {
        return zoom;
    }

    public static int getTileX() {
        return (int) (targetX / 128);
    }

    public static int getTileY() {
        return (int) (targetY / 128);
    }

    /** MOBA dev control. */
    public static void setTargetTile(int tileX, int tileY) {
        targetX = tileX * 128 + 64;
        targetY = tileY * 128 + 64;
        clamp();
    }

    public static void setZoom(double value) {
        zoom = value;
        if (zoom < 0.55) zoom = 0.55;
        if (zoom > 1.65) zoom = 1.65;
    }

    public static void setYaw(int value) {
        yaw = value & 0x7ff;
    }

    public static void setPitch(int value) {
        pitch = value;
        if (pitch < 128) pitch = 128;
        if (pitch > 383) pitch = 383;
    }

    public static int getYaw() {
        return yaw;
    }

    public static String status() {
        return "CAM tile=" + getTileX() + "," + getTileY()
                + " zoom=" + String.format(java.util.Locale.ROOT, "%.2f", zoom)
                + " yaw=" + yaw + " pitch=" + pitch;
    }

    public static void recentre() {
        targetX = MobaClient.ARENA_CENTER_X * 128 + 64;
        targetY = MobaClient.ARENA_CENTER_Y * 128 + 64;
        pitch = 300;
        yaw = 0;
        zoom = 1.0;
    }

    public static void zoomBy(int notches) {
        zoom -= notches * 0.12;
        if (zoom < 0.55) {
            zoom = 0.55;
        }
        if (zoom > 1.65) {
            zoom = 1.65;
        }
    }

    public static void tick(int mouseX, int mouseY, int viewportWidth, int viewportHeight) {
        long now = System.nanoTime();
        double dt = lastTickNanos == 0 ? 0.016 : (now - lastTickNanos) / 1_000_000_000.0;
        lastTickNanos = now;
        if (dt > 0.1) {
            dt = 0.1;
        }

        double speed = 18.0 * 128.0 * dt;
        double dx = 0;
        double dy = 0;
        if (held[KeyEvent.VK_W] || held[KeyEvent.VK_UP]) {
            dy += speed;
        }
        if (held[KeyEvent.VK_S] || held[KeyEvent.VK_DOWN]) {
            dy -= speed;
        }
        if (held[KeyEvent.VK_A] || held[KeyEvent.VK_LEFT]) {
            dx -= speed;
        }
        if (held[KeyEvent.VK_D] || held[KeyEvent.VK_RIGHT]) {
            dx += speed;
        }

        // Edge pan only once the pointer has actually been over the client;
        // AWT defaults mouseX/mouseY to 0,0, which would otherwise pin the
        // camera to the bottom-left corner forever.
        int edge = 24;
        boolean mouseKnown = RSApplet.lastMouseActivity != 0;
        if (mouseKnown && mouseX >= 0 && mouseY >= 0) {
            if (mouseX < edge) {
                dx -= speed * (1.0 - mouseX / (double) edge);
            } else if (mouseX > viewportWidth - edge) {
                dx += speed * (1.0 - (viewportWidth - mouseX) / (double) edge);
            }
            if (mouseY < edge) {
                dy += speed * (1.0 - mouseY / (double) edge);
            } else if (mouseY > viewportHeight - edge) {
                dy -= speed * (1.0 - (viewportHeight - mouseY) / (double) edge);
            }
        }

        if (held[KeyEvent.VK_Q]) {
            yaw = (yaw - (int) (420 * dt)) & 0x7ff;
        }
        if (held[KeyEvent.VK_E]) {
            yaw = (yaw + (int) (420 * dt)) & 0x7ff;
        }
        if (held[KeyEvent.VK_SPACE]) {
            recentre();
        }

        targetX += dx;
        targetY += dy;
        clamp();
    }

    private static void clamp() {
        double minX = MobaClient.ARENA_MIN_X * 128.0 + 64;
        double maxX = MobaClient.ARENA_MAX_X * 128.0 + 64;
        double minY = MobaClient.ARENA_MIN_Y * 128.0 + 64;
        double maxY = MobaClient.ARENA_MAX_Y * 128.0 + 64;
        if (targetX < minX) targetX = minX;
        if (targetX > maxX) targetX = maxX;
        if (targetY < minY) targetY = minY;
        if (targetY > maxY) targetY = maxY;
        // Keep the look-at point inside the visible scene with a margin for the
        // camera's backward offset at max pitch (values are tiles * 128).
        double margin = 8 * 128.0;
        double sceneMinX = 2808 * 128.0 + 64;
        double sceneMaxX = 2912 * 128.0 + 64;
        double sceneMinY = 3008 * 128.0 + 64;
        double sceneMaxY = 3112 * 128.0 + 64;
        if (targetX < sceneMinX + margin) targetX = sceneMinX + margin;
        if (targetX > sceneMaxX - margin) targetX = sceneMaxX - margin;
        if (targetY < sceneMinY + margin) targetY = sceneMinY + margin;
        if (targetY > sceneMaxY - margin) targetY = sceneMaxY - margin;
    }

    public static void apply(Game game) {
        // The world renderer works in scene-local units (0..104*128), not world
        // tiles: subtract the client's pinned scene base.
        int worldX = (int) Math.round(targetX);
        int worldY = (int) Math.round(targetY);
        int camX = worldX - game.baseX * 128;
        int camY = worldY - game.baseY * 128;
        int ground = game.method42(game.plane, camY, camX);
        int distance = (int) Math.round((820 + pitch * 2.4) * zoom);
        game.setCameraPos(distance, pitch, camX, ground - 70, yaw, camY);
    }
}
