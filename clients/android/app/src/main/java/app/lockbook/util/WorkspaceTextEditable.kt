package app.lockbook.util

import android.text.Editable
import android.text.InputFilter
import android.text.Selection
import android.text.Spannable
import android.text.Spanned
import android.view.KeyEvent
import app.lockbook.util.WorkspaceView.Companion.WGPU_OBJ
import app.lockbook.workspace.JTextRange
import app.lockbook.workspace.Workspace
import app.lockbook.workspace.Workspace.getSelection
import kotlinx.serialization.json.Json
import java.util.concurrent.atomic.AtomicReference

class WorkspaceTextEditable(val view: WorkspaceView, val wsInputConnection: WorkspaceTextInputConnection) : Editable {


    private var selectionStartSpanFlag = 0
    private var selectionEndSpanFlag = 0

    var composingStart = -1
    var composingEnd = -1

    private var composingFlag = 0
    private var composingTag: Any? = null

    val selectionStart: Int get() {
<<<<<<< Updated upstream
        val ret = getSelection(WGPU_OBJ).start
        return ret
    }

    val selectionEnd: Int get() {
        val ret = getSelection(WGPU_OBJ).end
        return ret
=======
        return ImePerfStats.measure("editable.selectionStart") { getSelection(WGPU_OBJ).start }
    }

    val selectionEnd: Int get() {
        return ImePerfStats.measure("editable.selectionEnd") { getSelection(WGPU_OBJ).end }
>>>>>>> Stashed changes
    }

    override fun toString(): String {
        return Workspace.getAllText(WGPU_OBJ)
    }

    fun getSelection(): JTextRange =
        ImePerfStats.measure("editable.getSelection") { getSelection(WorkspaceView.WGPU_OBJ) }

    override fun get(index: Int): Char {
        return ImePerfStats.measure("editable.getTextInRange.char") {
            Workspace.getTextInRange(WorkspaceView.WGPU_OBJ, index, index).getOrNull(0) ?: '0'
        }
    }

    override fun subSequence(startIndex: Int, endIndex: Int): CharSequence {
        return ImePerfStats.measure("editable.getTextInRange.subSequence") {
            Workspace.getTextInRange(WorkspaceView.WGPU_OBJ, startIndex, endIndex)
        }
    }

    override fun getChars(start: Int, end: Int, dest: CharArray?, destoff: Int) {
        dest?.let { realDest ->
            val text = ImePerfStats.measure("editable.getTextInRange.getChars") {
                Workspace.getTextInRange(WorkspaceView.WGPU_OBJ, start, end)
            }

            var index = destoff

            for (char in text) {
                if (index < realDest.size) {
                    dest[index] = char

                    index++
                } else {
                    break
                }
            }
        }
    }


    override fun <T> getSpans(start: Int, end: Int, type: Class<T>?): Array<T> {
        val spans: MutableList<Any> = mutableListOf()
        val spanRange = start..end

        if (type != null) {
            val instanceComposingTag = composingTag

            if (instanceComposingTag != null && type.isAssignableFrom(instanceComposingTag.javaClass) && (spanRange.contains(composingStart) || spanRange.contains(composingEnd))) {
                spans.add(instanceComposingTag)
            }

            if (type.isAssignableFrom(Selection.SELECTION_START.javaClass) && spanRange.contains(getSelection().start)) {
                spans.add(Selection.SELECTION_START)
            }

            if (type.isAssignableFrom(Selection.SELECTION_END.javaClass) && spanRange.contains(getSelection().end)) {
                spans.add(Selection.SELECTION_END)
            }
        }

        @Suppress("UNCHECKED_CAST")
        val returnSpans = java.lang.reflect.Array.newInstance(type, spans.size) as Array<T>

        for (i in spans.indices) {
            returnSpans[i] = spans[i] as T
        }

        return returnSpans
    }

    override fun getSpanStart(tag: Any?): Int {
        if (tag == Selection.SELECTION_START) {
            return selectionStart
        }

        if (tag == Selection.SELECTION_END) {
            return selectionEnd
        }

        if (tag == composingTag || ((tag ?: Unit)::class.simpleName ?: "").lowercase().contains("composing")) {
            return composingStart
        }

        return -1
    }

    override fun getSpanEnd(tag: Any?): Int {
        if (tag == Selection.SELECTION_START || tag == Selection.SELECTION_END) {
            TODO("not needed")
        }

        if (tag == composingTag || ((tag ?: Unit)::class.simpleName ?: "").lowercase().contains("composing")) {
            return composingEnd
        }

        return -1
    }

