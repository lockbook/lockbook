package app.lockbook.util

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Assert.fail
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class OpenLinkBuilderTest {
    private val id = "a6743b18-c7ef-4960-9825-8022e2fa5672"

    @Test
    fun buildsCanonicalLinkFromAccountOrigin() {
        assertEquals(
            "https://notes.example.com/open/$id",
            OpenLinkBuilder.build("https://Notes.Example.com:443/", id),
        )
    }

    @Test
    fun buildsIpv6Link() {
        assertEquals("https://[::1]:8443/open/$id", OpenLinkBuilder.build("https://[::1]:8443/", id))
    }

    @Test
    fun rejectsUnsafeAccountOrigin() {
        try {
            OpenLinkBuilder.build("http://notes.example.com", id)
            fail("Expected an invalid HTTP origin to be rejected")
        } catch (_: IllegalArgumentException) {
        }
    }
}
