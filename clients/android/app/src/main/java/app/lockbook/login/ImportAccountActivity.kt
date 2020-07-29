package app.lockbook.login

import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import app.lockbook.R
import app.lockbook.loggedin.mainscreen.MainScreenActivity
import app.lockbook.utils.Config
import app.lockbook.utils.CoreModel
import app.lockbook.utils.ImportError
import app.lockbook.utils.SharedPreferences.LOGGED_IN_KEY
import app.lockbook.utils.SharedPreferences.SHARED_PREF_FILE
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.github.michaelbull.result.Result
import com.google.zxing.integration.android.IntentIntegrator
import kotlinx.android.synthetic.main.activity_import_account.*
import kotlinx.coroutines.*

class ImportAccountActivity : AppCompatActivity() {
    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_import_account)

        welcome_import_lockbook.setOnClickListener {
            onClickImportAccount()
        }
        qr_import_button.setOnClickListener {
            navigateToQRCodeScanner()
        }
    }

    private fun onClickImportAccount() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                handleImportResult(
                    CoreModel.importAccount(
                        Config(filesDir.absolutePath),
                        text_import_account_string.text.toString()
                    )
                )
            }
        }
    }

    private fun navigateToQRCodeScanner() {
        IntentIntegrator(this)
            .setPrompt("Scan the account string QR Code.")
            .initiateScan()
    }

    private suspend fun handleImportResult(importAccountResult: Result<Unit, ImportError>) {
        withContext(Dispatchers.Main) {
            when (importAccountResult) {
                is Ok -> {
                    setUpLoggedInState()
                    startActivity(Intent(applicationContext, MainScreenActivity::class.java))
                    finishAffinity()
                }
                is Err -> when (importAccountResult.error) {
                    is ImportError.AccountStringCorrupted -> Toast.makeText(
                        applicationContext,
                        "Invalid Account String!",
                        Toast.LENGTH_LONG
                    ).show()
                    is ImportError.UnexpectedError -> Toast.makeText(
                        applicationContext,
                        "Unexpected error occurred, please create a bug report (activity_settings)",
                        Toast.LENGTH_LONG
                    ).show()
                }
            }
        }
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                val intentResult =
                    IntentIntegrator.parseActivityResult(requestCode, resultCode, data)
                if (intentResult != null) {
                    intentResult.contents?.let { account ->
                        handleImportResult(
                            CoreModel.importAccount(
                                Config(filesDir.absolutePath),
                                account
                            )
                        )
                    }
                } else {
                    super.onActivityResult(requestCode, resultCode, data)
                }
            }
        }
    }

    private fun setUpLoggedInState() {
        getSharedPreferences(
            SHARED_PREF_FILE,
            Context.MODE_PRIVATE
        ).edit().putBoolean(
            LOGGED_IN_KEY, true
        ).apply()
    }
}
