package app.lockbook.util

import android.content.Context
import android.net.Uri
import androidx.core.content.edit
import androidx.core.net.toUri
import java.net.URI
import java.util.UUID

private val uuidPattern = Regex("^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[1-5][0-9a-fA-F]{3}-[89aAbB][0-9a-fA-F]{3}-[0-9a-fA-F]{12}$")

internal fun canonicalFileId(value: String): String? {
    if (!uuidPattern.matches(value)) return null
    return UUID.fromString(value).toString()
}

object OpenLinkParser {
    fun parse(value: String): String? {
        val uri = runCatching { URI(value) }.getOrNull() ?: return null
        if (uri.isOpaque || uri.host == null || uri.fragment != null || uri.rawUserInfo != null) return null

        return when (uri.scheme?.lowercase()) {
            "https" -> parseOfficialWebLink(uri)
            "lb" -> parseAppLink(uri)
            else -> null
        }
    }

    private fun parseOfficialWebLink(uri: URI): String? {
        if (uri.rawQuery != null || !uri.host.equals("app.lockbook.net", ignoreCase = true) || uri.port !in listOf(-1, 443)) return null
        val parts = uri.rawPath.split('/')
        if (parts.size != 3 || parts[0] != "" || parts[1] != "open") return null
        return canonicalFileId(parts[2])
    }

    private fun parseAppLink(uri: URI): String? {
        if (uri.port != -1 || (uri.rawPath.isNotEmpty() && uri.rawPath != "/") || uri.rawQuery != null) return null
        return canonicalFileId(uri.host ?: return null)
    }
}

object OpenLinkBuilder {
    internal fun canonicalOrigin(value: String): String? {
        val uri = runCatching { URI(value) }.getOrNull() ?: return null
        val scheme = uri.scheme?.lowercase()
        if (
            scheme !in listOf("http", "https") ||
            uri.host == null ||
            uri.rawUserInfo != null ||
            uri.rawQuery != null ||
            uri.fragment != null ||
            (uri.rawPath.isNotEmpty() && uri.rawPath != "/")
        ) {
            return null
        }
        val defaultPort = if (scheme == "https") 443 else 80
        val port = uri.port.takeUnless { it == -1 || it == defaultPort }
        val host = uri.host.lowercase()
        return buildString {
            append("$scheme://")
            append(host)
            if (port != null) append(":$port")
        }
    }

    fun build(
        apiUrl: String,
        fileId: String,
    ): String {
        val origin = requireNotNull(canonicalOrigin(apiUrl)) { "Account server must be an HTTP(S) origin" }
        val id = requireNotNull(canonicalFileId(fileId)) { "Invalid file id" }
        val originUri = origin.toUri()
        return Uri
            .Builder()
            .scheme(originUri.scheme)
            .encodedAuthority(originUri.encodedAuthority)
            .appendPath("open")
            .appendPath(id)
            .build()
            .toString()
    }
}

object PendingOpenLinkStore {
    private const val PREFERENCES = "pending_open_link"
    private const val FILE_ID = "file_id"

    fun save(
        context: Context,
        fileId: String,
    ) {
        context
            .getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)
            .edit {
                putString(FILE_ID, fileId)
            }
    }

    fun peek(context: Context): String? {
        val preferences = context.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)
        val id = preferences.getString(FILE_ID, null)
        if (id == null) return null
        return canonicalFileId(id)
    }

    fun acknowledge(
        context: Context,
        fileId: String,
    ) {
        // A second link may arrive while the first is syncing. Don't discard it.
        if (peek(context) == fileId) {
            context.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE).edit { clear() }
        }
    }
}
