package app.lockbook

import android.content.Intent
import android.content.SharedPreferences
import android.os.Bundle
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.biometric.BiometricConstants
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricPrompt
import androidx.core.content.ContextCompat
import androidx.preference.PreferenceManager
import app.lockbook.loggedin.listfiles.ListFilesActivity
import app.lockbook.login.WelcomeActivity
import app.lockbook.utils.*
import app.lockbook.utils.Messages.UNEXPECTED_ERROR_OCCURRED
import app.lockbook.utils.SharedPreferences.BIOMETRIC_NONE
import app.lockbook.utils.SharedPreferences.BIOMETRIC_OPTION_KEY
import app.lockbook.utils.SharedPreferences.BIOMETRIC_RECOMMENDED
import app.lockbook.utils.SharedPreferences.BIOMETRIC_STRICT
import app.lockbook.utils.SharedPreferences.LOGGED_IN_KEY
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import timber.log.Timber

class InitialLaunchFigureOuter : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.splash_screen)
        Timber.plant(Timber.DebugTree())

        handleOnDBState()
    }

    private fun handleOnDBState() {
        when (val getDBStateResult = CoreModel.getDBState(Config(filesDir.absolutePath))) {
            is Ok -> {
                when(getDBStateResult.value) {
                    State.Empty -> {
                        startActivity(Intent(this, WelcomeActivity::class.java))
                        finish()
                    }
                    State.ReadyToUse -> {
                        val pref = PreferenceManager.getDefaultSharedPreferences(this)

                        if (!isBiometricsOptionsAvailable() && pref.getString(
                                BIOMETRIC_OPTION_KEY,
                                BIOMETRIC_NONE
                            ) != BIOMETRIC_NONE
                        ) {
                            pref.edit()
                                .putString(BIOMETRIC_OPTION_KEY, BIOMETRIC_NONE)
                                .apply()
                        }
                        performBiometricFlow(pref)
                    }
                    State.MigrationRequired -> {
                        Toast.makeText(applicationContext, "Your lockbook data is old and will require migrating to use this version, please wait.", Toast.LENGTH_LONG).show()
                    }
                    State.StateRequiresClearing -> {
                        Timber.e("DB state requires cleaning!")
                        Toast.makeText(applicationContext, "Your data is too old to use this Lockbook version, please clear your data in settings and open the app again.", Toast.LENGTH_LONG).show()
                    }
                    else -> {
                        Timber.e("State enum not matched: ${getDBStateResult.value::class.simpleName}")
                        Toast.makeText(applicationContext, UNEXPECTED_ERROR_OCCURRED, Toast.LENGTH_LONG)
                            .show()
                    }
                }
            }
            is Err -> when(val error = getDBStateResult.error) {
                is GetStateError.UnexpectedError -> {
                    Timber.e("Unable to get DB State: ${error.error}")
                    Toast.makeText(applicationContext, UNEXPECTED_ERROR_OCCURRED, Toast.LENGTH_LONG)
                        .show()
                }
                else -> {
                    Timber.e("GetStateError not matched: ${error::class.simpleName}")
                    Toast.makeText(applicationContext, UNEXPECTED_ERROR_OCCURRED, Toast.LENGTH_LONG)
                        .show()
                }
            }
        }

        finish()
    }

    private fun migrateDB() {
        when(val migrateDBResult = CoreModel.migrateDB(Config(filesDir.absolutePath))) {
            is Ok -> return
            is Err -> when(val error = migrateDBResult.error) {
                is MigrationError.StateRequiresCleaning -> {
                    Timber.e("DB state requires cleaning!")
                    Toast.makeText(applicationContext, "Your data is too old to use this Lockbook version, please clear your data in settings and open the app again.", Toast.LENGTH_LONG).show()
                }
                is MigrationError.UnexpectedError -> {
                    Timber.e("Unable to migrate DB: ${error.error}")
                    Toast.makeText(applicationContext, UNEXPECTED_ERROR_OCCURRED, Toast.LENGTH_LONG)
                        .show()
                }
                else -> {
                    Timber.e("MigrationError not matched: ${error::class.simpleName}")
                    Toast.makeText(applicationContext, UNEXPECTED_ERROR_OCCURRED, Toast.LENGTH_LONG)
                        .show()
                }
            }
        }

        finish()
    }

    private fun launchListFilesActivity() {
        val intent = Intent(this, ListFilesActivity::class.java)
        intent.addFlags(Intent.FLAG_ACTIVITY_NO_ANIMATION)
        overridePendingTransition(0, 0)
        startActivity(intent)
        finish()
    }

    private fun isBiometricsOptionsAvailable(): Boolean =
        BiometricManager.from(applicationContext)
            .canAuthenticate() == BiometricManager.BIOMETRIC_SUCCESS

    private fun performBiometricFlow(pref: SharedPreferences) {
        when (
            val optionValue = pref.getString(
                BIOMETRIC_OPTION_KEY,
                BIOMETRIC_NONE
            )
        ) {
            BIOMETRIC_STRICT -> {
                if (BiometricManager.from(applicationContext)
                    .canAuthenticate() != BiometricManager.BIOMETRIC_SUCCESS
                ) {
                    Timber.e("Biometric shared preference is strict despite no biometrics.")
                    Toast.makeText(this, UNEXPECTED_ERROR_OCCURRED, Toast.LENGTH_LONG)
                        .show()
                    finish()
                }

                val executor = ContextCompat.getMainExecutor(this)
                val biometricPrompt = BiometricPrompt(
                    this,
                    executor,
                    object : BiometricPrompt.AuthenticationCallback() {
                        override fun onAuthenticationError(
                            errorCode: Int,
                            errString: CharSequence
                        ) {
                            super.onAuthenticationError(errorCode, errString)
                            when (errorCode) {
                                BiometricConstants.ERROR_HW_UNAVAILABLE, BiometricConstants.ERROR_UNABLE_TO_PROCESS, BiometricConstants.ERROR_NO_BIOMETRICS, BiometricConstants.ERROR_HW_NOT_PRESENT -> {
                                    Timber.e("Biometric authentication error: $errString")
                                    Toast.makeText(
                                        applicationContext,
                                        UNEXPECTED_ERROR_OCCURRED,
                                        Toast.LENGTH_SHORT
                                    )
                                        .show()
                                    finish()
                                }
                                BiometricConstants.ERROR_LOCKOUT, BiometricConstants.ERROR_LOCKOUT_PERMANENT ->
                                    Toast.makeText(
                                        applicationContext,
                                        "Too many tries, try again later!",
                                        Toast.LENGTH_SHORT
                                    )
                                        .show()
                                else -> finish()
                            }
                        }

                        override fun onAuthenticationSucceeded(
                            result: BiometricPrompt.AuthenticationResult
                        ) {
                            super.onAuthenticationSucceeded(result)
                            launchListFilesActivity()
                        }
                    }
                )

                val promptInfo = BiometricPrompt.PromptInfo.Builder()
                    .setTitle("Lockbook Biometric Verification")
                    .setSubtitle("Enter your fingerprint to access lockbook.")
                    .setDeviceCredentialAllowed(true)
                    .build()

                biometricPrompt.authenticate(promptInfo)
            }
            BIOMETRIC_NONE, BIOMETRIC_RECOMMENDED -> launchListFilesActivity()
            else -> {
                Timber.e("Biometric shared preference does not match every supposed option: $optionValue")
                Toast.makeText(this, UNEXPECTED_ERROR_OCCURRED, Toast.LENGTH_LONG)
                    .show()
            }
        }
    }
}
