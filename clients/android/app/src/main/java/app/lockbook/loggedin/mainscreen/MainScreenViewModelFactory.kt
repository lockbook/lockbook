package app.lockbook.loggedin.mainscreen

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider

class MainScreenViewModelFactory(
    private val path: String
): ViewModelProvider.Factory {
    @Suppress("unchecked_cast")
    override fun <T : ViewModel?> create(modelClass: Class<T>): T {
        if(modelClass.isAssignableFrom(MainScreenViewModel::class.java))
            return MainScreenViewModel(path) as T
        throw IllegalArgumentException("Unknown ViewModel class")
    }
}