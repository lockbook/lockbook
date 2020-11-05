package app.lockbook.modelfactory

import android.app.Application
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import app.lockbook.model.ListFilesViewModel
import app.lockbook.model.MoveFileViewModel
import timber.log.Timber

class MoveFileViewModelFactory(
    private val path: String,
) : ViewModelProvider.Factory {
    @Suppress("unchecked_cast")
    override fun <T : ViewModel?> create(modelClass: Class<T>): T {
        if (modelClass.isAssignableFrom(MoveFileViewModel::class.java))
            return MoveFileViewModel(path) as T
        throw IllegalArgumentException("Unknown ViewModel class")
    }
}
