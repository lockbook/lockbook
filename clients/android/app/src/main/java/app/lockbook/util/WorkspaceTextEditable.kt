package app.lockbook.util

import android.text.Editable
import android.text.InputFilter
import android.text.Selection
import android.text.Spannable
import android.text.Spanned
import android.view.KeyEvent
import app.lockbook.workspace.JTextRange
import app.lockbook.workspace.Workspace
import java.util.concurrent.atomic.AtomicReference
import kotlin.text.iterator

class WorkspaceTextEditable(val view: WorkspaceView, val wsInputConnection: WorkspaceTextInputConnection) : Editable {

    private var selectionStartSpanFlag = 0
    private var selectionEndSpanFlag = 0

    var composingStart = -1
    var composingEnd = -1

    private var composingFlag = 0
    private var composingTag: Any? = null

    var localQueue: ArrayDeque<WorkspaceView.WsTextMutation> = ArrayDeque()

    val queueId : Int get(){
        return view.frameCount.get()
    }

    val selectionStart: Int get() {
        return getSelection().start
    }

    val selectionEnd: Int get() {
        return getSelection().end
    }

    override fun toString(): String {
        return Workspace.getAllText(WorkspaceView.WGPU_OBJ)
    }

    fun getSelection(): JTextRange = view.pendingSelection.get()

    override fun get(index: Int): Char {
        println("WorkspaceTextEditable: get ${index}")
        return getTextInRange(index, index).getOrNull(0) ?: '0'
//        view.nativeLock.withLock {
//            return Workspace.getTextInRange(WorkspaceView.WGPU_OBJ, index, index).getOrNull(0) ?: '0'
//        }
    }

    override fun subSequence(startIndex: Int, endIndex: Int): CharSequence {
        println("WorkspaceTextEditable: GRANGE ${startIndex}, ${endIndex}")
//        view.nativeLock.withLock {
//            return Workspace.getTextInRange(WorkspaceView.WGPU_OBJ, startIndex, endIndex)
//        }
        return getTextInRange(startIndex, endIndex)
    }

