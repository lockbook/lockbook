package app.lockbook.listfiles

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import java.io.File

class ListFilesViewModelFactory(
    private val path: String
): ViewModelProvider.Factory {
    @Suppress("unchecked_cast")
    override fun <T : ViewModel?> create(modelClass: Class<T>): T {
        if(modelClass.isAssignableFrom(ListFilesViewModel::class.java))
            return ListFilesViewModel(path) as T
        throw IllegalArgumentException("Unknown ViewModel class")
    }
}