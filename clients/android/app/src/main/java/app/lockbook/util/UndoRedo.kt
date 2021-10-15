package app.lockbook.util

import android.text.Editable
import android.text.TextWatcher
import android.text.style.UnderlineSpan
import android.widget.EditText
import app.lockbook.model.TextEditorViewModel
import timber.log.Timber
import java.util.*

class EditTextModel(private val editText: EditText, private val editorViewModel: TextEditorViewModel, private val toggleUndoButton: (Boolean) -> Unit, private val toggleRedoButton: (Boolean) -> Unit) {
    private val editHistory: EditHistory
    private val changeListener: EditTextChangeListener

    private var isUndoRedo = false

    init {
        editHistory = EditHistory()
        changeListener = EditTextChangeListener()
    }

    val canUndo get() = editHistory.position > 0
    val canRedo get() = editHistory.position < editHistory.history.size

    fun addTextChangeListener() {
        editText.addTextChangedListener(changeListener)
    }

    fun undo() {
        Timber.e("UNDOING")
        val edit = editHistory.previous ?: return
        val text = editText.editableText
        val start = edit.start
        val end = start + if (edit.after != null) edit.after.length else 0

        isUndoRedo = true
        text.replace(start, end, edit.before)
        isUndoRedo = false

        for (span in text.getSpans(0, text.length, UnderlineSpan::class.java)) {
            text.removeSpan(span)
        }

        editText.setSelection(if (edit.before == null) start else start + edit.before.length)

        updateUndoRedoButtons()
    }

    fun redo() {
        val edit: EditItem = editHistory.next ?: return
        val text = editText.editableText
        val start = edit.start
        val end = start + if (edit.before != null) edit.before.length else 0
        isUndoRedo = true
        text.replace(start, end, edit.after)
        isUndoRedo = false

        for (span in text.getSpans(0, text.length, UnderlineSpan::class.java)) {
            text.removeSpan(span)
        }

        editText.setSelection(if (edit.before == null) start else start + edit.before.length)

        updateUndoRedoButtons()
    }

    fun updateUndoRedoButtons() {
        toggleUndoButton(this.canUndo)
        toggleRedoButton(this.canRedo)
    }

    private inner class EditHistory {
        var position = 0
        private var maxHistorySize = 10
        val history = LinkedList<EditItem>()

        fun add(item: EditItem) {
            while (history.size > position) {
                history.removeLast()
            }
            history.add(item)
            position++

            if (maxHistorySize >= 0) {
                while (history.size > maxHistorySize) {
                    history.removeFirst()
                    position--
                }

                if (position < 0) {
                    position = 0
                }
            }
        }


        val previous: EditItem?
            get() {
                if (position == 0) {
                    return null
                }
                position--
                return history[position]
            }

        val next: EditItem?
            get() {
                if (position >= history.size - 1) {
                    return null
                }

                position++
                return history[position]
            }
    }

    private inner class EditItem(
        val start: Int,
        val before: CharSequence?,
        val after: CharSequence?
    )

    private inner class EditTextChangeListener : TextWatcher {
        private var beforeChange: CharSequence? = null
        private var afterChange: CharSequence? = null

        override fun beforeTextChanged(
            s: CharSequence, start: Int, count: Int,
            after: Int
        ) {
            if (!isUndoRedo) {
                beforeChange = s.subSequence(start, start + count)
            }
        }

        override fun onTextChanged(
            s: CharSequence, start: Int, before: Int,
            count: Int
        ) {
            if (!isUndoRedo) {
                afterChange = s.subSequence(start, start + count)
                editHistory.add(EditItem(start, beforeChange, afterChange))

                updateUndoRedoButtons()
            }
        }

        override fun afterTextChanged(s: Editable) {
            editorViewModel.waitAndSaveContents(s.toString())
        }
    }
}