    override fun getChars(start: Int, end: Int, dest: CharArray?, destoff: Int) {
        println("WorkspaceTextEditable: getChars ${start}, ${end}, ${dest}, $destoff")

        dest?.let { realDest ->
                val text = getTextInRange(start, end)

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
        println("WorkspaceTextEditable: get spand ${start}, ${end}")

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
//        println("WorkspaceTextEditable: getSpanStart ${tag}")

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
//        println("WorkspaceTextEditable: getSpanEnd ${tag}")

        if (tag == Selection.SELECTION_START || tag == Selection.SELECTION_END) {
            TODO("not needed")
        }

        if (tag == composingTag || ((tag ?: Unit)::class.simpleName ?: "").lowercase().contains("composing")) {
            return composingEnd
        }

        return -1
    }

    override fun getSpanFlags(tag: Any?): Int {
//        println("WorkspaceTextEditable: getSpanFlags ${tag}")

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
        println("WorkspaceTextEditable: setSpan ${start}, ${end}, ${what}")

        if (what == Selection.SELECTION_START) {
            selectionStartSpanFlag = flags
            view.textMutations.get().add(WorkspaceView.WsTextMutation.SetSelection(start, end) to -1)
            view.drawImmediately()
        } else if (what == Selection.SELECTION_END) {
            selectionEndSpanFlag = flags
            view.textMutations.get().add(WorkspaceView.WsTextMutation.SetSelection(start, end)to -1)
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
        println("WorkspaceTextEditable: removeSpan  ${what}")

        if (what == composingTag || ((what ?: Unit)::class.simpleName ?: "").lowercase().contains("composing")) {
            composingStart = -1
            composingEnd = -1

            wsInputConnection.notifySelectionUpdated()
        }
    }

    override fun append(text: CharSequence?): Editable {
        println("WorkspaceTextEditable: append ${text}")

        text?.let { realText ->
            localQueue.add(WorkspaceView.WsTextMutation.Append(realText.toString()))
            view.drawImmediately()
        }

        return this
    }

    override fun append(text: CharSequence?, start: Int, end: Int): Editable {
        println("WorkspaceTextEditable: append ${start} ${end}")

        localQueue.add(WorkspaceView.WsTextMutation.Append(text?.substring(start, end) ?: ""))

        view.drawImmediately()

        return this
    }

    override fun append(text: Char): Editable {
        println("WorkspaceTextEditable: append ${text}")

        localQueue.add(WorkspaceView.WsTextMutation.Append(text.toString()))
        view.drawImmediately()
        wsInputConnection.notifySelectionUpdated(true)

        return this
    }

    override fun replace(st: Int, en: Int, source: CharSequence?, start: Int, end: Int): Editable {
        println("WorkspaceTextEditable: REPL $st $en $source $start $end")
        source?.let { realText ->
            replace(st, en, realText.subSequence(start, end))
        }

        return this
    }

    private fun getComposingSpansFromSpannable(spannable: Spannable): Pair<Int, Int> {
        println("WorkspaceTextEditable: getComposingSpansFromSpannable $spannable")

        for (span in spannable.getSpans(0, spannable.length, Object::class.java)) {
            val flags = spannable.getSpanFlags(span)

            if ((flags and Spanned.SPAN_COMPOSING) != 0) {
                return Pair(spannable.getSpanStart(span), spannable.getSpanEnd(span))
            }
        }

        return Pair(-1, -1)
    }

    override fun replace(st: Int, en: Int, text: CharSequence?): Editable {
        println("WorkspaceTextEditable: REPL $st $en $text | ${wsInputConnection.batchEditCount}")
        text?.let { realText ->

            val pendingCommand = WorkspaceView.WsTextMutation.Replace(st, en, realText.toString(), wsInputConnection.batchEditCount.get())

            localQueue.add(pendingCommand)
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
            wsInputConnection.notifySelectionUpdated(true)
        }

        return this
    }

    override fun insert(where: Int, text: CharSequence?, start: Int, end: Int): Editable {
        println("WorkspaceTextEditable: insert $where $start $end $text")

        text?.let { realText ->
            val subRealText = realText.substring(start, end)

            if (subRealText == "\n" && selectionEnd == where && selectionStart == where) {
                localQueue.add(WorkspaceView.WsTextMutation.SendKeyEvent(KeyEvent.KEYCODE_ENTER,
                    "",
                    true,
                    false,
                    false,
                    false)
                )
            } else {
                localQueue.add(WorkspaceView.WsTextMutation.Insert(where, subRealText))
            }
            if (where < composingStart) {
                composingStart += subRealText.length
                composingEnd += subRealText.length
            }

            view.drawImmediately()
            wsInputConnection.notifySelectionUpdated(true)
        }

        return this
    }

    override fun insert(where: Int, text: CharSequence?): Editable {
        println("WorkspaceTextEditable: insert $text")

        text?.let { realText ->

            if (realText == "\n" && selectionEnd == where && selectionStart == where) {
                localQueue.add(WorkspaceView.WsTextMutation.SendKeyEvent(KeyEvent.KEYCODE_ENTER,
                    "",
                    true,
                    false,
                    false,
                    false)
                )
            } else {
                localQueue.add(WorkspaceView.WsTextMutation.Insert(where, realText.toString()))
            }

            if (where < composingStart) {
                composingStart += realText.length
                composingEnd += realText.length
            }

            view.drawImmediately()
            wsInputConnection.notifySelectionUpdated(true)
        }

        return this
    }

    override fun delete(st: Int, en: Int): Editable {
        println("WorkspaceTextEditable: DEL $st $en | ${wsInputConnection.batchEditCount}")

        val pendingCommand = WorkspaceView.WsTextMutation.Replace(st, en, "", wsInputConnection.batchEditCount.get())
        localQueue.add(pendingCommand)

        if (en < composingStart) {
            composingStart -= (en - st)
            composingEnd -= (en - st)
        }

        view.drawImmediately()
        wsInputConnection.notifySelectionUpdated(true)

        return this
    }

    override fun clear() {
        println("WorkspaceTextEditable: clear ")

        localQueue.add(WorkspaceView.WsTextMutation.Clear)

        composingStart = -1
        composingEnd = -1

        view.drawImmediately()
        wsInputConnection.notifySelectionUpdated(true)
    }

    override fun clearSpans() {
        println("WorkspaceTextEditable: clearSpans ")

        if (composingStart != -1 || composingEnd != -1) {
            composingStart = -1
            composingEnd = -1

            wsInputConnection.notifySelectionUpdated()
        }
    }
    override fun setFilters(filters: Array<out InputFilter>?) {}

    override fun getFilters(): Array<InputFilter> = arrayOf()

    override val length: Int get() {
        println("WorkspaceTextEditable: LEN ${view.pendingTextLength.get()}")
        return view.pendingTextLength.get()
    }

    private fun getTextInRange(st: Int, en: Int): String {
        return view.pendingBuffer.get().slice(st..<en)
    }

    fun flushQueue(){
        while (localQueue.isNotEmpty()) {
            val mutation = localQueue.removeFirst()
            view.textMutations.get().add(mutation to queueId)
        }
    }
}
