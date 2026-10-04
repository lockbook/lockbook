package app.lockbook.util

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class PendingOpenLinkStoreTest {
    @Test
    fun readingDoesNotConsumeAndAcknowledgingDoesNotDiscardANewerLink() {
        // Use the test APK's preferences, not the signed-in application's state.
        val context = InstrumentationRegistry.getInstrumentation().context
        val first = "a6743b18-c7ef-4960-9825-8022e2fa5672"
        val second = "b6743b18-c7ef-4960-9825-8022e2fa5672"
        try {
            PendingOpenLinkStore.save(context, first)
            assertEquals(first, PendingOpenLinkStore.peek(context))
            assertEquals(first, PendingOpenLinkStore.peek(context))
            PendingOpenLinkStore.save(context, second)
            PendingOpenLinkStore.acknowledge(context, first)
            assertEquals(second, PendingOpenLinkStore.peek(context))
            PendingOpenLinkStore.acknowledge(context, second)
            assertNull(PendingOpenLinkStore.peek(context))
        } finally {
            PendingOpenLinkStore.acknowledge(context, first)
            PendingOpenLinkStore.acknowledge(context, second)
        }
    }
}
