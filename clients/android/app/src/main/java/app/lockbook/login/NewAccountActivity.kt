package app.lockbook.login

import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.util.Log
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.databinding.DataBindingUtil
import app.lockbook.InitialLaunchFigureOuter
import app.lockbook.loggedin.mainscreen.MainScreenActivity
import app.lockbook.R
import app.lockbook.core.createAccount
import app.lockbook.core.importAccount
import app.lockbook.databinding.ActivityNewAccountBinding
import app.lockbook.utils.Config
import app.lockbook.utils.CreateAccountError
import app.lockbook.utils.ImportError
import app.lockbook.utils.createAccountConverter
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.github.michaelbull.result.Result
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
                handleCreateAccountResult(createAccountFromString(username.text.toString()))
            }
        }
    }

    private fun createAccountFromString(account: String): Result<Unit, CreateAccountError> {
        val json = Klaxon()
        val config = json.toJsonString(Config(filesDir.absolutePath))

        val createResult: Result<Unit, CreateAccountError>? =
            json.converter(createAccountConverter).parse(createAccount(config, account))

        createResult?.let {
            return createResult
        }

        return Err(CreateAccountError.UnexpectedError("Unable to parse import json!"))
    }

    private suspend fun handleCreateAccountResult(createAccountResult: Result<Unit, CreateAccountError>) { // add an invalid string choice, as an empty textview will call an error
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
                        is CreateAccountError.InvalidUsername -> {
                            username.error = "Username Taken!"
                        }
                        is CreateAccountError.CouldNotReachServer -> {
                            Toast.makeText(
                                applicationContext,
                                "Network Unavailable",
                                Toast.LENGTH_LONG
                            ).show()
                        }
                        else -> {
                            Toast.makeText(
                                applicationContext,
                                "Unexpected error occured, please create a bug report (activity_settings)",
                                Toast.LENGTH_LONG
                            ).show()
                        }
                    }
                }
            }
        }
    }
}

