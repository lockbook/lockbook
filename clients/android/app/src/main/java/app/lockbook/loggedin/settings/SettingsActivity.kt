package app.lockbook.loggedin.settings

import android.os.Bundle
import android.util.Log
import androidx.appcompat.app.AppCompatActivity
import app.lockbook.R
import app.lockbook.utils.Config

class SettingsActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_settings)
        title = "Settings"

        supportFragmentManager
            .beginTransaction()
            .replace(
                R.id.settings_preference_layout,
                SettingsFragment(Config(application.filesDir.absolutePath))
            )
            .commit()
    }
}