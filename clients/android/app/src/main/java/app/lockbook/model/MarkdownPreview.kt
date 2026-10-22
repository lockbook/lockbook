package app.lockbook.model

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.intOrNull
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

private val previewFence = Regex("^ {0,3}(`{3,}|~{3,})(.*)$")

internal data class MarkdownExcerpt(
    val markdown: String,
    val focusLine: String,
)

/** Reads the same per-document selection schema used by WsPersistentStore. Values are grapheme offsets. */
internal fun savedMarkdownCursors(content: String): Map<String, Int> {
    val files =
        Json
            .parseToJsonElement(content)
            .jsonObject["markdown"]
            ?.jsonObject
            ?.get("file")
            ?.jsonObject ?: return emptyMap()
    return files
        .mapNotNull { (id, value) ->
            runCatching {
                val cursor =
                    value.jsonObject["selection"]
                        ?.jsonArray
                        ?.lastOrNull()
                        ?.jsonPrimitive
                        ?.intOrNull
                cursor?.let { id to it.coerceAtLeast(0) }
            }.getOrNull()
        }.toMap()
}

/** A bounded source excerpt, retaining nearby lines and fenced-code context for Markdown rendering. */
internal fun markdownExcerpt(
    markdown: String,
    cursorUtf16: Int,
): MarkdownExcerpt {
    if (markdown.isBlank()) return MarkdownExcerpt("", "")
    val cursor = cursorUtf16.coerceIn(0, markdown.length)
    val lines = markdown.split('\n')
    val offsets = IntArray(lines.size)
    for (index in 1 until lines.size) offsets[index] = offsets[index - 1] + lines[index - 1].length + 1
    val current = offsets.binarySearch(cursor).let { if (it >= 0) it else (-it - 2).coerceAtLeast(0) }
    var first = (current - 2).coerceAtLeast(0)
    val last = (first + 10).coerceAtMost(lines.size)
    if (last == lines.size) first = (last - 10).coerceAtLeast(0)
    var start = offsets[first]
    var end = if (last < lines.size) offsets[last] else markdown.length
    if (end - start > 1400) {
        start = maxOf(start, cursor - 350)
        end = minOf(end, start + 1400)
    }

    fun safeBoundary(index: Int): Int =
        if (index in 1 until markdown.length && markdown[index].isLowSurrogate() &&
            markdown[index - 1].isHighSurrogate()
        ) {
            index - 1
        } else {
            index
        }
    start = safeBoundary(start)
    end = safeBoundary(end)
    val firstLine = offsets.indexOfLast { it <= start }
    var fence: String? = null

    fun scanFence(line: String) {
        val candidate = previewFence.find(line.removeSuffix("\r")) ?: return
        val marker = candidate.groupValues[1]
        val open = fence
        if (open == null) {
            fence = candidate.value.trimStart()
        } else {
            val openMarker = open.takeWhile { it == open.first() }
            if (marker.first() == openMarker.first() && marker.length >= openMarker.length &&
                candidate.groupValues[2].isBlank()
            ) {
                fence = null
            }
        }
    }
    lines.asSequence().take(firstLine).forEach(::scanFence)
    val opening = fence
    val source = markdown.substring(start, end).trim('\n', '\r')
    source.split('\n').forEach(::scanFence)
    val preview =
        buildString {
            if (opening != null) append(opening).append('\n')
            append(source)
            fence?.let { append('\n').append(it.takeWhile { char -> char == it.first() }) }
        }
    val focusLine =
        if (lines[current].isNotBlank()) {
            current
        } else {
            (current downTo firstLine).firstOrNull { lines[it].isNotBlank() } ?: current
        }
    val focusStart = maxOf(start, offsets[focusLine])
    val focusEnd = minOf(end, offsets[focusLine] + lines[focusLine].length)
    val focus = if (focusEnd > focusStart) markdown.substring(focusStart, focusEnd).trim() else ""
    return MarkdownExcerpt(preview, focus)
}
