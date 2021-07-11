package app.lockbook.modelfactory

import android.app.Application
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import app.lockbook.model.MoveFileViewModel

class MoveFileViewModelFactory(
    private val application: Application
) : ViewModelProvider.Factory {
    @Suppress("unchecked_cast")
    override fun <T : ViewModel?> create(modelClass: Class<T>): T {
        if (modelClass.isAssignableFrom(MoveFileViewModel::class.java))
            return MoveFileViewModel(application) as T
        throw IllegalArgumentException("Unknown ViewModel class")
    }
}
