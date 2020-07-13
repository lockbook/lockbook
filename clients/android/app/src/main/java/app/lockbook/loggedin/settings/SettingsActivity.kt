package app.lockbook.loggedin.settings

import android.app.Activity
import android.os.Bundle
import app.lockbook.R
import kotlinx.android.synthetic.main.activity_settings.*

class SettingsActivity : Activity() {
    companion object {
        private const val EXPORT_ACCOUNT_QR_CODE: Int = 1
        private const val EXPORT_ACCOUNT_RAW: Int = 2
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_settings)

        settings_list.adapter = SettingsAdapter(
            listOf("Export Account String (QR Code)", "Export Raw Account String"),
            applicationContext
        )

        settings_list.setOnItemClickListener { _, _, position, _ ->
            when (position) {
                EXPORT_ACCOUNT_QR_CODE -> {

                }
                EXPORT_ACCOUNT_RAW -> {

                }
            }
        }
    }
}
