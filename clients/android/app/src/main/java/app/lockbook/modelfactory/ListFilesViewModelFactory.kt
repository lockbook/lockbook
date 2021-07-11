package app.lockbook.modelfactory

import android.app.Application
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import app.lockbook.model.ListFilesViewModel

class ListFilesViewModelFactory(
    private val application: Application,
    private val isThisAnImport: Boolean
) : ViewModelProvider.Factory {
    @Suppress("unchecked_cast")
    override fun <T : ViewModel?> create(modelClass: Class<T>): T {
        if (modelClass.isAssignableFrom(ListFilesViewModel::class.java))
            return ListFilesViewModel(application, isThisAnImport) as T
        throw IllegalArgumentException("Unknown ViewModel class")
    }
}
