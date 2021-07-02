package app.lockbook.screen

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.view.Gravity
import android.view.ViewGroup
import android.widget.ImageView
import android.widget.PopupWindow
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricManager.Authenticators.BIOMETRIC_WEAK
import androidx.preference.*
import app.lockbook.R
import app.lockbook.model.AlertModel
import app.lockbook.model.BiometricModel
import app.lockbook.model.CoreModel
import app.lockbook.ui.NumberPickerPreference
import app.lockbook.ui.NumberPickerPreferenceDialog
import app.lockbook.util.*
import app.lockbook.util.SharedPreferences.BACKGROUND_SYNC_ENABLED_KEY
import app.lockbook.util.SharedPreferences.BACKGROUND_SYNC_PERIOD_KEY
import app.lockbook.util.SharedPreferences.BIOMETRIC_OPTION_KEY
import app.lockbook.util.SharedPreferences.CLEAR_LOGS_KEY
import app.lockbook.util.SharedPreferences.EXPORT_ACCOUNT_QR_KEY
import app.lockbook.util.SharedPreferences.EXPORT_ACCOUNT_RAW_KEY
import app.lockbook.util.SharedPreferences.VIEW_LOGS_KEY
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.google.zxing.BarcodeFormat
import com.journeyapps.barcodescanner.BarcodeEncoder
import timber.log.Timber
import java.io.File
import java.lang.ref.WeakReference

class SettingsFragment : PreferenceFragmentCompat() {
    lateinit var config: Config
    private lateinit var selectedKey: String
    private lateinit var newValueForPref: String

    val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()))
    }

    override fun onCreatePreferences(savedInstanceState: Bundle?, rootKey: String?) {
        setPreferencesFromResource(R.xml.settings_preference, rootKey)
        config = Config(requireContext().filesDir.absolutePath)
        setUpPreferences()
    }

    private fun setUpPreferences() {
        findPreference<Preference>(BIOMETRIC_OPTION_KEY)?.setOnPreferenceChangeListener { _, newValue ->
            if (newValue is String) {
                newValueForPref = newValue

                BiometricModel.verify(
                    requireActivity(),
                    ::matchKey
                )
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
            @Suppress("DEPRECATION")
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
                BiometricModel.verify(
                    requireActivity(),
                    ::matchKey
                )
            }
            VIEW_LOGS_KEY -> startActivity(Intent(context, LogActivity::class.java))
            CLEAR_LOGS_KEY -> File("${config.writeable_path}/${LogActivity.LOG_FILE_NAME}").writeText(
                ""
            )
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
                alertModel.notifyBasicError()
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
                qrCodeView.findViewById<ImageView>(R.id.qr_code).setImageBitmap(bitmap)
                val popUpWindow = PopupWindow(qrCodeView, 900, 900, true)
                popUpWindow.showAtLocation(view, Gravity.CENTER, 0, 0)
            }
            is Err -> alertModel.notifyError(exportResult.error.toLbError())
        }.exhaustive
    }

    private fun exportAccountRaw() {
        when (val exportResult = CoreModel.exportAccount(config)) {
            is Ok -> {
                val clipBoard: ClipboardManager =
                    requireContext().getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                val clipBoardData = ClipData.newPlainText("account string", exportResult.value)
                clipBoard.setPrimaryClip(clipBoardData)
                alertModel.notify(resIdToString(R.string.settings_export_account_copied))
            }
            is Err -> alertModel.notifyError(exportResult.error.toLbError())
        }.exhaustive
    }

    private fun isBiometricsOptionsAvailable(): Boolean =
        BiometricManager.from(requireContext())
            .canAuthenticate(BIOMETRIC_WEAK) == BiometricManager.BIOMETRIC_SUCCESS
}
