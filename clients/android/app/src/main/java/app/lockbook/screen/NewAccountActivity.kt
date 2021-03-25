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
import app.lockbook.util.*
import app.lockbook.util.SharedPreferences.LOGGED_IN_KEY
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.android.synthetic.main.activity_new_account.*
import kotlinx.android.synthetic.main.splash_screen.*
import kotlinx.coroutines.*
import timber.log.Timber

class NewAccountActivity : AppCompatActivity() {

    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_new_account)

        new_account_create_lockbook.setOnClickListener {
            onClickCreateAccount()
        }

        new_account_username.setOnEditorActionListener { _, actionId, _ ->
            if (actionId == EditorInfo.IME_ACTION_DONE) {
                onClickCreateAccount()
            }

            true
        }
    }

    private fun onClickCreateAccount() {
        new_account_progress_bar.visibility = View.VISIBLE
        uiScope.launch {
            withContext(Dispatchers.IO) {
                handleCreateAccountResult()
            }
        }
    }

    private suspend fun handleCreateAccountResult() {
        val createAccountResult = CoreModel.generateAccount(
            Config(filesDir.absolutePath),
            new_account_username.text.toString()
        )

        withContext(Dispatchers.Main) {
            when (createAccountResult) {
                is Ok -> {
                    new_account_progress_bar.visibility = View.GONE
                    setUpLoggedInState()
                    startActivity(Intent(applicationContext, ListFilesActivity::class.java))
                    finishAffinity()
                }
                is Err -> {
                    new_account_progress_bar.visibility = View.GONE
                    when (val error = createAccountResult.error) {
                        is CreateAccountError.UsernameTaken ->
                            new_account_username.error =
                                "Username taken!"
                        is CreateAccountError.InvalidUsername ->
                            new_account_username.error =
                                "Invalid username!"
                        is CreateAccountError.CouldNotReachServer -> AlertModel.errorHasOccurred(
                            new_account_layout,
                            "Network unavailable.",
                            OnFinishAlert.DoNothingOnFinishAlert
                        )
                        is CreateAccountError.AccountExistsAlready -> AlertModel.errorHasOccurred(
                            new_account_layout,
                            "Account already exists.",
                            OnFinishAlert.DoNothingOnFinishAlert
                        )
                        is CreateAccountError.ClientUpdateRequired -> AlertModel.errorHasOccurred(
                            new_account_layout,
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
