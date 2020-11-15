package app.lockbook.screen

import android.os.Bundle
import androidx.appcompat.app.AppCompatActivity
import app.lockbook.R

class SettingsActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_settings)
        title = "Settings"

        supportFragmentManager
            .beginTransaction()
            .replace(
                R.id.settings_preference_layout,
                SettingsFragment()
            )
            .commit()
    }
}
