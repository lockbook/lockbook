package app.lockbook.loggedin.listfiles

import android.app.Application
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider

class ListFilesViewModelFactory(
    private val path: String,
    private val application: Application
) : ViewModelProvider.Factory {
    @Suppress("unchecked_cast")
    override fun <T : ViewModel?> create(modelClass: Class<T>): T {
        if (modelClass.isAssignableFrom(ListFilesViewModel::class.java))
            return ListFilesViewModel(path, application) as T
        throw IllegalArgumentException("Unknown ViewModel class")
    }
}
