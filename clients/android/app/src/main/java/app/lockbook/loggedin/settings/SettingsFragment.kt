package app.lockbook.loggedin.settings

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.view.Gravity
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.PopupWindow
import androidx.appcompat.app.AlertDialog
import androidx.biometric.BiometricConstants
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricPrompt
import androidx.core.content.ContextCompat
import androidx.preference.*
import app.lockbook.R
import app.lockbook.loggedin.logs.LogActivity
import app.lockbook.utils.*
import app.lockbook.utils.Messages.UNEXPECTED_CLIENT_ERROR
import app.lockbook.utils.Messages.UNEXPECTED_ERROR
import app.lockbook.utils.SharedPreferences.BACKGROUND_SYNC_ENABLED_KEY
import app.lockbook.utils.SharedPreferences.BACKGROUND_SYNC_PERIOD_KEY
import app.lockbook.utils.SharedPreferences.BIOMETRIC_NONE
import app.lockbook.utils.SharedPreferences.BIOMETRIC_OPTION_KEY
import app.lockbook.utils.SharedPreferences.BIOMETRIC_RECOMMENDED
import app.lockbook.utils.SharedPreferences.BIOMETRIC_STRICT
import app.lockbook.utils.SharedPreferences.CLEAR_LOGS_KEY
import app.lockbook.utils.SharedPreferences.EXPORT_ACCOUNT_QR_KEY
import app.lockbook.utils.SharedPreferences.EXPORT_ACCOUNT_RAW_KEY
import app.lockbook.utils.SharedPreferences.VIEW_LOGS_KEY
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.google.android.material.snackbar.Snackbar
import com.google.zxing.BarcodeFormat
import com.journeyapps.barcodescanner.BarcodeEncoder
import kotlinx.android.synthetic.main.activity_account_qr_code.view.*
import kotlinx.android.synthetic.main.splash_screen.*
import timber.log.Timber
import java.io.File

class SettingsFragment : PreferenceFragmentCompat() {
    lateinit var config: Config

    override fun onCreatePreferences(savedInstanceState: Bundle?, rootKey: String?) {
        setPreferencesFromResource(R.xml.settings_preference, rootKey)
        config = Config(requireContext().filesDir.absolutePath)
        setUpPreferences()
    }

    private fun setUpPreferences() {
        findPreference<Preference>(BIOMETRIC_OPTION_KEY)?.setOnPreferenceChangeListener { preference, newValue ->
            if (newValue is String) {
                performBiometricFlow(preference.key, newValue)
            }

            false
        }

        findPreference<Preference>(BACKGROUND_SYNC_PERIOD_KEY)?.isEnabled =
            PreferenceManager.getDefaultSharedPreferences(
                requireContext()
            ).getBoolean(
                BACKGROUND_SYNC_ENABLED_KEY,
                true
            )

        if (!isBiometricsOptionsAvailable()) {
            findPreference<ListPreference>(BIOMETRIC_OPTION_KEY)?.isEnabled = false
        }
    }

    override fun onDisplayPreferenceDialog(preference: Preference?) {
        if (preference is NumberPickerPreference) {
            val numberPickerPreferenceDialog =
                NumberPickerPreferenceDialog.newInstance(preference.key)
            numberPickerPreferenceDialog.setTargetFragment(this, 0)
            numberPickerPreferenceDialog.show(requireFragmentManager(), null)
        } else {
            super.onDisplayPreferenceDialog(preference)
        }
    }

    override fun onPreferenceTreeClick(preference: Preference?): Boolean {
        when (preference?.key) {
            EXPORT_ACCOUNT_QR_KEY, EXPORT_ACCOUNT_RAW_KEY -> performBiometricFlow(preference.key)
            VIEW_LOGS_KEY -> startActivity(Intent(context, LogActivity::class.java))
            CLEAR_LOGS_KEY -> File("${config.writeable_path}/$LOG_FILE_NAME").writeText("")
            BACKGROUND_SYNC_ENABLED_KEY ->
                findPreference<Preference>(BACKGROUND_SYNC_PERIOD_KEY)?.isEnabled =
                    (preference as SwitchPreference).isChecked
            else -> super.onPreferenceTreeClick(preference)
        }

        return true
    }

    private fun performBiometricFlow(key: String, newValue: String = "") {
        when (
            val optionValue = PreferenceManager.getDefaultSharedPreferences(
                requireContext()
            ).getString(
                BIOMETRIC_OPTION_KEY,
                BIOMETRIC_NONE
            )
        ) {
            BIOMETRIC_RECOMMENDED, BIOMETRIC_STRICT -> {
                if (BiometricManager.from(requireContext())
                    .canAuthenticate() != BiometricManager.BIOMETRIC_SUCCESS
                ) {
                    Timber.e("Biometric shared preference is strict despite no biometrics.")
                    Snackbar.make(
                        requireActivity().findViewById(android.R.id.content),
                        UNEXPECTED_CLIENT_ERROR,
                        Snackbar.LENGTH_SHORT
                    ).show()
                    return
                }

                val executor = ContextCompat.getMainExecutor(requireContext())
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
                                    Snackbar.make(
                                        requireActivity().findViewById(android.R.id.content),
                                        UNEXPECTED_CLIENT_ERROR,
                                        Snackbar.LENGTH_SHORT
                                    ).show()
                                }
                                BiometricConstants.ERROR_LOCKOUT, BiometricConstants.ERROR_LOCKOUT_PERMANENT -> {
                                    Snackbar.make(
                                        requireActivity().findViewById(android.R.id.content),
                                        "Too many tries, try again later!",
                                        Snackbar.LENGTH_SHORT
                                    ).show()
                                }
                                else -> {}
                            }
                        }

