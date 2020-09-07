package app.lockbook.loggedin.editor

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider

class TextEditorViewModelFactory(
    private val id: String,
    private val path: String,
    private val initialContents: String
) : ViewModelProvider.Factory {
    @Suppress("unchecked_cast")
    override fun <T : ViewModel?> create(modelClass: Class<T>): T {
        if (modelClass.isAssignableFrom(TextEditorViewModel::class.java))
            return TextEditorViewModel(id, path, initialContents) as T
        throw IllegalArgumentException("Unknown ViewModel class")
    }
}
