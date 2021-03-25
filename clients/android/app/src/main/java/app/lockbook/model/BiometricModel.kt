package app.lockbook.model

import android.content.Context
import android.view.View
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricPrompt
import androidx.core.content.ContextCompat
import androidx.fragment.app.FragmentActivity
import androidx.preference.PreferenceManager
import app.lockbook.util.BASIC_ERROR
import app.lockbook.util.SharedPreferences
import app.lockbook.util.exhaustive
import kotlinx.android.synthetic.main.splash_screen.*
import timber.log.Timber

object BiometricModel {
    fun isBiometricVerificationAvailable(context: Context): Boolean =
        BiometricManager.from(context)
            .canAuthenticate(BiometricManager.Authenticators.BIOMETRIC_WEAK) == BiometricManager.BIOMETRIC_SUCCESS

    fun verify(context: Context, view: View, fragmentActivity: FragmentActivity, onSuccess: () -> Unit) {
        val pref = PreferenceManager.getDefaultSharedPreferences(context)

        when (
            val optionValue = pref.getString(
                SharedPreferences.BIOMETRIC_OPTION_KEY,
                SharedPreferences.BIOMETRIC_NONE
            )
        ) {
            SharedPreferences.BIOMETRIC_STRICT -> {
                if (BiometricManager.from(context)
                    .canAuthenticate(BiometricManager.Authenticators.BIOMETRIC_WEAK) != BiometricManager.BIOMETRIC_SUCCESS
                ) {
                    Timber.e("Biometric shared preference is strict despite no biometrics.")
                    AlertModel.errorHasOccurred(
                        view,
                        BASIC_ERROR,
                        OnFinishAlert.DoNothingOnFinishAlert
                    )
                }

                val executor = ContextCompat.getMainExecutor(context)
                val biometricPrompt = BiometricPrompt(
                    fragmentActivity,
                    executor,
                    object : BiometricPrompt.AuthenticationCallback() {
                        override fun onAuthenticationError(
                            errorCode: Int,
                            errString: CharSequence
                        ) {
                            super.onAuthenticationError(errorCode, errString)
                            when (errorCode) {
                                BiometricPrompt.ERROR_HW_UNAVAILABLE, BiometricPrompt.ERROR_UNABLE_TO_PROCESS, BiometricPrompt.ERROR_NO_BIOMETRICS, BiometricPrompt.ERROR_HW_NOT_PRESENT -> {
                                    Timber.e("Biometric authentication error: $errString")
                                    AlertModel.errorHasOccurred(view, BASIC_ERROR, OnFinishAlert.DoNothingOnFinishAlert)
                                }
                                BiometricPrompt.ERROR_LOCKOUT, BiometricPrompt.ERROR_LOCKOUT_PERMANENT -> {
                                    AlertModel.errorHasOccurred(view, "Too many tries, try again later!", OnFinishAlert.DoNothingOnFinishAlert)
                                }
                                else -> {}
                            }.exhaustive
                        }

                        override fun onAuthenticationSucceeded(
                            result: BiometricPrompt.AuthenticationResult
                        ) {
                            super.onAuthenticationSucceeded(result)
                            onSuccess()
                        }
                    }
                )

                val promptInfo = BiometricPrompt.PromptInfo.Builder()
                    .setTitle("Lockbook Biometric Verification")
                    .setSubtitle("Verify your identity to access Lockbook.")
                    .setAllowedAuthenticators(BiometricManager.Authenticators.BIOMETRIC_WEAK)
                    .setNegativeButtonText("Cancel")
                    .build()

                biometricPrompt.authenticate(promptInfo)
            }
            SharedPreferences.BIOMETRIC_NONE, SharedPreferences.BIOMETRIC_RECOMMENDED -> onSuccess()
            else -> {
                Timber.e("Biometric shared preference does not match every supposed option: $optionValue")
                AlertModel.errorHasOccurred(view, BASIC_ERROR, OnFinishAlert.DoNothingOnFinishAlert)
            }
        }.exhaustive
    }
}
