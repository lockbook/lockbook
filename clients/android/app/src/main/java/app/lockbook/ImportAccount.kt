package app.lockbook

import android.content.Intent
import android.os.Bundle
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import app.lockbook.core.importAccount
import kotlinx.android.synthetic.main.import_account.*

class ImportAccount : AppCompatActivity() {

    private val success = 0                 // should handle
    private val accountStringInvalid = 2    // should handle

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.import_account)

        import_lockbook.setOnClickListener {

        }

    }

    fun isAccountStringValid() {
        when (importAccount(filesDir.absolutePath, account_string.text.toString())) {
            success -> startActivity(Intent(applicationContext, ListFiles::class.java))
            accountStringInvalid -> Toast.makeText(
                applicationContext,
                "Account String invalid!",
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
