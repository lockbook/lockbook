package app.lockbook.screen

import android.app.Activity
import android.content.Intent
import android.os.Bundle
import android.view.View
import android.view.inputmethod.EditorInfo
import androidx.activity.result.contract.ActivityResultContracts.StartActivityForResult
import androidx.appcompat.app.AppCompatActivity
import app.lockbook.databinding.ActivityImportAccountBinding
import app.lockbook.model.AlertModel
import app.lockbook.model.CoreModel
import app.lockbook.util.Config
import app.lockbook.util.ImportError
import app.lockbook.util.exhaustive
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.github.michaelbull.result.Result
import com.google.zxing.integration.android.IntentIntegrator
import kotlinx.coroutines.*
import java.lang.ref.WeakReference

class ImportAccountActivity : AppCompatActivity() {
    private var _binding: ActivityImportAccountBinding? = null
    // This property is only valid between onCreateView and
    // onDestroyView.
    private val binding get() = _binding!!

    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)

    private val alertModel by lazy {
        AlertModel(WeakReference(this))
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
                    val intent = Intent(applicationContext, MainScreenActivity::class.java)
                    intent.putExtra("is_this_an_import", true)

                    startActivity(intent)
                    finishAffinity()
                }
                is Err -> {
                    binding.importAccountProgressBar.visibility = View.GONE
                    alertModel.notifyError(importAccountResult.error.toLbError(resources))
                }
            }.exhaustive
        }
    }
}
