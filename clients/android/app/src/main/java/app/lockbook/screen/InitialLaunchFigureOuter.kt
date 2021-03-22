package app.lockbook.screen

import android.content.Intent
import android.os.Bundle
import android.view.View
import androidx.appcompat.app.AppCompatActivity
import androidx.biometric.BiometricPrompt.*
import androidx.preference.PreferenceManager
import app.lockbook.R
import app.lockbook.model.AlertModel
import app.lockbook.model.BiometricModel
import app.lockbook.model.CoreModel
import app.lockbook.model.OnFinishAlert
import app.lockbook.util.*
import app.lockbook.util.SharedPreferences.BIOMETRIC_NONE
import app.lockbook.util.SharedPreferences.BIOMETRIC_OPTION_KEY
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.android.synthetic.main.splash_screen.*
import kotlinx.coroutines.*
import timber.log.Timber

const val STATE_REQUIRES_CLEANING =
    "This lockbook version is incompatible with your data, please clear your data or downgrade your lockbook."

class InitialLaunchFigureOuter : AppCompatActivity() {
    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.splash_screen)
        Timber.plant(Timber.DebugTree())

        handleOnDBState()
    }

    private fun handleOnDBState() {
        when (val getDBStateResult = CoreModel.getDBState(Config(filesDir.absolutePath))) {
            is Ok -> {
                when (getDBStateResult.value) {
                    State.Empty -> {
                        startActivity(Intent(this, WelcomeActivity::class.java))
                        finish()
                    }
                    State.ReadyToUse -> startFromExistingAccount()
                    State.MigrationRequired -> {
                        AlertModel.notify(
                            splash_screen,
                            "Upgrading data...",
                            OnFinishAlert.DoNothingOnFinishAlert
                        )
                        migrate_progress_bar.visibility = View.VISIBLE
                        migrateDB()
                    }
                    State.StateRequiresClearing -> {
                        Timber.e("DB state requires cleaning!")
                        AlertModel.errorHasOccurred(
                            splash_screen,
                            STATE_REQUIRES_CLEANING, OnFinishAlert.DoNothingOnFinishAlert
                        )
                    }
                }
            }
            is Err -> when (val error = getDBStateResult.error) {
                is GetStateError.Unexpected -> {
                    AlertModel.unexpectedCoreErrorHasOccurred(this, error.error, OnFinishAlert.DoNothingOnFinishAlert)
                    Timber.e("Unable to get DB State: ${error.error}")
                }
            }
        }.exhaustive
    }

    private fun migrateDB() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                when (val migrateDBResult = CoreModel.migrateDB(Config(filesDir.absolutePath))) {
                    is Ok -> {
                        withContext(Dispatchers.Main) {
                            migrate_progress_bar.visibility = View.GONE
                            AlertModel.notify(
                                splash_screen,
                                "Your data has been migrated.",
                                OnFinishAlert.DoSomethingOnFinishAlert(::startFromExistingAccount)
                            )
                        }
                    }
                    is Err -> when (val error = migrateDBResult.error) {
                        is MigrationError.StateRequiresCleaning -> {
                            withContext(Dispatchers.Main) {
                                migrate_progress_bar.visibility = View.GONE
                                AlertModel.errorHasOccurred(
                                    splash_screen,
                                    STATE_REQUIRES_CLEANING,
                                    OnFinishAlert.DoSomethingOnFinishAlert(::finish)
                                )
                            }
                            Timber.e("DB state requires cleaning!")
                        }
                        is MigrationError.Unexpected -> {
                            withContext(Dispatchers.Main) {
                                migrate_progress_bar.visibility = View.GONE
                                AlertModel.unexpectedCoreErrorHasOccurred(this@InitialLaunchFigureOuter, error.error, OnFinishAlert.DoSomethingOnFinishAlert(::finish))
                            }
                            Timber.e("Unable to migrate DB: ${error.error}")
                        }
                    }
                }.exhaustive
            }
        }
    }

    private fun startFromExistingAccount() {
        val pref = PreferenceManager.getDefaultSharedPreferences(this)

        if (!BiometricModel.isBiometricVerificationAvailable(this) && pref.getString(
                BIOMETRIC_OPTION_KEY,
                BIOMETRIC_NONE
            ) != BIOMETRIC_NONE
        ) {
            pref.edit()
                .putString(BIOMETRIC_OPTION_KEY, BIOMETRIC_NONE)
                .apply()
        }

        BiometricModel.verify(this, splash_screen, this, ::launchListFilesActivity)
    }

    private fun launchListFilesActivity() {
        val intent = Intent(this, ListFilesActivity::class.java)
        intent.addFlags(Intent.FLAG_ACTIVITY_NO_ANIMATION)
        overridePendingTransition(0, 0)
        startActivity(intent)
        finish()
    }
}
