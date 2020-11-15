package app.lockbook.modelfactory

import android.app.Application
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import app.lockbook.model.HandwritingEditorViewModel

class HandwritingEditorViewModelFactory(
    private val application: Application,
    private val id: String,
) : ViewModelProvider.Factory {
    @Suppress("unchecked_cast")
    override fun <T : ViewModel?> create(modelClass: Class<T>): T {
        if (modelClass.isAssignableFrom(HandwritingEditorViewModel::class.java))
            return HandwritingEditorViewModel(application, id) as T
        throw IllegalArgumentException("Unknown ViewModel class")
    }
}
