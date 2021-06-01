package app.lockbook.screen

import android.content.Intent
import android.os.Bundle
import android.view.View
import android.view.inputmethod.EditorInfo
import androidx.appcompat.app.AppCompatActivity
import androidx.preference.PreferenceManager
import app.lockbook.databinding.ActivityNewAccountBinding
import app.lockbook.model.AlertModel
import app.lockbook.model.CoreModel
import app.lockbook.model.OnFinishAlert
import app.lockbook.util.*
import app.lockbook.util.SharedPreferences.LOGGED_IN_KEY
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.*
import timber.log.Timber

class NewAccountActivity : AppCompatActivity() {
    private var _binding: ActivityNewAccountBinding? = null
    // This property is only valid between onCreateView and
    // onDestroyView.
    private val binding get() = _binding!!

    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)

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
                    setUpLoggedInState()
                    startActivity(Intent(applicationContext, ListFilesActivity::class.java))
                    finishAffinity()
                }
                is Err -> {
                    binding.newAccountProgressBar.visibility = View.GONE
                    when (val error = createAccountResult.error) {
                        is CreateAccountError.UsernameTaken ->
                            binding.newAccountUsername.error =
                                "Username taken!"
                        is CreateAccountError.InvalidUsername ->
                            binding.newAccountUsername.error =
                                "Invalid username!"
                        is CreateAccountError.CouldNotReachServer -> AlertModel.errorHasOccurred(
                            binding.newAccountLayout,
                            "Network unavailable.",
                            OnFinishAlert.DoNothingOnFinishAlert
                        )
                        is CreateAccountError.AccountExistsAlready -> AlertModel.errorHasOccurred(
                            binding.newAccountLayout,
                            "Account already exists.",
                            OnFinishAlert.DoNothingOnFinishAlert
                        )
                        is CreateAccountError.ClientUpdateRequired -> AlertModel.errorHasOccurred(
                            binding.newAccountLayout,
                            "Update required.",
                            OnFinishAlert.DoNothingOnFinishAlert
                        )
                        is CreateAccountError.Unexpected -> {
                            AlertModel.unexpectedCoreErrorHasOccurred(this@NewAccountActivity, error.error, OnFinishAlert.DoNothingOnFinishAlert)
                            Timber.e("Unable to create account.")
                        }
                    }
                }
            }.exhaustive
        }
    }

    private fun setUpLoggedInState() {
        PreferenceManager.getDefaultSharedPreferences(this).edit().putBoolean(
            LOGGED_IN_KEY,
            true
        ).apply()
    }
}
