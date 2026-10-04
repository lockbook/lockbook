package app.lockbook.util

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class OpenLinkParserTest {
    private val id = "a6743b18-c7ef-4960-9825-8022e2fa5672"

    @Test
    fun `parses exact official web link`() {
        assertEquals(
            id,
            OpenLinkParser.parse("https://app.lockbook.net/open/$id"),
        )
    }

    @Test
    fun `parses legacy app handoff`() {
        assertEquals(
            id,
            OpenLinkParser.parse("lb://$id"),
        )
    }

    @Test
    fun `official hostname and default port normalize without storing a server`() {
        assertEquals(id, OpenLinkParser.parse("HTTPS://APP.LOCKBOOK.NET:443/open/${id.uppercase()}"))
        assertNull(OpenLinkParser.parse("https://app.lockbook.net:8443/open/$id"))
        assertNull(OpenLinkParser.parse("https://other.example/open/$id"))
    }

    @Test
    fun `rejects removed server aware handoffs`() {
        assertNull(OpenLinkParser.parse("lb://open?server=https%3A%2F%2Fapp.lockbook.net&file=$id"))
        assertNull(OpenLinkParser.parse("lb://$id?server=https%3A%2F%2Fapp.lockbook.net"))
    }

    @Test
    fun `validates file ids independently of link parsing`() {
        assertEquals(id, canonicalFileId(id.uppercase()))
        assertNull(canonicalFileId("not-a-uuid"))
        assertNull(canonicalFileId("lb://$id"))
    }

    @Test
    fun `rejects unsafe and malformed links`() {
        val invalid =
            listOf(
                "https:///open/$id",
                "https:opaque",
                "lb:opaque",
                "lb:///open",
                "https://app.lockbook.net//open/$id",
                "https://app.lockbook.net/open/$id/",
                "http://app.lockbook.net/open/$id",
                "https://user@app.lockbook.net/open/$id",
                "https://app.lockbook.net/open/$id#fragment",
                "https://app.lockbook.net/open/$id?extra=true",
                "https://app.lockbook.net/other/$id",
                "https://app.lockbook.net/open/not-a-uuid",
            )
        invalid.forEach { assertNull(it, OpenLinkParser.parse(it)) }
    }

    @Test
    fun `ipv6 origins retain exactly one pair of brackets`() {
        assertEquals("https://[::1]:8443", OpenLinkBuilder.canonicalOrigin("https://[::1]:8443/"))
        assertEquals(id, OpenLinkParser.parse("lb://$id"))
    }
}
