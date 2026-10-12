package app.lockbook.util

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class OpenLinkParserTest {
    private val id = "a6743b18-c7ef-4960-9825-8022e2fa5672"

    @Test
    fun `parses official web link`() {
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
    fun `official hostname and UUID normalize without storing a server`() {
        assertEquals(id, OpenLinkParser.parse("HTTPS://APP.LOCKBOOK.NET:443/open/${id.uppercase()}"))
        assertEquals(id, OpenLinkParser.parse("http://app.lockbook.net:8443/open/$id"))
        assertNull(OpenLinkParser.parse("https://other.example/open/$id"))
    }

    @Test
    fun `rejects removed server aware handoff`() {
        assertNull(OpenLinkParser.parse("lb://open?server=https%3A%2F%2Fapp.lockbook.net&file=$id"))
    }

    @Test
    fun `ignores suffixes after the file ID like Apple`() {
        assertEquals(id, OpenLinkParser.parse("https://app.lockbook.net/open/$id/extra?source=chat#preview"))
        assertEquals(id, OpenLinkParser.parse("lb://$id/extra?source=chat#preview"))
        assertEquals(id, OpenLinkParser.parse("https://user@app.lockbook.net/open/$id"))
    }

    @Test
    fun `validates file ids independently of link parsing`() {
        assertEquals(id, canonicalFileId(id.uppercase()))
        assertEquals("00000000-0000-0000-0000-000000000000", canonicalFileId("00000000-0000-0000-0000-000000000000"))
        assertNull(canonicalFileId("not-a-uuid"))
        assertNull(canonicalFileId("1-1-1-1-1"))
        assertNull(canonicalFileId("lb://$id"))
    }

    @Test
    fun `rejects links without a supported host route and UUID`() {
        val invalid =
            listOf(
                "https:///open/$id",
                "https:opaque",
                "lb:opaque",
                "lb:///open",
                "https://app.lockbook.net//open/$id",
                "ftp://app.lockbook.net/open/$id",
                "https://app.lockbook.net/other/$id",
                "https://app.lockbook.net/open/not-a-uuid",
            )
        invalid.forEach { assertNull(it, OpenLinkParser.parse(it)) }
    }
}
