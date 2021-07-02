package app.lockbook.model

import android.app.Activity
import android.content.Context
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricPrompt
import androidx.core.content.ContextCompat
import androidx.fragment.app.FragmentActivity
import androidx.preference.PreferenceManager
import app.lockbook.util.SharedPreferences
import app.lockbook.util.exhaustive
import timber.log.Timber
import java.lang.ref.WeakReference

object BiometricModel {
    fun isBiometricVerificationAvailable(context: Context): Boolean =
        BiometricManager.from(context)
            .canAuthenticate(BiometricManager.Authenticators.BIOMETRIC_WEAK) == BiometricManager.BIOMETRIC_SUCCESS

    fun verify(activity: Activity, onSuccess: () -> Unit) {
        val alertModel = AlertModel(WeakReference(activity))
        val context = activity.applicationContext

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
                    alertModel.notifyBasicError()
                }

                val executor = ContextCompat.getMainExecutor(context)
                val biometricPrompt = BiometricPrompt(
                    activity as FragmentActivity,
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
                                    alertModel.notifyBasicError()
                                }
                                BiometricPrompt.ERROR_LOCKOUT, BiometricPrompt.ERROR_LOCKOUT_PERMANENT -> {
                                    alertModel.notifyBasicError()
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
                alertModel.notifyBasicError()
            }
        }.exhaustive
    }
}
