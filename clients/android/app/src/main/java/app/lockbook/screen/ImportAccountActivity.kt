package app.lockbook.screen

import android.content.Intent
import android.os.Bundle
import android.view.View
import android.view.inputmethod.EditorInfo
import androidx.appcompat.app.AppCompatActivity
import androidx.preference.PreferenceManager
import app.lockbook.R
import app.lockbook.model.AlertModel
import app.lockbook.model.CoreModel
import app.lockbook.model.OnFinishAlert
import app.lockbook.util.Config
import app.lockbook.util.ImportError
import app.lockbook.util.SharedPreferences.IS_THIS_AN_IMPORT_KEY
import app.lockbook.util.SharedPreferences.LOGGED_IN_KEY
import app.lockbook.util.exhaustive
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

        text_import_account_string.setOnEditorActionListener { _, actionId, _ ->
            if (actionId == EditorInfo.IME_ACTION_DONE) {
                onClickImportAccount()
            }

            true
        }
    }

    private fun onClickImportAccount() {
        import_account_progress_bar.visibility = View.VISIBLE
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
                    import_account_progress_bar.visibility = View.GONE
                    setUpLoggedInImportState()
                    startActivity(Intent(applicationContext, ListFilesActivity::class.java))
                    finishAffinity()
                }
                is Err -> {
                    import_account_progress_bar.visibility = View.GONE
                    when (val error = importAccountResult.error) {
                        is ImportError.AccountStringCorrupted -> AlertModel.errorHasOccurred(
                            import_account_layout,
                            "Invalid account string!", OnFinishAlert.DoNothingOnFinishAlert
                        )
                        is ImportError.AccountExistsAlready -> AlertModel.errorHasOccurred(
                            import_account_layout,
                            "Account already exists!", OnFinishAlert.DoNothingOnFinishAlert
                        )
                        is ImportError.AccountDoesNotExist -> AlertModel.errorHasOccurred(
                            import_account_layout,
                            "That account does not exist on this server!", OnFinishAlert.DoNothingOnFinishAlert
                        )
                        is ImportError.UsernamePKMismatch -> AlertModel.errorHasOccurred(
                            import_account_layout,
                            "That username does not correspond with that public_key on this server!", OnFinishAlert.DoNothingOnFinishAlert
                        )
                        is ImportError.CouldNotReachServer -> AlertModel.errorHasOccurred(
                            import_account_layout,
                            "Could not reach server!", OnFinishAlert.DoNothingOnFinishAlert
                        )
                        is ImportError.ClientUpdateRequired -> AlertModel.errorHasOccurred(
                            import_account_layout,
                            "Update required!", OnFinishAlert.DoNothingOnFinishAlert
                        )
                        is ImportError.Unexpected -> {
                            AlertModel.unexpectedCoreErrorHasOccurred(this@ImportAccountActivity, error.error, OnFinishAlert.DoNothingOnFinishAlert)
                            Timber.e("Unable to import an account.")
                        }
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