    override fun getSpanFlags(tag: Any?): Int {
        return when (tag) {
            Selection.SELECTION_START -> {
                selectionStartSpanFlag
            }
            Selection.SELECTION_END -> {
                selectionEndSpanFlag
            }
            else -> {
                if (tag == composingTag) {
                    return composingFlag
                }

                0
            }
        }
    }

    override fun nextSpanTransition(start: Int, limit: Int, type: Class<*>?): Int {
        return -1
    }

    override fun setSpan(what: Any?, start: Int, end: Int, flags: Int) {
        if (what == Selection.SELECTION_START) {
            ImePerfStats.count("editable.setSpan.selectionStart")
            selectionStartSpanFlag = flags
<<<<<<< Updated upstream
            Workspace.setSelection(WGPU_OBJ, start, end)
            view.drawImmediately()
=======
            Workspace.setSelection(WorkspaceView.WGPU_OBJ, start, end)
>>>>>>> Stashed changes
        } else if (what == Selection.SELECTION_END) {
            ImePerfStats.count("editable.setSpan.selectionEnd")
            selectionEndSpanFlag = flags
<<<<<<< Updated upstream
            Workspace.setSelection(WGPU_OBJ, start, end)
            view.drawImmediately()
=======
            Workspace.setSelection(WorkspaceView.WGPU_OBJ, start, end)
>>>>>>> Stashed changes
        } else if ((flags and Spanned.SPAN_COMPOSING) != 0) {
            ImePerfStats.count("editable.setSpan.composing")
            composingFlag = flags
            composingTag = what
            composingStart = start
            composingEnd = end
        } else {
            return
        }

        wsInputConnection.notifySelectionUpdated()
    }

    override fun removeSpan(what: Any?) {
        if (what == composingTag || ((what ?: Unit)::class.simpleName ?: "").lowercase().contains("composing")) {
            ImePerfStats.count("editable.removeSpan.composing")
            composingStart = -1
            composingEnd = -1

            wsInputConnection.notifySelectionUpdated()
        }
    }

    override fun append(text: CharSequence?): Editable {
        text?.let { realText ->
            ImePerfStats.count("editable.append")
            Workspace.append(WorkspaceView.WGPU_OBJ, realText.toString())

            view.drawImmediately()
        }

        return this
    }

    override fun append(text: CharSequence?, start: Int, end: Int): Editable {
<<<<<<< Updated upstream
        val appendText = text?.substring(start, end) ?: return this
        Workspace.append(WorkspaceView.WGPU_OBJ, appendText)
=======
        ImePerfStats.count("editable.append.slice")
        Workspace.append(WorkspaceView.WGPU_OBJ, text?.substring(start, end) ?: "null")
        view.kickTypingPump()
>>>>>>> Stashed changes
        view.drawImmediately()

        return this
    }

    override fun append(text: Char): Editable {
        ImePerfStats.count("editable.append.char")
        Workspace.append(WorkspaceView.WGPU_OBJ, text.toString())
        view.kickTypingPump()
        view.drawImmediately()
        wsInputConnection.notifySelectionUpdated()

        return this
    }

    override fun replace(st: Int, en: Int, source: CharSequence?, start: Int, end: Int): Editable {
        source?.let { realText ->
            ImePerfStats.count("editable.replace.slice")
            replace(st, en, realText.subSequence(start, end))
        }
        view.kickTypingPump()
        return this
    }

    private fun getComposingSpansFromSpannable(spannable: Spannable): Pair<Int, Int> {
        for (span in spannable.getSpans(0, spannable.length, Object::class.java)) {
            val flags = spannable.getSpanFlags(span)

            if ((flags and Spanned.SPAN_COMPOSING) != 0) {
                return Pair(spannable.getSpanStart(span), spannable.getSpanEnd(span))
            }
        }

        return Pair(-1, -1)
    }

