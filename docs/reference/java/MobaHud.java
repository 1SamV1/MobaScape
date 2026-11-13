
import java.util.Locale;

public final class MobaHud {

    private static final int BAR = 0x0b0d10;
    private static final int ACCENT = 0xd8b24a;
    private static final int BLUE = 0x4aa3ff;
    private static final int RED = 0xff5a4a;
    private static final int TEXT = 0xe8e8e8;
    private static final int DIM = 0x9a9a9a;

    private MobaHud() {
    }

    public static void draw(Game game) {
        int w = ClientSettings.VIEWPORT_WIDTH;
        int h = ClientSettings.VIEWPORT_HEIGHT;

        DrawingArea.fillArea(BAR, 0, w, 30, 210, 0);
        DrawingArea.drawHorizontalLine(29, ACCENT, w, 0);

        game.chatTextDrawingArea.textCenterShadow(BLUE, 40, "BLUE", 13, true);
        game.chatTextDrawingArea.textCenterShadow(0xbfd8ff, 40, "0", 25, true);
        game.chatTextDrawingArea.textCenterShadow(RED, w - 40, "RED", 13, true);
        game.chatTextDrawingArea.textCenterShadow(0xffd0c8, w - 40, "0", 25, true);

        game.chatTextDrawingArea.textCenterShadow(ACCENT, w / 2, "KARAMJA ARENA", 13, true);
        game.chatTextDrawingArea.textCenterShadow(TEXT, w / 2, "00:00", 25, true);

        DrawingArea.fillArea(BAR, h - 58, w, 58, 210, 0);
        DrawingArea.drawHorizontalLine(h - 59, ACCENT, w, 0);

        game.chatTextDrawingArea.textLeftShadow(true, 14, TEXT, "MOBA DEV BUILD", h - 40);
        game.chatTextDrawingArea.textLeftShadow(true, 14, DIM, "Karamja total conversion", h - 24);

        String help = "WASD / edge pan  -  wheel zoom  -  Q/E rotate  -  space recentre";
        game.chatTextDrawingArea.textCenterShadow(DIM, w / 2, help, h - 30, true);

        String cam = "cam " + MobaCamera.getTileX() + "," + MobaCamera.getTileY()
                + "   zoom " + String.format(Locale.ROOT, "%.2f", MobaCamera.getZoom());
        game.chatTextDrawingArea.textRightShadow(true, w - 14, DIM, cam, h - 40);
    }
}
