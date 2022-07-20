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

    private val canUndo get() = editHistory.position >= 0
    private val canRedo get() = editHistory.position < editHistory.history.size - 1

    fun addTextChangeListener() {
        editText.addTextChangedListener(changeListener)
    }

    fun undo() {
        val change = editHistory.previous

        val start = change.start
        val end = start + change.after.length
        val newEnd = start + change.before.length

        isUndoRedo = true
        editText.editableText.replace(start, end, change.before)
        isUndoRedo = false

        editText.setSelection(newEnd)

        updateUndoRedoButtons()
    }

    fun redo() {
        val change = editHistory.next

        val start = change.start
        val end = start + change.before.length
        val newEnd = start + change.after.length

        isUndoRedo = true
        editText.editableText.replace(start, end, change.after)
        isUndoRedo = false

        editText.setSelection(newEnd)

        updateUndoRedoButtons()
    }

    fun updateUndoRedoButtons() {
        toggleUndoButton(this.canUndo)
        toggleRedoButton(this.canRedo)
    }

    class EditHistory {
        var position = -1
        val history = mutableListOf<EditItem>()
        var isDirty = false

        fun add(item: EditItem) {
            while (history.size - 1 > position) {
                history.removeLast()
            }

            if (position + 1 < MAX_HISTORY_SIZE) {
                position++
            } else {
                history.removeFirst()
            }

            history.add(item)
        }

        val previous: EditItem
            get() {
                val previous = history[position]
                position--

                return previous
            }

        val next: EditItem
            get() {
                position++

                return history[position]
            }

        companion object {
            const val MAX_HISTORY_SIZE: Int = 100
        }
    }

    inner class EditItem(
        val start: Int,
        val before: CharSequence,
        val after: CharSequence
    )

    private inner class EditTextChangeListener : TextWatcher {
        private lateinit var beforeChange: CharSequence
        private lateinit var afterChange: CharSequence

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
