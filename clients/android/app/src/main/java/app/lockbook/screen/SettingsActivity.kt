package app.lockbook.screen

import android.os.Bundle
import androidx.appcompat.app.AppCompatActivity
import app.lockbook.R
import com.google.android.material.appbar.MaterialToolbar

class SettingsActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_settings)

        findViewById<MaterialToolbar>(R.id.settings_toolbar).setNavigationOnClickListener {
            finish()
        }

        supportFragmentManager
            .beginTransaction()
            .replace(
                R.id.settings_preference_layout,
                SettingsFragment()
            )
            .commit()
    }

    fun scrollToPreference(): Int? {
        return intent.extras?.getInt(SettingsFragment.SCROLL_TO_PREFERENCE_KEY)?.apply {
            if (this == 0) {
                return null
            }
        }
    }

    fun upgradeNow(): Boolean? {
        return intent.extras?.getBoolean(SettingsFragment.UPGRADE_NOW)
    }
}
