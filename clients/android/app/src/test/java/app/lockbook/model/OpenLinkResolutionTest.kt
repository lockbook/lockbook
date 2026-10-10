package app.lockbook.model

import net.lockbook.LbError
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

class OpenLinkResolutionTest {
    private val request = "a6743b18-c7ef-4960-9825-8022e2fa5672"

    @Test
    fun `syncs and resolves an available file`() {
        var synced = false
        val result = resolveOpenLink(request, { synced = true }, { true })
        assertTrue(synced)
        assertTrue(result.fileFound)
    }

    @Test
    fun `syncs before looking up the requested file`() {
        val calls = mutableListOf<String>()
        val result =
            resolveOpenLink(
                request,
                { calls.add("sync") },
                {
                    calls.add(it)
                    true
                },
            )
        assertEquals(listOf("sync", request), calls)
        assertTrue(result.fileFound)
    }

    @Test
    fun `sync failure still allows opening an available file`() {
        val failure = LbError().apply { kind = LbError.LbEC.ServerUnreachable }
        val result = resolveOpenLink(request, { throw failure }, { true })
        assertTrue(result.fileFound)
        assertSame(failure, result.error)
    }

    @Test
    fun `missing files distinguish successful sync from failed sync`() {
        val missing = resolveOpenLink(request, {}, { false })
        assertFalse(missing.fileFound)
        assertNull(missing.error)
        val failure = LbError().apply { kind = LbError.LbEC.ServerUnreachable }
        val failed = resolveOpenLink(request, { throw failure }, { false })
        assertFalse(failed.fileFound)
        assertSame(failure, failed.error)
    }
}
