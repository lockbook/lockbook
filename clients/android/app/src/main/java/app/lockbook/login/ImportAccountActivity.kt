package app.lockbook.login

import android.content.Intent
import android.os.Bundle
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.databinding.DataBindingUtil
import app.lockbook.mainscreen.MainScreenActivity
import app.lockbook.R
import app.lockbook.core.importAccount
import app.lockbook.databinding.ActivityImportAccountBinding
import kotlinx.android.synthetic.main.activity_import_account.*
import kotlinx.coroutines.*

class ImportAccountActivity : AppCompatActivity() {

    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)

    private val success = 0                 // should handle
    private val accountStringInvalid = 2    // should handle

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val binding: ActivityImportAccountBinding = DataBindingUtil.setContentView(this,
            R.layout.activity_import_account
        )
        binding.importAccountActivity = this
    }

    fun importAccountFromAccountString() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                when (importAccount(filesDir.absolutePath, account_string.text.toString())) {
                    success -> {
                        startActivity(Intent(applicationContext, MainScreenActivity::class.java))
                        finishAffinity()
                    }
                    accountStringInvalid -> Toast.makeText(
                        applicationContext,
                        "Account String invalid!",
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
