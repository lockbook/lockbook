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
    fun preservesHttpForLocalDevelopment() {
        assertEquals("http://10.0.2.2:8000/open/$id", OpenLinkBuilder.build("http://10.0.2.2:8000", id))
        assertEquals("http://notes.example.com/open/$id", OpenLinkBuilder.build("http://Notes.Example.com:80/", id))
    }

    @Test
    fun rejectsUnsafeAccountOrigin() {
        val unsafeOrigins =
            listOf(
                "ftp://notes.example.com",
                "http://user@notes.example.com",
                "http://notes.example.com/api",
                "http://notes.example.com?x=1",
                "http://notes.example.com#fragment",
            )
        for (origin in unsafeOrigins) {
            try {
                OpenLinkBuilder.build(origin, id)
                fail("Expected an invalid origin to be rejected: $origin")
            } catch (_: IllegalArgumentException) {
            }
        }
    }
}
