package app.lockbook.screen

import android.os.Bundle
import androidx.appcompat.app.AppCompatActivity
import app.lockbook.R
import com.google.android.material.appbar.MaterialToolbar

class SubAccountsActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_settings)
        findViewById<MaterialToolbar>(R.id.settings_toolbar).setNavigationOnClickListener {
            onBackPressedDispatcher.onBackPressed()
        }

        if (savedInstanceState == null) {
            supportFragmentManager
                .beginTransaction()
                .replace(
                    R.id.settings_preference_layout,
                    SubAccountsFragment(),
                ).commit()
        }
    }

    fun setToolbarTitle(title: String) {
        findViewById<MaterialToolbar>(R.id.settings_toolbar).title = title
    }
}
