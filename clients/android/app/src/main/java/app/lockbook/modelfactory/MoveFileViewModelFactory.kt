package app.lockbook.modelfactory

import android.app.Application
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import app.lockbook.model.ListFilesViewModel
import timber.log.Timber

class MoveFileViewModelFactory(
    private val path: String,
    private val application: Application
) : ViewModelProvider.Factory {
    @Suppress("unchecked_cast")
    override fun <T : ViewModel?> create(modelClass: Class<T>): T {
        if (modelClass.isAssignableFrom(MoveFileViewModelFactory::class.java))
            return MoveFileViewModelFactory(path, application) as T
        throw IllegalArgumentException("Unknown ViewModel class")
    }
}
