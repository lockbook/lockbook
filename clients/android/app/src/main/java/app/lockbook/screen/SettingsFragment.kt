package app.lockbook.screen

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.view.Gravity
import android.view.ViewGroup
import android.widget.PopupWindow
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricManager.Authenticators.BIOMETRIC_WEAK
import androidx.biometric.BiometricPrompt.*
import androidx.fragment.app.FragmentActivity
import androidx.preference.*
import app.lockbook.R
import app.lockbook.model.AlertModel
import app.lockbook.model.BiometricModel
import app.lockbook.model.CoreModel
import app.lockbook.model.OnFinishAlert
import app.lockbook.ui.NumberPickerPreference
import app.lockbook.ui.NumberPickerPreferenceDialog
import app.lockbook.util.*
import app.lockbook.util.SharedPreferences.BACKGROUND_SYNC_ENABLED_KEY
import app.lockbook.util.SharedPreferences.BACKGROUND_SYNC_PERIOD_KEY
import app.lockbook.util.SharedPreferences.BIOMETRIC_OPTION_KEY
import app.lockbook.util.SharedPreferences.BYTE_USAGE_KEY
import app.lockbook.util.SharedPreferences.CLEAR_LOGS_KEY
import app.lockbook.util.SharedPreferences.EXPORT_ACCOUNT_QR_KEY
import app.lockbook.util.SharedPreferences.EXPORT_ACCOUNT_RAW_KEY
import app.lockbook.util.SharedPreferences.VIEW_LOGS_KEY
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.google.zxing.BarcodeFormat
import com.journeyapps.barcodescanner.BarcodeEncoder
import kotlinx.android.synthetic.main.activity_account_qr_code.view.*
import timber.log.Timber
import java.io.File

class SettingsFragment : PreferenceFragmentCompat() {
    lateinit var config: Config
    private lateinit var selectedKey: String
    private lateinit var newValueForPref: String

    override fun onCreatePreferences(savedInstanceState: Bundle?, rootKey: String?) {
        setPreferencesFromResource(R.xml.settings_preference, rootKey)
        config = Config(requireContext().filesDir.absolutePath)
        setUpPreferences()
    }

