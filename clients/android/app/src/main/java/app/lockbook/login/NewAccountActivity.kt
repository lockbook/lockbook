package app.lockbook.login

import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import app.lockbook.R
import app.lockbook.loggedin.listfiles.ListFilesActivity
import app.lockbook.utils.Config
import app.lockbook.utils.CoreModel
import app.lockbook.utils.CreateAccountError
import app.lockbook.utils.SharedPreferences.LOGGED_IN_KEY
import app.lockbook.utils.SharedPreferences.SHARED_PREF_FILE
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.android.synthetic.main.activity_new_account.*
import kotlinx.coroutines.*

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
                    when (createAccountResult.error) {
                        is CreateAccountError.UsernameTaken ->
                            new_account_username.error =
                                "Username Taken!"
                        is CreateAccountError.InvalidUsername ->
                            new_account_username.error =
                                "Invalid Username!"
                        is CreateAccountError.CouldNotReachServer -> Toast.makeText(
                            applicationContext,
                            "Network Unavailable",
                            Toast.LENGTH_LONG
                        ).show()
                        is CreateAccountError.UnexpectedError -> Toast.makeText(
                            applicationContext,
                            "Unexpected error occurred, please create a bug report (activity_settings)",
                            Toast.LENGTH_LONG
                        ).show()
                    }
                }
            }
        }
    }

    private fun setUpLoggedInState() {
        getSharedPreferences(
            SHARED_PREF_FILE,
            Context.MODE_PRIVATE
        ).edit().putBoolean(
            LOGGED_IN_KEY, true
        ).apply()
    }
}
