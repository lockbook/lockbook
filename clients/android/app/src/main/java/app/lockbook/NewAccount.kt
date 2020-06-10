package app.lockbook

import android.content.Intent
import android.os.Bundle
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import app.lockbook.core.createAccount
import kotlinx.android.synthetic.main.new_account.*

class NewAccount : AppCompatActivity() {

    private val success = 0 // should handle
    private val networkError = 4 // should handle
    private val usernameTaken = 6 // should handle

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.new_account)

        create_lockbook.setOnClickListener {
            when (createAccount(filesDir.absolutePath, username.text.toString())) {
                success -> startActivity(Intent(applicationContext, ListFiles::class.java))
                usernameTaken -> username.error = "Username Taken!"
                networkError -> Toast.makeText(
                    applicationContext,
                    "Network Unavailable",
                    Toast.LENGTH_LONG
                ).show()
                else -> Toast.makeText(
                    applicationContext,
                    "Unexpected error occured, please create a bug report (settings)",
                    Toast.LENGTH_LONG
                ).show()
            }
        }
    }
}
