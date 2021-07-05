package app.lockbook.model

import android.app.Activity
import android.content.Context
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricPrompt
import androidx.core.content.ContextCompat
import androidx.fragment.app.FragmentActivity
import androidx.preference.PreferenceManager
import app.lockbook.R
import app.lockbook.util.exhaustive
import app.lockbook.util.getString
import timber.log.Timber
import java.lang.ref.WeakReference

object BiometricModel {
    fun isBiometricVerificationAvailable(context: Context): Boolean =
        BiometricManager.from(context)
            .canAuthenticate(BiometricManager.Authenticators.BIOMETRIC_WEAK) == BiometricManager.BIOMETRIC_SUCCESS

    fun verify(activity: Activity, onSuccess: () -> Unit, onFailure: (() -> Unit)? = null, isThisBiometricsChange: Boolean = false,) {
        val alertModel = AlertModel(WeakReference(activity))
        val context = activity.applicationContext

        val biometricKey = getString(context.resources, R.string.biometric_key)
        val biometricNoneValue = getString(context.resources, R.string.biometric_none_value)
        val biometricRecommendedValue = getString(context.resources, R.string.biometric_recommended_value)
        val biometricStrictValue = getString(context.resources, R.string.biometric_strict_value)

        val pref = PreferenceManager.getDefaultSharedPreferences(context)
        val currentBiometricValue = pref.getString(
            biometricKey,
            biometricNoneValue
        )

        when {
            currentBiometricValue == biometricStrictValue ||
                isThisBiometricsChange && currentBiometricValue == biometricRecommendedValue -> {
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
                                BiometricPrompt.ERROR_LOCKOUT, BiometricPrompt.ERROR_LOCKOUT_PERMANENT, BiometricPrompt.ERROR_CANCELED, BiometricPrompt.ERROR_NEGATIVE_BUTTON, BiometricPrompt.ERROR_USER_CANCELED, BiometricPrompt.ERROR_TIMEOUT -> {
                                    if (onFailure != null) {
                                        onFailure()
                                    }
                                }
                                else -> {}
                            }
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
                    .setTitle(getString(context.resources, R.string.biometrics_title))
                    .setSubtitle(getString(context.resources, R.string.biometrics_subtitle))
                    .setAllowedAuthenticators(BiometricManager.Authenticators.BIOMETRIC_WEAK)
                    .setNegativeButtonText(getString(context.resources, R.string.biometrics_cancel))
                    .build()

                biometricPrompt.authenticate(promptInfo)
            }
            currentBiometricValue == biometricNoneValue || currentBiometricValue == biometricRecommendedValue -> onSuccess()
            else -> {
                Timber.e("Biometric shared preference does not match every supposed option: $currentBiometricValue")
                alertModel.notifyBasicError()
            }
        }.exhaustive
    }
}
