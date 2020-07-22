package app.lockbook.login

import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.databinding.DataBindingUtil
import app.lockbook.InitialLaunchFigureOuter
import app.lockbook.loggedin.mainscreen.MainScreenActivity
import app.lockbook.R
import app.lockbook.databinding.ActivityNewAccountBinding
import app.lockbook.loggedin.mainscreen.FileFolderModel
import app.lockbook.utils.Config
import app.lockbook.utils.CreateAccountError
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.android.synthetic.main.activity_new_account.*
import kotlinx.coroutines.*

class NewAccountActivity : AppCompatActivity() {

    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val binding: ActivityNewAccountBinding = DataBindingUtil.setContentView(
            this,
            R.layout.activity_new_account
        )
        binding.newAccountActivity = this
    }

    fun onClickCreateAccount() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                handleCreateAccountResult()
            }
        }
    }

    private suspend fun handleCreateAccountResult() { // add an invalid string choice, as an empty textview will call an error
        val createAccountResult = FileFolderModel.generateAccount(
            Config(filesDir.absolutePath),
            username.text.toString()
        )

        withContext(Dispatchers.Main) {
            when (createAccountResult) {
                is Ok -> {
                    startActivity(Intent(applicationContext, MainScreenActivity::class.java))
                    getSharedPreferences(
                        InitialLaunchFigureOuter.SHARED_PREF_FILE,
                        Context.MODE_PRIVATE
                    ).edit().putBoolean(
                        InitialLaunchFigureOuter.KEY, true
                    ).apply()
                    finishAffinity()
                }
                is Err -> {
                    when (createAccountResult.error) {
                        is CreateAccountError.UsernameTaken -> username.error = "Username Taken!"
                        is CreateAccountError.InvalidUsername -> username.error =
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
}

