package app.lockbook.util

import java.net.URI
import java.util.UUID

internal fun canonicalFileId(value: String): String? {
    val canonical = try {
        UUID.fromString(value).toString()
    } catch (_: IllegalArgumentException) {
        return null
    }
    return canonical.takeIf { it.equals(value, ignoreCase = true) }
}

object OpenLinkParser {
    fun parse(value: String): String? {
        val uri = runCatching { URI(value) }.getOrNull() ?: return null

        return when (uri.scheme?.lowercase()) {
            "http", "https" -> parseOfficialWebLink(uri)
            "lb" -> uri.host?.let(::canonicalFileId)
            else -> null
        }
    }

    private fun parseOfficialWebLink(uri: URI): String? {
        if (!uri.host.equals("app.lockbook.net", ignoreCase = true)) return null
        val parts = uri.rawPath?.split('/') ?: return null
        if (parts.size < 3 || parts[0] != "" || parts[1] != "open") return null
        return canonicalFileId(parts[2])
    }
}
