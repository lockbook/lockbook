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
        return getSelection(WGPU_OBJ).start
    }

    val selectionEnd: Int get() {
        return getSelection(WGPU_OBJ).end
    }

    override fun toString(): String {
        return Workspace.getAllText(WGPU_OBJ)
    }

    fun getSelection(): JTextRange = getSelection(WorkspaceView.WGPU_OBJ)

    override fun get(index: Int): Char {
        return Workspace.getTextInRange(WorkspaceView.WGPU_OBJ, index, index).getOrNull(0) ?: '0'
    }

    override fun subSequence(startIndex: Int, endIndex: Int): CharSequence {
        return Workspace.getTextInRange(WorkspaceView.WGPU_OBJ, startIndex, endIndex)
    }

    override fun getChars(start: Int, end: Int, dest: CharArray?, destoff: Int) {
        dest?.let { realDest ->
            val text = Workspace.getTextInRange(WorkspaceView.WGPU_OBJ, start, end)

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
            selectionStartSpanFlag = flags
            Workspace.setSelection(WorkspaceView.WGPU_OBJ, start, end)
            view.drawImmediately()
        } else if (what == Selection.SELECTION_END) {
            selectionEndSpanFlag = flags
            Workspace.setSelection(WorkspaceView.WGPU_OBJ, start, end)
            view.drawImmediately()
        } else if ((flags and Spanned.SPAN_COMPOSING) != 0) {
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
            composingStart = -1
            composingEnd = -1

            wsInputConnection.notifySelectionUpdated()
        }
    }

    override fun append(text: CharSequence?): Editable {
        text?.let { realText ->
            Workspace.append(WorkspaceView.WGPU_OBJ, realText.toString())

            view.drawImmediately()
        }

        return this
    }

    override fun append(text: CharSequence?, start: Int, end: Int): Editable {
        Workspace.append(WorkspaceView.WGPU_OBJ, text?.substring(start, end) ?: "null")
        view.drawImmediately()

        return this
    }

    override fun append(text: Char): Editable {
        Workspace.append(WorkspaceView.WGPU_OBJ, text.toString())
        view.drawImmediately()
        wsInputConnection.notifySelectionUpdated()

        return this
    }

    override fun replace(st: Int, en: Int, source: CharSequence?, start: Int, end: Int): Editable {
        source?.let { realText ->
            replace(st, en, realText.subSequence(start, end))
        }
        view.drawImmediately()
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
        println("ad-tra: replacing $st $en where selection is $selectionStart $selectionEnd")
        text?.let { realText ->
            if (st == selectionStart && en == selectionEnd) {
                if (realText == "\n") {
                    Workspace.sendKeyEvent(WorkspaceView.WGPU_OBJ, KeyEvent.KEYCODE_ENTER, "", true, false, false, false)
                } else {
                    Workspace.insertTextAtCursor(WorkspaceView.WGPU_OBJ, realText.toString())
                }
            } else {
                Workspace.replace(WorkspaceView.WGPU_OBJ, st, en, realText.toString())
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

            view.drawImmediately()
            wsInputConnection.notifySelectionUpdated()
        }

        return this
    }

    override fun insert(where: Int, text: CharSequence?, start: Int, end: Int): Editable {
        text?.let { realText ->
            val subRealText = realText.substring(start, end)

            if (subRealText == "\n" && selectionEnd == where && selectionStart == where) {
                Workspace.sendKeyEvent(WorkspaceView.WGPU_OBJ, KeyEvent.KEYCODE_ENTER, "", true, false, false, false)
            } else {
                Workspace.insert(WorkspaceView.WGPU_OBJ, where, subRealText)
            }

            if (where < composingStart) {
                composingStart += subRealText.length
                composingEnd += subRealText.length
            }

            view.drawImmediately()
            wsInputConnection.notifySelectionUpdated()
        }

        return this
    }

    override fun insert(where: Int, text: CharSequence?): Editable {
        text?.let { realText ->
            if (realText == "\n" && selectionEnd == where && selectionStart == where) {
                Workspace.sendKeyEvent(WorkspaceView.WGPU_OBJ, KeyEvent.KEYCODE_ENTER, "", true, false, false, false)
            } else {
                Workspace.insert(WorkspaceView.WGPU_OBJ, where, realText.toString())
            }

            if (where < composingStart) {
                composingStart += realText.length
                composingEnd += realText.length
            }

            view.drawImmediately()
            wsInputConnection.notifySelectionUpdated()
        }

        return this
    }

    override fun delete(st: Int, en: Int): Editable {
        Workspace.replace(WorkspaceView.WGPU_OBJ, st, en, "")

        if (en < composingStart) {
            composingStart -= (en - st)
            composingEnd -= (en - st)
        }

        view.drawImmediately()
        wsInputConnection.notifySelectionUpdated()

        return this
    }

    override fun clear() {
        Workspace.clear(WorkspaceView.WGPU_OBJ)

        composingStart = -1
        composingEnd = -1

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
        return Workspace.getTextLength(WorkspaceView.WGPU_OBJ)
    }
}
