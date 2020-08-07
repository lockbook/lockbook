package app.lockbook.loggedin.texteditor

import android.text.Editable
import android.text.TextWatcher
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.ViewModel
import timber.log.Timber

class TextEditorViewModel(initialContents: String): ViewModel(), TextWatcher {
    private var history: MutableList<String> = mutableListOf()
    private var historyIndex = 0
    var ignoreChange = false
    private val _canUndo = MutableLiveData<Boolean>()
    private val _canRedo = MutableLiveData<Boolean>()

    val canUndo: LiveData<Boolean>
        get() = _canUndo

    val canRedo: LiveData<Boolean>
        get() = _canRedo

    init {
        history.add(history.lastIndex + 1, initialContents)
    }

    override fun afterTextChanged(s: Editable?) {
        if(!ignoreChange) {
            if (history.size - 1 > historyIndex) {
                history.subList(historyIndex, history.size).clear()
            }

            if (history.size >= 10) {
                history.removeAt(history.lastIndex)
            } else {
                historyIndex++
            }
            history.add(history.lastIndex + 1, s.toString())
        } else {
            ignoreChange = false
        }
        canUndo()
        canRedo()
    }

    fun undo(): String {
        historyIndex--
        canUndo()
        return history[historyIndex]
    }

    fun redo(): String {
        historyIndex++
        canRedo()
        return history[historyIndex]
    }

    private fun canUndo() {
        Timber.i("Index: $historyIndex")
        _canUndo.value = historyIndex != 0
    }

    private fun canRedo() {
        _canRedo.value = historyIndex != history.lastIndex
    }


    override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {}

    override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {}


}