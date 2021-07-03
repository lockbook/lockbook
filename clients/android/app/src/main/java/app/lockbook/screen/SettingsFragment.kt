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
import androidx.preference.*
import app.lockbook.App.Companion.config
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
import java.io.File
import java.lang.ref.WeakReference

class SettingsFragment : PreferenceFragmentCompat() {
    val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()))
    }

    override fun onCreatePreferences(savedInstanceState: Bundle?, rootKey: String?) {
        setPreferencesFromResource(R.xml.settings_preference, rootKey)
        setUpPreferences()
    }

    private fun setUpPreferences() {
        findPreference<Preference>(getString(R.string.biometric_key))?.setOnPreferenceChangeListener { _, newValue ->
            var shouldReturn = false

            if (newValue is String) {
                BiometricModel.verify(
                    requireActivity()) {
                    shouldReturn = true
                }
            }

            shouldReturn
        }

        findPreference<Preference>(BACKGROUND_SYNC_PERIOD_KEY)?.isEnabled =
            PreferenceManager.getDefaultSharedPreferences(
                requireContext()
            ).getBoolean(
                BACKGROUND_SYNC_ENABLED_KEY,
                true
            )

        if (!BiometricModel.isBiometricVerificationAvailable(requireContext())) {
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
        when (preference?.key) {
            EXPORT_ACCOUNT_QR_KEY -> BiometricModel.verify(
                requireActivity(),
                ::exportAccountQR
            )
            EXPORT_ACCOUNT_RAW_KEY -> BiometricModel.verify(
                    requireActivity(),
                    ::exportAccountRaw
                )
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
                alertModel.notify(app.lockbook.util.getString(R.string.settings_export_account_copied))
            }
            is Err -> alertModel.notifyError(exportResult.error.toLbError())
        }.exhaustive
    }
}
