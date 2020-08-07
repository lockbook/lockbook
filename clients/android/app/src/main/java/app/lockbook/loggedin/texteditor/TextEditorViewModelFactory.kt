package app.lockbook.loggedin.texteditor

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider

class TextEditorViewModelFactory(val initialContents: String) : ViewModelProvider.Factory {
    @Suppress("unchecked_cast")
    override fun <T : ViewModel?> create(modelClass: Class<T>): T {
        if (modelClass.isAssignableFrom(TextEditorViewModel::class.java))
            return TextEditorViewModel(initialContents) as T
        throw IllegalArgumentException("Unknown ViewModel class")
    }
}