    private fun setUpPreferences() {
        findPreference<Preference>(BIOMETRIC_OPTION_KEY)?.setOnPreferenceChangeListener { preference, newValue ->
            if (newValue is String) {
                newValueForPref = newValue

                BiometricModel.verify(requireContext(), requireActivity().findViewById(android.R.id.content), activity as FragmentActivity, ::matchKey)
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

        setCurrentUsage()

        if (!isBiometricsOptionsAvailable()) {
            findPreference<ListPreference>(BIOMETRIC_OPTION_KEY)?.isEnabled = false
        }
    }

    private fun setCurrentUsage() {
        when (val getUsageHumanStringResult = CoreModel.getUsageHumanString(config, false)) {
            is Ok -> findPreference<Preference>(BYTE_USAGE_KEY)?.summary = getUsageHumanStringResult.value
            is Err -> when (val error = getUsageHumanStringResult.error) {
                GetUsageError.NoAccount -> {
                    AlertModel.errorHasOccurred(requireActivity().findViewById(android.R.id.content), "Error! No account.", OnFinishAlert.DoNothingOnFinishAlert)
                    findPreference<Preference>(BYTE_USAGE_KEY)?.summary =
                        "Error! No account."
                }
                GetUsageError.CouldNotReachServer -> {
                    AlertModel.errorHasOccurred(requireActivity().findViewById(android.R.id.content), "You are offline.", OnFinishAlert.DoNothingOnFinishAlert)
                    findPreference<Preference>(BYTE_USAGE_KEY)?.summary =
                        resources.getString(R.string.list_files_offline_snackbar)
                }
                GetUsageError.ClientUpdateRequired -> {
                    AlertModel.errorHasOccurred(requireActivity().findViewById(android.R.id.content), "Update required.", OnFinishAlert.DoNothingOnFinishAlert)
                    findPreference<Preference>(BYTE_USAGE_KEY)?.summary =
                        "Update required."
                }
                is GetUsageError.Unexpected -> {
                    AlertModel.unexpectedCoreErrorHasOccurred(requireContext(), error.error, OnFinishAlert.DoNothingOnFinishAlert)
                    Timber.e("Unable to get usage: ${error.error}")
                }
            }
        }.exhaustive
    }

    override fun onDisplayPreferenceDialog(preference: Preference?) {
        if (preference is NumberPickerPreference) {
            val numberPickerPreferenceDialog =
                NumberPickerPreferenceDialog.newInstance(preference.key)
            numberPickerPreferenceDialog.setTargetFragment(this, 0)
            numberPickerPreferenceDialog.show(parentFragmentManager, null)
        } else {
            super.onDisplayPreferenceDialog(preference)
        }
    }

    override fun onPreferenceTreeClick(preference: Preference?): Boolean {
        selectedKey = preference?.key ?: ""

        when (preference?.key) {
            EXPORT_ACCOUNT_QR_KEY, EXPORT_ACCOUNT_RAW_KEY -> {
                BiometricModel.verify(requireContext(), requireActivity().findViewById(android.R.id.content), activity as FragmentActivity, ::matchKey)
            }
            VIEW_LOGS_KEY -> startActivity(Intent(context, LogActivity::class.java))
            CLEAR_LOGS_KEY -> File("${config.writeable_path}/$LOG_FILE_NAME").writeText("")
            BACKGROUND_SYNC_ENABLED_KEY ->
                findPreference<Preference>(BACKGROUND_SYNC_PERIOD_KEY)?.isEnabled =
                    (preference as SwitchPreference).isChecked
            else -> super.onPreferenceTreeClick(preference)
        }

        return true
    }

    private fun matchKey() {
        when (selectedKey) {
            EXPORT_ACCOUNT_RAW_KEY -> exportAccountRaw()
            EXPORT_ACCOUNT_QR_KEY -> exportAccountQR()
            BIOMETRIC_OPTION_KEY -> changeBiometricPreference(newValueForPref)
            else -> {
                Timber.e("Shared preference key not matched: $selectedKey")
                AlertModel.errorHasOccurred(requireActivity().findViewById(android.R.id.content), BASIC_ERROR, OnFinishAlert.DoNothingOnFinishAlert)
            }
        }.exhaustive
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
                    is AccountExportError.NoAccount -> AlertModel.errorHasOccurred(
                        requireActivity().findViewById(android.R.id.content),
                        "Error! No account!",
                        OnFinishAlert.DoNothingOnFinishAlert
                    )
                    is AccountExportError.Unexpected -> {
                        AlertModel.unexpectedCoreErrorHasOccurred(requireContext(), error.error, OnFinishAlert.DoNothingOnFinishAlert)
                        Timber.e("Unable to export account: ${error.error}")
                    }
                }
            }
        }.exhaustive
    }

    private fun exportAccountRaw() {
        when (val exportResult = CoreModel.exportAccount(config)) {
            is Ok -> {
                val clipBoard: ClipboardManager =
                    requireContext().getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                val clipBoardData = ClipData.newPlainText("account string", exportResult.value)
                clipBoard.setPrimaryClip(clipBoardData)
                AlertModel.notify(
                    requireActivity().findViewById(android.R.id.content),
                    "Account string copied!", OnFinishAlert.DoNothingOnFinishAlert
                )
            }
            is Err -> when (val error = exportResult.error) {
                is AccountExportError.NoAccount -> AlertModel.errorHasOccurred(
                    requireActivity().findViewById(android.R.id.content),
                    "Error! No account!", OnFinishAlert.DoNothingOnFinishAlert
                )
                is AccountExportError.Unexpected -> {
                    AlertModel.unexpectedCoreErrorHasOccurred(requireContext(), error.error, OnFinishAlert.DoNothingOnFinishAlert)
                    Timber.e("Unable to export account: ${error.error}")
                }
            }
        }.exhaustive
    }

    private fun isBiometricsOptionsAvailable(): Boolean =
        BiometricManager.from(requireContext())
            .canAuthenticate(BIOMETRIC_WEAK) == BiometricManager.BIOMETRIC_SUCCESS
}
