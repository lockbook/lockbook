package app.lockbook.screen

import android.app.Activity
import android.content.Intent
import android.os.Bundle
import android.view.View
import android.view.inputmethod.EditorInfo
import androidx.activity.result.contract.ActivityResultContracts.StartActivityForResult
import androidx.appcompat.app.AppCompatActivity
import androidx.preference.PreferenceManager
import app.lockbook.databinding.ActivityImportAccountBinding
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
import kotlinx.coroutines.*
import timber.log.Timber

class ImportAccountActivity : AppCompatActivity() {
    private var _binding: ActivityImportAccountBinding? = null
    // This property is only valid between onCreateView and
    // onDestroyView.
    private val binding get() = _binding!!

    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)

    private val alertModel by lazy {
        AlertModel(this)
    }

    private var onQRCodeResult = registerForActivityResult(StartActivityForResult()) { result ->
        if (result.resultCode == Activity.RESULT_OK) {
            uiScope.launch {
                withContext(Dispatchers.IO) {
                    val intentResult = IntentIntegrator.parseActivityResult(result.resultCode, result.data)

                    if (intentResult != null) {
                        intentResult.contents?.let { account ->
                            handleImportResult(
                                CoreModel.importAccount(
                                    Config(filesDir.absolutePath),
                                    account
                                )
                            )
                        }
                    }
                }
            }
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        _binding = ActivityImportAccountBinding.inflate(layoutInflater)
        setContentView(binding.root)

        binding.importLockbook.setOnClickListener {
            onClickImportAccount()
        }

        binding.qrImportButton.setOnClickListener {
            navigateToQRCodeScanner()
        }

        binding.textImportAccountString.setOnEditorActionListener { _, actionId, _ ->
            if (actionId == EditorInfo.IME_ACTION_DONE) {
                onClickImportAccount()
            }

            true
        }
    }

    private fun onClickImportAccount() {
        binding.importAccountProgressBar.visibility = View.VISIBLE
        uiScope.launch {
            withContext(Dispatchers.IO) {
                handleImportResult(
                    CoreModel.importAccount(
                        Config(filesDir.absolutePath),
                        binding.textImportAccountString.text.toString()
                    )
                )
            }
        }
    }

    private fun navigateToQRCodeScanner() {
        onQRCodeResult.launch(
            IntentIntegrator(this)
                .setPrompt("Scan the account string QR Code.")
                .createScanIntent()
        )
    }

    private suspend fun handleImportResult(importAccountResult: Result<Unit, ImportError>) {
        withContext(Dispatchers.Main) {
            when (importAccountResult) {
                is Ok -> {
                    binding.importAccountProgressBar.visibility = View.GONE
                    setUpLoggedInImportState()
                    startActivity(Intent(applicationContext, ListFilesActivity::class.java))
                    finishAffinity()
                }
                is Err -> {
                    binding.importAccountProgressBar.visibility = View.GONE
                    when (val error = importAccountResult.error) {
                        is ImportError.AccountStringCorrupted -> alertModel.notify("Invalid account string!")
                        is ImportError.AccountExistsAlready -> AlertModel.errorHasOccurred(
                            binding.importAccountLayout,
                            "Account already exists!", OnFinishAlert.DoNothingOnFinishAlert
                        )
                        is ImportError.AccountDoesNotExist -> AlertModel.errorHasOccurred(
                            binding.importAccountLayout,
                            "That account does not exist on this server!", OnFinishAlert.DoNothingOnFinishAlert
                        )
                        is ImportError.UsernamePKMismatch -> AlertModel.errorHasOccurred(
                            binding.importAccountLayout,
                            "That username does not correspond with that public_key on this server!", OnFinishAlert.DoNothingOnFinishAlert
                        )
                        is ImportError.CouldNotReachServer -> AlertModel.errorHasOccurred(
                            binding.importAccountLayout,
                            "Could not reach server!", OnFinishAlert.DoNothingOnFinishAlert
                        )
                        is ImportError.ClientUpdateRequired -> AlertModel.errorHasOccurred(
                            binding.importAccountLayout,
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
