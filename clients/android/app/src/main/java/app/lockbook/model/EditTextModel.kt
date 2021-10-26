package app.lockbook.model

import android.text.Editable
import android.text.TextWatcher
import android.widget.EditText

class EditTextModel(
    private val editText: EditText,
    private val editorViewModel: TextEditorViewModel,
    private val toggleUndoButton: (Boolean) -> Unit,
    private val toggleRedoButton: (Boolean) -> Unit
) {
    private val editHistory get() = editorViewModel.editHistory
    private val changeListener: EditTextChangeListener

    private var isUndoRedo = false

    init {
        changeListener = EditTextChangeListener()
    }

    val canUndo get() = editHistory.position > 0
    val canRedo get() = editHistory.position < editHistory.history.size

    fun addTextChangeListener() {
        editText.addTextChangedListener(changeListener)
    }

    fun undo() {
        val change = editHistory.previous ?: return
        val text = editText.editableText

        val start = change.start
        val end = start + (change.after?.length ?: 0)

        isUndoRedo = true
        text.replace(start, end, change.before)
        isUndoRedo = false

        editText.setSelection(start + (change.before?.length ?: 0))

        updateUndoRedoButtons()
    }

    fun redo() {
        val change = editHistory.next ?: return
        val text = editText.editableText

        val start = change.start
        val end = start + (change.before?.length ?: 0)

        isUndoRedo = true
        text.replace(start, end, change.after)
        isUndoRedo = false

        editText.setSelection(start + (change.after?.length ?: 0))

        updateUndoRedoButtons()
    }

    fun updateUndoRedoButtons() {
        toggleUndoButton(this.canUndo)
        toggleRedoButton(this.canRedo)
    }

    class EditHistory {
        var position = 0
        val history = mutableListOf<EditItem>()
        var isDirty = false

        fun add(item: EditItem) {
            while (history.size > position) {
                history.removeLast()
            }

            history.add(item)
            position++

            while (history.size > MAX_HISTORY_SIZE) {
                history.removeFirst()
                position--
            }

            if (position < 0) {
                position = 0
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

        companion object {
            const val MAX_HISTORY_SIZE: Int = 30
        }
    }

    inner class EditItem(
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
