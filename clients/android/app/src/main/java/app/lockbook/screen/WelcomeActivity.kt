package app.lockbook.screen

import android.content.Intent
import android.os.Bundle
import androidx.appcompat.app.AppCompatActivity
import app.lockbook.R
import kotlinx.android.synthetic.main.activity_main.*

class WelcomeActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        welcome_new_lockbook.setOnClickListener {
            launchNewAccount()
        }

        welcome_import_lockbook.setOnClickListener {
            launchImportAccount()
        }
    }

    private fun launchNewAccount() {
        startActivity(Intent(applicationContext, NewAccountActivity::class.java))
    }

    private fun launchImportAccount() {
        startActivity(Intent(applicationContext, ImportAccountActivity::class.java))
    }
}
