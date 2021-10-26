package app.lockbook.screen

import android.content.Intent
import android.os.Bundle
import android.view.View
import android.view.inputmethod.EditorInfo
import androidx.appcompat.app.AppCompatActivity
import app.lockbook.databinding.ActivityNewAccountBinding
import app.lockbook.model.AlertModel
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.*
import java.lang.ref.WeakReference

class NewAccountActivity : AppCompatActivity() {
    private var _binding: ActivityNewAccountBinding? = null

    // This property is only valid between onCreateView and
    // onDestroyView.
    private val binding get() = _binding!!

    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)

    private val alertModel by lazy {
        AlertModel(WeakReference(this))
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        _binding = ActivityNewAccountBinding.inflate(layoutInflater)
        setContentView(binding.root)

        binding.newAccountCreateLockbook.setOnClickListener {
            onClickCreateAccount()
        }

        binding.newAccountUsername.setOnEditorActionListener { _, actionId, _ ->
            if (actionId == EditorInfo.IME_ACTION_DONE) {
                onClickCreateAccount()
            }

            true
        }
    }

    private fun onClickCreateAccount() {
        binding.newAccountProgressBar.visibility = View.VISIBLE
        uiScope.launch {
            withContext(Dispatchers.IO) {
                handleCreateAccountResult()
            }
        }
    }

    private suspend fun handleCreateAccountResult() {
        val createAccountResult = CoreModel.generateAccount(
            Config(filesDir.absolutePath),
            binding.newAccountUsername.text.toString()
        )

        withContext(Dispatchers.Main) {
            when (createAccountResult) {
                is Ok -> {
                    binding.newAccountProgressBar.visibility = View.GONE
                    startActivity(Intent(applicationContext, MainScreenActivity::class.java))
                    finishAffinity()
                }
                is Err -> {
                    binding.newAccountProgressBar.visibility = View.GONE
                    when (val error = createAccountResult.error) {
                        is CreateAccountError.UsernameTaken,
                        is CreateAccountError.InvalidUsername ->
                            binding.newAccountUsername.error =
                                error.toLbError(resources).msg
                        else -> {
                            alertModel.notifyError(error.toLbError(resources))
                        }
                    }
                }
            }
        }
    }
}
