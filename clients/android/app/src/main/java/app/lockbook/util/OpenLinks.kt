package app.lockbook.util

import java.net.URI
import java.util.UUID

private val uuidPattern = Regex("^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$")

internal fun canonicalFileId(value: String): String? {
    if (!uuidPattern.matches(value)) return null
    return UUID.fromString(value).toString()
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
