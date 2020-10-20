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
import app.lockbook.utils.CreateAccountError
import app.lockbook.utils.Messages
import app.lockbook.utils.Messages.UNEXPECTED_ERROR
import app.lockbook.utils.SharedPreferences.LOGGED_IN_KEY
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.google.android.material.snackbar.Snackbar
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
    }

    private fun onClickCreateAccount() {
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
                    setUpLoggedInState()
                    startActivity(Intent(applicationContext, ListFilesActivity::class.java))
                    finishAffinity()
                }
                is Err -> {
                    when (val error = createAccountResult.error) {
                        is CreateAccountError.UsernameTaken ->
                            new_account_username.error =
                                "Username taken!"
                        is CreateAccountError.InvalidUsername ->
                            new_account_username.error =
                                "Invalid username!"
                        is CreateAccountError.CouldNotReachServer -> Snackbar.make(
                            splash_screen,
                            "Network unavailable.",
                            Snackbar.LENGTH_SHORT
                        ).show()
                        is CreateAccountError.AccountExistsAlready -> Snackbar.make(
                            splash_screen,
                            "Account already exists.",
                            Snackbar.LENGTH_SHORT
                        ).show()
                        is CreateAccountError.Unexpected -> {
                            Timber.e("Unable to create account.")
                            AlertDialog.Builder(this@NewAccountActivity, R.style.DarkBlue_Dialog)
                                .setTitle(UNEXPECTED_ERROR)
                                .setMessage(error.error)
                                .show()
                        }
                        else -> {
                            Timber.e("CreateAccountError not matched: ${error::class.simpleName}.")
                            Snackbar.make(
                                splash_screen,
                                Messages.UNEXPECTED_CLIENT_ERROR,
                                Snackbar.LENGTH_SHORT
                            ).show()
                        }
                    }
                }
            }
        }
    }

    private fun setUpLoggedInState() {
        PreferenceManager.getDefaultSharedPreferences(this).edit().putBoolean(
            LOGGED_IN_KEY,
            true
        ).apply()
    }
}
