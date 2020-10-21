package app.lockbook.login

import android.content.Intent
import android.os.Bundle
import androidx.appcompat.app.AlertDialog
import androidx.appcompat.app.AppCompatActivity
import androidx.preference.PreferenceManager
import app.lockbook.R
import app.lockbook.loggedin.listfiles.ListFilesActivity
import app.lockbook.utils.Config
import app.lockbook.utils.CoreModel
import app.lockbook.utils.ImportError
import app.lockbook.utils.Messages.UNEXPECTED_ERROR
import app.lockbook.utils.SharedPreferences.IS_THIS_AN_IMPORT_KEY
import app.lockbook.utils.SharedPreferences.LOGGED_IN_KEY
import app.lockbook.utils.exhaustive
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.github.michaelbull.result.Result
import com.google.android.material.snackbar.Snackbar
import com.google.zxing.integration.android.IntentIntegrator
import kotlinx.android.synthetic.main.activity_import_account.*
import kotlinx.android.synthetic.main.splash_screen.*
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

    private suspend fun handleImportResult(importAccountResult: Result<Unit, ImportError>) {
        withContext(Dispatchers.Main) {
            when (importAccountResult) {
                is Ok -> {
                    setUpLoggedInImportState()
                    startActivity(Intent(applicationContext, ListFilesActivity::class.java))
                    finishAffinity()
                }
                is Err -> when (val error = importAccountResult.error) {
                    is ImportError.AccountStringCorrupted -> Snackbar.make(
                        splash_screen,
                        "Invalid account string!",
                        Snackbar.LENGTH_SHORT
                    ).show()
                    is ImportError.AccountExistsAlready -> Snackbar.make(
                        splash_screen,
                        "Account already exists!",
                        Snackbar.LENGTH_SHORT
                    ).show()
                    is ImportError.AccountDoesNotExist -> Snackbar.make(
                        splash_screen,
                        "That account does not exist on this server!",
                        Snackbar.LENGTH_SHORT
                    ).show()
                    is ImportError.UsernamePKMismatch -> Snackbar.make(
                        splash_screen,
                        "That username does not correspond with that public_key on this server!",
                        Snackbar.LENGTH_SHORT
                    ).show()
                    is ImportError.CouldNotReachServer -> Snackbar.make(
                        splash_screen,
                        "Could not access server to ensure this!",
                        Snackbar.LENGTH_SHORT
                    ).show()
                    is ImportError.ClientUpdateRequired -> Snackbar.make(
                        splash_screen,
                        "Update required.",
                        Snackbar.LENGTH_SHORT
                    ).show()
                    is ImportError.Unexpected -> {
                        AlertDialog.Builder(this@ImportAccountActivity, R.style.DarkBlue_Dialog)
                            .setTitle(UNEXPECTED_ERROR)
                            .setMessage(error.error)
                            .show()
                        Timber.e("Unable to import an account.")
                    }
                }
            }.exhaustive
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
