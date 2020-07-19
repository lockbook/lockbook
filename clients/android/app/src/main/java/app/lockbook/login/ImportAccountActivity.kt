package app.lockbook.login

import android.content.Intent
import android.os.Bundle
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.databinding.DataBindingUtil
import app.lockbook.loggedin.mainscreen.MainScreenActivity
import app.lockbook.R
import app.lockbook.core.importAccount
import app.lockbook.databinding.ActivityImportAccountBinding
import app.lockbook.utils.Config
import app.lockbook.utils.ImportError
import app.lockbook.utils.importAccountConverter
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.github.michaelbull.result.Result
import com.google.zxing.integration.android.IntentIntegrator
import kotlinx.android.synthetic.main.activity_import_account.*
import kotlinx.coroutines.*

class ImportAccountActivity : AppCompatActivity() {

    companion object {
        private const val QR_CODE_SCANNER_REQUEST_CODE = 101
    }

    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)
    private val intentIntegrator: IntentIntegrator = IntentIntegrator(this)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val binding: ActivityImportAccountBinding = DataBindingUtil.setContentView(
            this,
            R.layout.activity_import_account
        )

        binding.importAccountActivity = this
    }

    fun onClickImportAccount() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                handleImportResult(importAccountFromString(account_string.text.toString()))
            }
        }
    }

    fun navigateToQRCodeScanner() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                intentIntegrator
                    .setBeepEnabled(true)
                    .setRequestCode(QR_CODE_SCANNER_REQUEST_CODE)
                    .setOrientationLocked(false)
                    .setDesiredBarcodeFormats(IntentIntegrator.QR_CODE)
                    .setPrompt("Scan the account string QR Code.")
                    .initiateScan()
            }
        }
    }

    private fun importAccountFromString(account: String): Result<Unit, ImportError> {
        val json = Klaxon()
        val config = json.toJsonString(Config(filesDir.absolutePath))

        val importResult: Result<Unit, ImportError>? = json.converter(importAccountConverter).parse(importAccount(config, account))

        importResult?.let {
            return importResult
        }

        return Err(ImportError.UnexpectedError("Unable to parse import json!"))
    }

    private fun handleImportResult(importResult: Result<Unit, ImportError>) {
        when (importResult) {
            is Ok -> {
                startActivity(Intent(applicationContext, MainScreenActivity::class.java))
                finishAffinity()
            }
            is Err ->
                if (importResult.error is ImportError.AccountStringCorrupted) {
                    Toast.makeText(
                        applicationContext,
                        "Account String invalid!",
                        Toast.LENGTH_LONG
                    ).show()
                } else {
                    Toast.makeText(
                        applicationContext,
                        "Unexpected error occurred, please create a bug report (activity_settings)",
                        Toast.LENGTH_LONG
                    ).show()
                }
        }
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                if (requestCode == QR_CODE_SCANNER_REQUEST_CODE) {
                    val intentResult =
                        IntentIntegrator.parseActivityResult(requestCode, resultCode, data)
                    if (intentResult != null) {
                        intentResult.contents?.let {
                            handleImportResult(importAccountFromString(it))
                        }
                    }
                }
                super.onActivityResult(requestCode, resultCode, data)
            }
        }
    }
}
