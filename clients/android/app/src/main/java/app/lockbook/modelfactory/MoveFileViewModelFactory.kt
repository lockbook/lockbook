package app.lockbook.modelfactory

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import app.lockbook.model.MoveFileViewModel

class MoveFileViewModelFactory : ViewModelProvider.Factory {
    @Suppress("unchecked_cast")
    override fun <T : ViewModel?> create(modelClass: Class<T>): T {
        if (modelClass.isAssignableFrom(MoveFileViewModel::class.java))
            return MoveFileViewModel() as T
        throw IllegalArgumentException("Unknown ViewModel class")
    }
}
