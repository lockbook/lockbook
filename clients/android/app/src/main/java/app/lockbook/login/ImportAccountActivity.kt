package app.lockbook.login

import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.util.Log
import android.view.LayoutInflater
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.databinding.DataBindingUtil
import app.lockbook.InitialLaunchFigureOuter.Companion.KEY
import app.lockbook.InitialLaunchFigureOuter.Companion.SHARED_PREF_FILE
import app.lockbook.loggedin.mainscreen.MainScreenActivity
import app.lockbook.R
import app.lockbook.core.importAccount
import app.lockbook.databinding.ActivityImportAccountBinding
import app.lockbook.loggedin.mainscreen.FileFolderModel
import app.lockbook.utils.Config
import app.lockbook.utils.ImportError
import app.lockbook.utils.importAccountConverter
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.github.michaelbull.result.Result
import com.google.zxing.integration.android.IntentIntegrator
import kotlinx.android.synthetic.main.activity_import_account.*
import kotlinx.android.synthetic.main.activity_new_account.*
import kotlinx.coroutines.*

class ImportAccountActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val binding: ActivityImportAccountBinding = DataBindingUtil.setContentView(
            this,
            R.layout.activity_import_account
        )

        binding.importAccountActivity = this
    }

    fun onClickImportAccount() {
        handleImportResult(
            FileFolderModel.importAccount(
                Config(filesDir.absolutePath),
                account_string.text.toString()
            )
        )
    }

    fun navigateToQRCodeScanner() {
        IntentIntegrator(this)
            .setPrompt("Scan the account string QR Code.")
            .initiateScan()
    }

    private fun handleImportResult(importAccountResult: Result<Unit, ImportError>) {
        when (importAccountResult) {
            is Ok -> {
                startActivity(Intent(applicationContext, MainScreenActivity::class.java))
                getSharedPreferences(SHARED_PREF_FILE, Context.MODE_PRIVATE).edit()
                    .putBoolean(KEY, true).apply()
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

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        val intentResult =
            IntentIntegrator.parseActivityResult(requestCode, resultCode, data)
        if (intentResult != null) {
            intentResult.contents?.let { account ->
                handleImportResult(
                    FileFolderModel.importAccount(
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
