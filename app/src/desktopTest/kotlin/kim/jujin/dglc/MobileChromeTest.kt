package kim.jujin.dglc

import androidx.compose.ui.unit.*
import kim.jujin.dglc.ui.NotificationPopupPosition
import kotlin.test.*

class MobileChromeTest {
    @Test fun notificationPositionRespectsAnchorScreenInsetsAndKeyboard() {
        val phone = NotificationPopupPosition(IntRect(0, 24, 320, 740), 8)
        val anchor = IntRect(224, 28, 272, 76)
        assertEquals(IntOffset(72, 76), phone.calculatePosition(anchor, IntSize(320, 760), LayoutDirection.Ltr, IntSize(200, 300)))
        assertEquals(IntOffset(8, 76), phone.calculatePosition(anchor, IntSize(320, 760), LayoutDirection.Rtl, IntSize(304, 480)))
        val keyboard = NotificationPopupPosition(IntRect(16, 24, 823, 400), 8)
        assertEquals(IntOffset(444, 76), keyboard.calculatePosition(IntRect(756, 28, 804, 76), IntSize(839, 1000),
            LayoutDirection.Ltr, IntSize(360, 316)))
    }
}
