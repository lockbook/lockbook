package app.lockbook.model

import android.app.Activity
import android.content.Context
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricPrompt
import androidx.core.content.ContextCompat
import androidx.fragment.app.FragmentActivity
import androidx.preference.PreferenceManager
import app.lockbook.R
import app.lockbook.util.getString
import timber.log.Timber
import java.lang.ref.WeakReference

enum class VerificationItem {
    BiometricsSettingsChange,
    ViewPrivateKey,
    OpenApp
}

object BiometricModel {
    fun isBiometricVerificationAvailable(context: Context): Boolean =
        BiometricManager.from(context)
            .canAuthenticate(BiometricManager.Authenticators.BIOMETRIC_WEAK) == BiometricManager.BIOMETRIC_SUCCESS

    fun verify(activity: Activity, verificationItem: VerificationItem, onSuccess: () -> Unit, onFailure: (() -> Unit)? = null) {
        val alertModel = AlertModel(WeakReference(activity))
        val context = activity.applicationContext

        when (doVerificationOrNot(context, verificationItem)) {
            true -> {
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
            false -> onSuccess()
        }
    }

    private fun doVerificationOrNot(context: Context, verificationItem: VerificationItem): Boolean {
        val currentBiometricValue = PreferenceManager.getDefaultSharedPreferences(context).getString(
            getString(context.resources, R.string.biometric_key),
            getString(context.resources, R.string.biometric_none_value)
        )

        val isStrict = currentBiometricValue == getString(context.resources, R.string.biometric_strict_value)
        val isRecommended = currentBiometricValue == getString(context.resources, R.string.biometric_recommended_value)

        val isABiometricsSettingChange = verificationItem == VerificationItem.BiometricsSettingsChange
        val isViewingPrivateKey = verificationItem == VerificationItem.ViewPrivateKey

        return isStrict || isRecommended && isABiometricsSettingChange || isRecommended && isViewingPrivateKey
    }
}