    override fun replace(st: Int, en: Int, text: CharSequence?): Editable {
        text?.let { realText ->
            ImePerfStats.measure("editable.replace") {
                if (st == selectionStart && en == selectionEnd) {
                    if (realText == "\n") {
                        ImePerfStats.count("editable.replace.cursor.newline")
                        Workspace.sendKeyEvent(WorkspaceView.WGPU_OBJ, KeyEvent.KEYCODE_ENTER, "", true, false, false, false)
                    } else {
                        ImePerfStats.count("editable.replace.cursor.insertTextAtCursor")
                        Workspace.insertTextAtCursor(WorkspaceView.WGPU_OBJ, realText.toString())
                    }
                } else {
                    ImePerfStats.count("editable.replace.region")
                    Workspace.replace(WorkspaceView.WGPU_OBJ, st, en, realText.toString())
                }
            }

            if (en < composingStart) {
                val replacedLen = en - st

                composingStart = composingStart - replacedLen + realText.length
                composingEnd = composingEnd - replacedLen + realText.length
            }

            val spannableSource = realText as? Spannable
            if (spannableSource != null) {
                val (sourceComposingStart, sourceComposingEnd) = if (composingTag == null) {
                    getComposingSpansFromSpannable(spannableSource)
                } else {
                    Pair(spannableSource.getSpanStart(composingTag), spannableSource.getSpanEnd(composingTag))
                }

                if (sourceComposingStart != -1) {
                    val newStart = st + sourceComposingStart

                    if (composingStart == -1 || composingStart > newStart) {
                        composingStart = newStart
                    }
                }

                if (sourceComposingEnd != -1) {
                    val newEnd = st + sourceComposingEnd

                    if (composingEnd < newEnd) {
                        composingEnd = newEnd
                    }
                }
            }

            view.kickTypingPump()
            view.drawImmediately()
            wsInputConnection.notifySelectionUpdated()
        }

        return this
    }

    override fun insert(where: Int, text: CharSequence?, start: Int, end: Int): Editable {
        text?.let { realText ->
            val subRealText = realText.substring(start, end)

            ImePerfStats.measure("editable.insert.slice") {
                if (subRealText == "\n" && selectionEnd == where && selectionStart == where) {
                    ImePerfStats.count("editable.insert.slice.newline")
                    Workspace.sendKeyEvent(WorkspaceView.WGPU_OBJ, KeyEvent.KEYCODE_ENTER, "", true, false, false, false)
                } else {
                    ImePerfStats.count("editable.insert.slice.region")
                    Workspace.insert(WorkspaceView.WGPU_OBJ, where, subRealText)
                }
            }

            if (where < composingStart) {
                composingStart += subRealText.length
                composingEnd += subRealText.length
            }

            view.kickTypingPump()
            view.drawImmediately()
            wsInputConnection.notifySelectionUpdated()
        }

        return this
    }

    override fun insert(where: Int, text: CharSequence?): Editable {
        text?.let { realText ->
            ImePerfStats.measure("editable.insert") {
                if (realText == "\n" && selectionEnd == where && selectionStart == where) {
                    ImePerfStats.count("editable.insert.newline")
                    Workspace.sendKeyEvent(WorkspaceView.WGPU_OBJ, KeyEvent.KEYCODE_ENTER, "", true, false, false, false)
                } else {
                    ImePerfStats.count("editable.insert.region")
                    Workspace.insert(WorkspaceView.WGPU_OBJ, where, realText.toString())
                }
            }

            if (where < composingStart) {
                composingStart += realText.length
                composingEnd += realText.length
            }

            view.kickTypingPump()
            view.drawImmediately()
            wsInputConnection.notifySelectionUpdated()
        }

        return this
    }

    override fun delete(st: Int, en: Int): Editable {
        ImePerfStats.measure("editable.delete") {
            Workspace.replace(WorkspaceView.WGPU_OBJ, st, en, "")
        }

        if (en < composingStart) {
            composingStart -= (en - st)
            composingEnd -= (en - st)
        }

        view.kickTypingPump()
        view.drawImmediately()
        wsInputConnection.notifySelectionUpdated()

        return this
    }

    override fun clear() {
        ImePerfStats.count("editable.clear")
        Workspace.clear(WorkspaceView.WGPU_OBJ)

        composingStart = -1
        composingEnd = -1

        view.kickTypingPump()
        view.drawImmediately()
        wsInputConnection.notifySelectionUpdated()
    }

    override fun clearSpans() {
        if (composingStart != -1 || composingEnd != -1) {
            composingStart = -1
            composingEnd = -1

            wsInputConnection.notifySelectionUpdated()
        }
    }
    override fun setFilters(filters: Array<out InputFilter>?) {}

    override fun getFilters(): Array<InputFilter> = arrayOf()
    override val length: Int get() {
        return ImePerfStats.measure("editable.length") {
            Workspace.getTextLength(WorkspaceView.WGPU_OBJ)
        }
    }
}
