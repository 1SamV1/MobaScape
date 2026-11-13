
import java.awt.event.KeyEvent;

public final class MobaClient {

    public static boolean ACTIVE = true;

    public static final int ARENA_CENTER_X = 2860;
    public static final int ARENA_CENTER_Y = 3060;
    public static final int ARENA_HALF = 52;

    public static final int SCENE_REGION_X = ARENA_CENTER_X / 8; // 357
    public static final int SCENE_REGION_Y = ARENA_CENTER_Y / 8; // 382

    public static final int ARENA_MIN_X = ARENA_CENTER_X - ARENA_HALF;
    public static final int ARENA_MAX_X = ARENA_CENTER_X + ARENA_HALF;
    public static final int ARENA_MIN_Y = ARENA_CENTER_Y - ARENA_HALF;
    public static final int ARENA_MAX_Y = ARENA_CENTER_Y + ARENA_HALF;

    private MobaClient() {
    }

    public static boolean isCameraKey(int keyCode) {
        switch (keyCode) {
            case KeyEvent.VK_W:
            case KeyEvent.VK_A:
            case KeyEvent.VK_S:
            case KeyEvent.VK_D:
            case KeyEvent.VK_UP:
            case KeyEvent.VK_DOWN:
            case KeyEvent.VK_LEFT:
            case KeyEvent.VK_RIGHT:
            case KeyEvent.VK_Q:
            case KeyEvent.VK_E:
            case KeyEvent.VK_SPACE:
                return true;
            default:
                return false;
        }
    }
}
