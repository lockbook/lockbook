package app.lockbook.login

import android.content.Intent
import android.os.Bundle
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.preference.PreferenceManager
import app.lockbook.R
import app.lockbook.loggedin.listfiles.ListFilesActivity
import app.lockbook.utils.Config
import app.lockbook.utils.CoreError
import app.lockbook.utils.CoreModel
import app.lockbook.utils.Messages.UNEXPECTED_ERROR_OCCURRED
import app.lockbook.utils.SharedPreferences.LOGGED_IN_KEY
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.android.synthetic.main.activity_new_account.*
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
                        is CoreError.UsernameTaken ->
                            new_account_username.error =
                                "Username taken!"
                        is CoreError.InvalidUsername ->
                            new_account_username.error =
                                "Invalid username!"
                        is CoreError.CouldNotReachServer -> Toast.makeText(
                            applicationContext,
                            "Network unavailable.",
                            Toast.LENGTH_LONG
                        ).show()
                        is CoreError.AccountExistsAlready -> Toast.makeText(
                            applicationContext,
                            "Account already exists!",
                            Toast.LENGTH_LONG
                        ).show()
                        is CoreError.Unexpected -> {
                            Timber.e("Unable to create an account: ${error.error}")
                            Toast.makeText(
                                applicationContext,
                                UNEXPECTED_ERROR_OCCURRED,
                                Toast.LENGTH_LONG
                            ).show()
                        }
                        else -> {
                            Timber.e("CreateAccountError not matched: ${error::class.simpleName}.")
                            Toast.makeText(
                                applicationContext,
                                UNEXPECTED_ERROR_OCCURRED,
                                Toast.LENGTH_LONG
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
