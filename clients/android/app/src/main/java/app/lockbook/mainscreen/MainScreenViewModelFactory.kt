package app.lockbook.mainscreen

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider

class MainScreenViewModelFactory(
    private val path: String
): ViewModelProvider.Factory {
    @Suppress("unchecked_cast")
    override fun <T : ViewModel?> create(modelClass: Class<T>): T {
        if(modelClass.isAssignableFrom(ListFilesViewModel::class.java))
            return ListFilesViewModel(path) as T
        throw IllegalArgumentException("Unknown ViewModel class")
    }
}