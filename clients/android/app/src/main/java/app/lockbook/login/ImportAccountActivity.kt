package app.lockbook.login

import android.content.Intent
import android.os.Bundle
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.preference.PreferenceManager
import app.lockbook.R
import app.lockbook.loggedin.listfiles.ListFilesActivity
import app.lockbook.utils.Config
import app.lockbook.utils.CoreError
import app.lockbook.utils.CoreModel
import app.lockbook.utils.Messages.UNEXPECTED_ERROR_OCCURRED
import app.lockbook.utils.SharedPreferences.IS_THIS_AN_IMPORT_KEY
import app.lockbook.utils.SharedPreferences.LOGGED_IN_KEY
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.github.michaelbull.result.Result
import com.google.zxing.integration.android.IntentIntegrator
import kotlinx.android.synthetic.main.activity_import_account.*
import kotlinx.coroutines.*
import timber.log.Timber

class ImportAccountActivity : AppCompatActivity() {
    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_import_account)

        import_lockbook.setOnClickListener {
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

    private suspend fun handleImportResult(importAccountResult: Result<Unit, CoreError>) {
        withContext(Dispatchers.Main) {
            when (importAccountResult) {
                is Ok -> {
                    setUpLoggedInImportState()
                    startActivity(Intent(applicationContext, ListFilesActivity::class.java))
                    finishAffinity()
                }
                is Err -> when (val error = importAccountResult.error) {
                    is CoreError.AccountStringCorrupted -> Toast.makeText(
                        applicationContext,
                        "Invalid account string!",
                        Toast.LENGTH_LONG
                    ).show()
                    is CoreError.AccountExistsAlready -> Toast.makeText(
                        applicationContext,
                        "Account already exists!",
                        Toast.LENGTH_LONG
                    ).show()
                    is CoreError.AccountDoesNotExist -> Toast.makeText(
                        applicationContext,
                        "That account does not exist on this server!",
                        Toast.LENGTH_LONG
                    ).show()
                    is CoreError.UsernamePKMismatch -> Toast.makeText(
                        applicationContext,
                        "That username does not correspond with that public_key on this server!",
                        Toast.LENGTH_LONG
                    ).show()
                    is CoreError.CouldNotReachServer -> Toast.makeText(
                        applicationContext,
                        "Could not access server to ensure this !",
                        Toast.LENGTH_LONG
                    ).show()
                    is CoreError.Unexpected -> {
                        Timber.e("Unable to import an account.")
                        Toast.makeText(
                            applicationContext,
                            UNEXPECTED_ERROR_OCCURRED,
                            Toast.LENGTH_LONG
                        ).show()
                    }
                    else -> {
                        Timber.e("ImportError not matched: ${error::class.simpleName}.")
                        Toast.makeText(
                            applicationContext,
                            UNEXPECTED_ERROR_OCCURRED,
                            Toast.LENGTH_LONG
                        ).show()
                    }
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

    private fun setUpLoggedInImportState() {
        PreferenceManager.getDefaultSharedPreferences(this).edit().putBoolean(
            LOGGED_IN_KEY,
            true
        ).apply()
        PreferenceManager.getDefaultSharedPreferences(this).edit().putBoolean(
            IS_THIS_AN_IMPORT_KEY,
            true
        ).apply()
    }
}
