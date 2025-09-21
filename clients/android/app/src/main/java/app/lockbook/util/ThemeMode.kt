package app.lockbook.util

import android.content.Context
import androidx.appcompat.app.AppCompatDelegate
import androidx.core.content.edit
import androidx.preference.PreferenceManager
import app.lockbook.R

object ThemeMode {
    fun getThemeModes(context: Context): Array<String> {
        return arrayOf(context.getString(R.string.theme_light), context.getString(R.string.theme_dark), context.getString(R.string.theme_system))
    }

    fun getSavedThemeIndex(context: Context): Int {
        val pref = PreferenceManager.getDefaultSharedPreferences(context)

        val default = when (AppCompatDelegate.getDefaultNightMode()) {
            AppCompatDelegate.MODE_NIGHT_NO -> 0
            AppCompatDelegate.MODE_NIGHT_YES -> 1
            AppCompatDelegate.MODE_NIGHT_FOLLOW_SYSTEM -> 2
            else -> 2
        }

        return pref.getInt(context.getString(R.string.theme_mode_key), default)
    }

    private fun setThemeModeFromIndex(selected: Int) {
        when (selected) {
            0 -> AppCompatDelegate.setDefaultNightMode(AppCompatDelegate.MODE_NIGHT_NO)
            1 -> AppCompatDelegate.setDefaultNightMode(AppCompatDelegate.MODE_NIGHT_YES)
            2 -> AppCompatDelegate.setDefaultNightMode(AppCompatDelegate.MODE_NIGHT_FOLLOW_SYSTEM)
        }
    }

    fun saveAndSetThemeIndex(context: Context, selected: Int) {
        setThemeModeFromIndex(selected)

        PreferenceManager.getDefaultSharedPreferences(context).edit {
            putInt(context.getString(R.string.theme_mode_key), selected)
        }
    }

    fun affirmThemeModeFromSaved(context: Context) {
        setThemeModeFromIndex(getSavedThemeIndex(context))
    }
}
