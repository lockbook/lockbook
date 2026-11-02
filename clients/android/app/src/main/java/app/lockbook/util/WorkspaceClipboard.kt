package app.lockbook.util

import android.content.ClipboardManager
import android.content.Context
import android.widget.Toast
import app.lockbook.workspace.Workspace
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/** Clipboard import shared by text input and the canvas's native paste menu. */
object WorkspaceClipboard {
    fun paste(
        view: WorkspaceView,
        canvas: Boolean = false,
    ): Boolean {
        if (!view.canForwardTouches()) return false
        val handle = WorkspaceView.wgpuObj

        fun cancelCanvas() {
            if (canvas && view.canForwardTouches()) {
                Workspace.canvasSendContent(handle, byteArrayOf(), false)
                view.invalidate()
            }
        }
        val clipboard = view.context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        val clip =
            clipboard.primaryClip ?: run {
                cancelCanvas()
                return false
            }
        if (clip.itemCount == 0) {
            cancelCanvas()
            return false
        }
        val item = clip.getItemAt(0)

        fun targetIsCurrent(): Boolean = view.isAttachedToWindow && view.canForwardTouches() && WorkspaceView.wgpuObj == handle

        fun send(
            bytes: ByteArray,
            isSvg: Boolean,
        ) {
            if (!targetIsCurrent()) return
            if (canvas) {
                Workspace.canvasSendContent(handle, bytes, isSvg)
            } else if (isSvg) {
                Workspace.clipboardPaste(handle, bytes.toString(Charsets.UTF_8))
            } else {
                Workspace.clipboardSendImage(handle, bytes, true)
            }
            view.drawImmediately()
        }

        val uri = item.uri
        val mime = uri?.let { runCatching { view.context.contentResolver.getType(it) }.getOrNull() }
        val imageUri =
            uri != null &&
                (mime?.startsWith("image/") == true || clip.description.hasMimeType("image/*"))
        if (imageUri) {
            view.launchIo {
                try {
                    val bytes = readAttachmentBytes(view.context, uri!!) ?: error("Could not read clipboard content")
                    val svg =
                        canvas &&
                            (mime == "image/svg+xml" || isSvgClipboardText(bytes.toString(Charsets.UTF_8)))
                    withContext(Dispatchers.Main) { send(bytes, svg) }
                } catch (error: CancellationException) {
                    throw error
                } catch (error: Exception) {
                    withContext(Dispatchers.Main) {
                        if (targetIsCurrent()) {
                            cancelCanvas()
                            val message =
                                if (error is AttachmentTooLargeException) {
                                    "Files larger than 25 MiB cannot be imported"
                                } else {
                                    "Could not read clipboard content"
                                }
                            Toast.makeText(view.context, message, Toast.LENGTH_SHORT).show()
                        }
                    }
                }
            }
            return true
        }

        val text = item.text?.toString()
        if (text != null && (!canvas || isSvgClipboardText(text))) {
            if (!canvas) {
                Workspace.clipboardPaste(handle, text)
                view.drawImmediately()
            } else {
                send(text.toByteArray(Charsets.UTF_8), true)
            }
            return true
        }
        cancelCanvas()
        Toast.makeText(view.context, "Clipboard content not supported", Toast.LENGTH_SHORT).show()
        return false
    }
}

/** Recognize the SVG root while allowing the XML preamble used by vector editors. */
internal fun isSvgClipboardText(text: String): Boolean {
    var remaining = text.trimStart('\uFEFF', ' ', '\t', '\r', '\n')
    while (true) {
        remaining =
            when {
                remaining.startsWith("<?xml") -> {
                    val end = remaining.indexOf("?>")
                    if (end < 0) return false
                    remaining.substring(end + 2).trimStart()
                }

                remaining.startsWith("<!--") -> {
                    val end = remaining.indexOf("-->")
                    if (end < 0) return false
                    remaining.substring(end + 3).trimStart()
                }

                remaining.startsWith("<!DOCTYPE svg") -> {
                    val end = remaining.indexOf('>')
                    if (end < 0) return false
                    remaining.substring(end + 1).trimStart()
                }

                else -> {
                    return remaining.startsWith("<svg") &&
                        remaining.getOrNull(4)?.let { it.isWhitespace() || it == '>' || it == '/' } == true
                }
            }
    }
}
