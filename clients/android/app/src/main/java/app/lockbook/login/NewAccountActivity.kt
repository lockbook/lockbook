package app.lockbook.login

import android.content.Intent
import android.os.Bundle
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.databinding.DataBindingUtil
import app.lockbook.listfiles.ListFilesActivity
import app.lockbook.R
import app.lockbook.core.createAccount
import app.lockbook.databinding.ActivityNewAccountBinding
import kotlinx.android.synthetic.main.activity_new_account.*
import kotlinx.coroutines.*

class NewAccountActivity : AppCompatActivity() {

    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)

    private val success = 0 // should handle
    private val networkError = 4 // should handle
    private val usernameTaken = 6 // should handle

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val binding: ActivityNewAccountBinding = DataBindingUtil.setContentView(this,
            R.layout.activity_new_account
        )
        binding.newAccountActivity = this
    }

    fun createAccount() { // add an invalid string choice, as an empty textview will call an error
        uiScope.launch {
            withContext(Dispatchers.IO) {
                when (createAccount(filesDir.absolutePath, username.text.toString())) {
                    success -> {
                        startActivity(Intent(applicationContext, ListFilesActivity::class.java))
                        finishAffinity()
                    }
                    usernameTaken -> username.error = "Username Taken!"
                    networkError -> Toast.makeText(
                        applicationContext,
                        "Network Unavailable",
                        Toast.LENGTH_LONG
                    ).show()
                    else -> Toast.makeText(
                        applicationContext,
                        "Unexpected error occured, please create a bug report (activity_settings)",
                        Toast.LENGTH_LONG
                    ).show()
                }
            }
        }
    }
}