                        override fun onAuthenticationSucceeded(
                            result: BiometricPrompt.AuthenticationResult
                        ) {
                            super.onAuthenticationSucceeded(result)
                            matchKey(key, newValue)
                        }
                    }
                )

                val promptInfo = BiometricPrompt.PromptInfo.Builder()
                    .setTitle("Lockbook Biometric Verification")
                    .setSubtitle("Enter your fingerprint to modify this biometric sensitive setting.")
                    .setDeviceCredentialAllowed(true)
                    .build()

                biometricPrompt.authenticate(promptInfo)
            }
            BIOMETRIC_NONE -> matchKey(key, newValue)
            else -> {
                Timber.e("Biometric shared preference does not match every supposed option: $optionValue")
                Snackbar.make(
                    requireActivity().findViewById(android.R.id.content),
                    UNEXPECTED_CLIENT_ERROR,
                    Snackbar.LENGTH_SHORT
                ).show()
            }
        }
    }

    private fun matchKey(key: String, newValue: String) {
        when (key) {
            EXPORT_ACCOUNT_RAW_KEY -> exportAccountRaw()
            EXPORT_ACCOUNT_QR_KEY -> exportAccountQR()
            BIOMETRIC_OPTION_KEY -> changeBiometricPreference(newValue)
            else -> {
                Timber.e("Shared preference key not matched: $key")
                Snackbar.make(
                    requireActivity().findViewById(android.R.id.content),
                    UNEXPECTED_CLIENT_ERROR,
                    Snackbar.LENGTH_SHORT
                ).show()
            }
        }
    }

    private fun changeBiometricPreference(newValue: String) {
        findPreference<ListPreference>(BIOMETRIC_OPTION_KEY)?.value = newValue
    }

    private fun exportAccountQR() {
        when (val exportResult = CoreModel.exportAccount(config)) {
            is Ok -> {
                val bitmap = BarcodeEncoder().encodeBitmap(
                    exportResult.value,
                    BarcodeFormat.QR_CODE,
                    400,
                    400
                )

                val qrCodeView = layoutInflater.inflate(
                    R.layout.activity_account_qr_code,
                    view as ViewGroup,
                    false
                )
                qrCodeView.qr_code.setImageBitmap(bitmap)
                val popUpWindow = PopupWindow(qrCodeView, 900, 900, true)
                popUpWindow.showAtLocation(view, Gravity.CENTER, 0, 0)
            }
            is Err -> {
                when (val error = exportResult.error) {
                    is AccountExportError.NoAccount -> Snackbar.make(
                        requireActivity().findViewById(android.R.id.content),
                        "Error! No account!",
                        Snackbar.LENGTH_SHORT
                    ).show()
                    is AccountExportError.Unexpected -> {
                        Timber.e("Unable to export account: ${error.error}")
                        AlertDialog.Builder(requireContext(), R.style.DarkBlue_Dialog)
                            .setTitle(UNEXPECTED_ERROR)
                            .setMessage(error.error)
                            .show()
                    }
                    else -> {
                        Timber.e("AccountExportError not matched: ${error::class.simpleName}.")
                        Snackbar.make(
                            requireActivity().findViewById(android.R.id.content),
                            UNEXPECTED_CLIENT_ERROR,
                            Snackbar.LENGTH_SHORT
                        ).show()
                    }
                }
            }
        }
    }

    private fun exportAccountRaw() {
        when (val exportResult = CoreModel.exportAccount(config)) {
            is Ok -> {
                val clipBoard: ClipboardManager =
                    requireContext().getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                val clipBoardData = ClipData.newPlainText("account string", exportResult.value)
                clipBoard.setPrimaryClip(clipBoardData)
                Snackbar.make(
                    requireActivity().findViewById(android.R.id.content),
                    "Account string copied!",
                    Snackbar.LENGTH_SHORT
                ).show()
            }
            is Err -> when (val error = exportResult.error) {
                is AccountExportError.NoAccount -> Snackbar.make(
                    requireActivity().findViewById(android.R.id.content),
                    "Error! No account!",
                    Snackbar.LENGTH_SHORT
                ).show()
                is AccountExportError.Unexpected -> {
                    Timber.e("Unable to export account: ${error.error}")
                    AlertDialog.Builder(requireContext(), R.style.DarkBlue_Dialog)
                        .setTitle(UNEXPECTED_ERROR)
                        .setMessage(error.error)
                        .show()
                }
                else -> {
                    Timber.e("AccountExportError not matched: ${error::class.simpleName}.")
                    Snackbar.make(
                        requireActivity().findViewById(android.R.id.content),
                        UNEXPECTED_CLIENT_ERROR,
                        Snackbar.LENGTH_SHORT
                    ).show()
                }
            }
        }
    }

    private fun isBiometricsOptionsAvailable(): Boolean =
        BiometricManager.from(requireContext())
            .canAuthenticate() == BiometricManager.BIOMETRIC_SUCCESS
}
