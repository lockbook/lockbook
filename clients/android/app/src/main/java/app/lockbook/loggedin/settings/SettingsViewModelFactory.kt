package app.lockbook.loggedin.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider

class SettingsViewModelFactory(
    private val settings: List<String>,
    private val path: String
): ViewModelProvider.Factory {
    @Suppress("unchecked_cast")
    override fun <T : ViewModel?> create(modelClass: Class<T>): T {
        if(modelClass.isAssignableFrom(SettingsViewModel::class.java))
            return SettingsViewModel(settings, path) as T
        throw IllegalArgumentException("Unknown ViewModel class")
    }
}