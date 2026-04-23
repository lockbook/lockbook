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

class WorkspaceTextEditable(val view: WorkspaceView, val wsInputConnection: WorkspaceTextInputConnection) : Editable {

    private val logger = AppLogger.getLogger("InputBridging")

    private var selectionStartSpanFlag = 0
    private var selectionEndSpanFlag = 0

    var composingStart = -1
    var composingEnd = -1

    private var composingFlag = 0
    private var composingTag: Any? = null

    var localQueue: ArrayDeque<WorkspaceView.WsTextMutation> = ArrayDeque()


    val queueId: Int get() {
        return view.pendingWorkspaceTextState.get().frameCount
    }

    var selectionStart = AtomicReference(0)

    var selectionEnd = AtomicReference(0)

    override fun toString(): String {
        return Workspace.getAllText(WorkspaceView.WGPU_OBJ)
    }


    override fun get(index: Int): Char {
        logger.d("GET ${index}")

        return getTextInRange(index, index).getOrNull(0) ?: '0'
    }

    override fun subSequence(startIndex: Int, endIndex: Int): CharSequence {
        logger.d("GET SUB SEQUENCE ${startIndex} ${endIndex}")

        return getTextInRange(startIndex, endIndex)
    }

    override fun getChars(start: Int, end: Int, dest: CharArray?, destoff: Int) {
        logger.d("GET CHARS ${start} ${end}")
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
        logger.d("GET SPANS ${start} ${end} ${type}")

        val spans: MutableList<Any> = mutableListOf()
        val spanRange = start..end

        if (type != null) {
            val instanceComposingTag = composingTag

            if (instanceComposingTag != null && type.isAssignableFrom(instanceComposingTag.javaClass) && (spanRange.contains(composingStart) || spanRange.contains(composingEnd))) {
                spans.add(instanceComposingTag)
            }

            if (type.isAssignableFrom(Selection.SELECTION_START.javaClass) && spanRange.contains(selectionStart.get())) {
                spans.add(Selection.SELECTION_START)
            }

            if (type.isAssignableFrom(Selection.SELECTION_END.javaClass) && spanRange.contains(selectionEnd.get())) {
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
            logger.d("GET SPAN START a: ${selectionStart}")

            return selectionStart.get()
        }

        if (tag == Selection.SELECTION_END) {
            logger.d("GET SPAN START b: ${selectionEnd}")
            return selectionEnd.get()
        }

        if (tag == composingTag || ((tag ?: Unit)::class.simpleName ?: "").lowercase().contains("composing")) {
            logger.d("GET SPAN START c: ${composingStart}")
            return composingStart
        }

        return -1
    }

    override fun getSpanEnd(tag: Any?): Int {

        if (tag == Selection.SELECTION_START || tag == Selection.SELECTION_END) {
            logger.d("GET SPAN END a: ${composingEnd}")

            TODO("not needed")
        }

        if (tag == composingTag || ((tag ?: Unit)::class.simpleName ?: "").lowercase().contains("composing")) {
            logger.d("GET SPAN END b: ${composingEnd}")
            return composingEnd
        }

        return -1
    }

    override fun getSpanFlags(tag: Any?): Int {

        return when (tag) {
            Selection.SELECTION_START -> {
                logger.d("GET SPAN FLAGS START")
                selectionStartSpanFlag
            }
            Selection.SELECTION_END -> {
                logger.d("GET SPAN FLAGS END")
                selectionEndSpanFlag
            }
            else -> {
                if (tag == composingTag) {
                    logger.d("GET SPAN FLAGS COMPOSING")
                    return composingFlag
                }
                logger.d("GET SPAN FLAGS UNKNOWN")

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
            logger.d("SET SPAN a ${what} ${start} ${end} ${flags}")
            wsInputConnection.pushTextMutationEvent(WorkspaceView.WsTextMutation.SetSelection(start, end) )
            selectionStart.set(start)
            view.drawImmediately()
        } else if (what == Selection.SELECTION_END) {
            selectionEndSpanFlag = flags
            logger.d("SET SPAN b ${what} ${start} ${end} ${flags}")
            wsInputConnection.pushTextMutationEvent(WorkspaceView.WsTextMutation.SetSelection(start, end))
            selectionEnd.set(end)
            view.drawImmediately()
        } else if ((flags and Spanned.SPAN_COMPOSING) != 0) {
            logger.d("SET SPAN c ${what} ${start} ${end} ${flags}")
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

        if (what === Selection.SELECTION_START || what === Selection.SELECTION_END) {
            logger.d("DEL SPAN SELECTION ${what}")
            return
        }
        if (what == composingTag || ((what ?: Unit)::class.simpleName ?: "").lowercase().contains("composing")) {
            logger.d("DEL SPAN COMPOSING ${what}")
            composingStart = -1
            composingEnd = -1

            wsInputConnection.notifySelectionUpdated()
        }
    }

    override fun append(text: CharSequence?): Editable {
        logger.d("APPEND ${text}")

        text?.let { realText ->
            wsInputConnection.pushTextMutationEvent(WorkspaceView.WsTextMutation.Append(realText.toString()) )
            view.drawImmediately()
        }

        return this
    }

    override fun append(text: CharSequence?, start: Int, end: Int): Editable {

        logger.d("APPEND ${start} ${end} ${text} ")
        
        wsInputConnection.pushTextMutationEvent(WorkspaceView.WsTextMutation.Append(text?.substring(start, end) ?: ""))

        view.drawImmediately()

        return this
    }

    override fun append(text: Char): Editable {
        logger.d("APPEND ${text} ")

        wsInputConnection.pushTextMutationEvent(WorkspaceView.WsTextMutation.Append(text.toString()) )
        view.drawImmediately()
        wsInputConnection.notifySelectionUpdated(true)

        return this
    }

    override fun replace(st: Int, en: Int, source: CharSequence?, start: Int, end: Int): Editable {
        logger.d("REPLACE ${st} ${end} $source, $start, $end")

        source?.let { realText ->
            replace(st, en, realText.subSequence(start, end))
        }

        return this
    }

    private fun getComposingSpansFromSpannable(spannable: Spannable): Pair<Int, Int> {
        logger.d("GET COMPOSING SPANS $spannable")

        for (span in spannable.getSpans(0, spannable.length, Object::class.java)) {
            val flags = spannable.getSpanFlags(span)

            if ((flags and Spanned.SPAN_COMPOSING) != 0) {
                return Pair(spannable.getSpanStart(span), spannable.getSpanEnd(span))
            }
        }

        return Pair(-1, -1)
    }

    override fun replace(st: Int, en: Int, text: CharSequence?): Editable {
        logger.d("REPLACE $st $en $text")

        text?.let { realText ->
            // 1. Dispatch the mutation to the Rust core
            val pendingCommand = WorkspaceView.WsTextMutation.Replace(
                st.coerceAtMost(length), en.coerceAtMost(length), realText.toString(), wsInputConnection.batchEditCount.get()
            )
            wsInputConnection.pushTextMutationEvent(pendingCommand)

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

            // Clamp the values to prevent Rust panics
            if (composingStart != -1) {
                composingStart = composingStart.coerceIn(0, this.length)
                composingEnd = composingEnd.coerceIn(0, this.length)

                if (composingStart > composingEnd) {
                    composingStart = -1
                    composingEnd = -1
                }
            }

            view.drawImmediately()
            wsInputConnection.notifySelectionUpdated(true)
        }

        return this
    }

    override fun insert(where: Int, text: CharSequence?, start: Int, end: Int): Editable {
        logger.d("INSERT $where $text $start $end")

        text?.let { realText ->
            val subRealText = realText.substring(start, end)

            if (subRealText == "\n" && selectionEnd.get() == where && selectionStart.get() == where) {
                wsInputConnection.pushTextMutationEvent(
                    WorkspaceView.WsTextMutation.SendKeyEvent(
                        KeyEvent.KEYCODE_ENTER,
                        "",
                        true,
                        false,
                        false,
                        false
                    ) 
                )
            } else {
                wsInputConnection.pushTextMutationEvent(WorkspaceView.WsTextMutation.Insert(where, subRealText) )
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
        logger.d("INSERT $where $text")

        text?.let { realText ->

            if (realText == "\n" && selectionEnd.get() == where && selectionStart.get() == where) {
                wsInputConnection.pushTextMutationEvent(
                    WorkspaceView.WsTextMutation.SendKeyEvent(
                        KeyEvent.KEYCODE_ENTER,
                        "",
                        true,
                        false,
                        false,
                        false
                    ) 
                )
            } else {
                wsInputConnection.pushTextMutationEvent(WorkspaceView.WsTextMutation.Insert(where, realText.toString()) )
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
        logger.d("REPL $st $en <DELETE>")

        val pendingCommand = WorkspaceView.WsTextMutation.Replace(st, en, "", wsInputConnection.batchEditCount.get())
        wsInputConnection.pushTextMutationEvent(pendingCommand )

        if (en < composingStart) {
            composingStart -= (en - st)
            composingEnd -= (en - st)
        }

        view.drawImmediately()
        wsInputConnection.notifySelectionUpdated(true)

        return this
    }

    override fun clear() {
        logger.d("CLEAR")

        wsInputConnection.pushTextMutationEvent(WorkspaceView.WsTextMutation.Clear )

        composingStart = -1
        composingEnd = -1

        view.drawImmediately()
        wsInputConnection.notifySelectionUpdated(true)
    }

    override fun clearSpans() {
        logger.d("CLEAR SPANS")

        if (composingStart != -1 || composingEnd != -1) {
            composingStart = -1
            composingEnd = -1

            wsInputConnection.notifySelectionUpdated()
        }
    }
    override fun setFilters(filters: Array<out InputFilter>?) {}

    override fun getFilters(): Array<InputFilter> = arrayOf()

    override val length: Int get() {
        logger.d("GET LENGTH ${view.pendingWorkspaceTextState.get().textLength}")

        return view.pendingWorkspaceTextState.get().textLength
    }

    private fun getTextInRange(st: Int, en: Int): String {
        return view.pendingWorkspaceTextState.get().buffer.slice(st.coerceAtMost(length)..<en.coerceAtMost(length))
    }

    fun flushQueue() {
        while (localQueue.isNotEmpty()) {
            val mutation = localQueue.removeFirst()
            wsInputConnection.pushTextMutationEvent(mutation )
        }
    }
}
