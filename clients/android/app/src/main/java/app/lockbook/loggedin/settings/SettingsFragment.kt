package app.lockbook.loggedin.settings

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.os.Bundle
import android.view.Gravity
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.PopupWindow
import android.widget.Toast
import androidx.preference.Preference
import androidx.preference.PreferenceFragmentCompat
import app.lockbook.R
import app.lockbook.utils.AccountExportError
import app.lockbook.utils.Config
import app.lockbook.utils.CoreModel
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.google.zxing.BarcodeFormat
import com.journeyapps.barcodescanner.BarcodeEncoder
import kotlinx.android.synthetic.main.activity_account_qr_code.view.*

class SettingsFragment(private val config: Config) : PreferenceFragmentCompat() {
    override fun onCreatePreferences(savedInstanceState: Bundle?, rootKey: String?) {
        setPreferencesFromResource(R.xml.settings_preference, rootKey)
    }

    override fun onPreferenceTreeClick(preference: Preference?): Boolean {
        if (preference is Preference) {
            when (preference.key) {
                "export_account_raw" -> exportAccountRaw()
                "export_account_qr" -> exportAccountQR()
            }
        } else {
            Toast.makeText(context, "An unexpected error has occurred!", Toast.LENGTH_LONG).show()
            return false
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

                val qrCodeView = layoutInflater.inflate(R.layout.activity_account_qr_code, null)
                qrCodeView.qr_code.setImageBitmap(bitmap)
                val popUpWindow = PopupWindow(qrCodeView, 900, 900, true)
                popUpWindow.showAtLocation(view, Gravity.CENTER, 0, 0)
            }
            is Err -> {
                when (exportResult.error) {
                    is AccountExportError.NoAccount -> Toast.makeText(
                        context,
                        "Error! No account!",
                        Toast.LENGTH_LONG
                    ).show()
                    is AccountExportError.UnexpectedError -> Toast.makeText(
                        context,
                        "An unexpected error has occurred!",
                        Toast.LENGTH_LONG
                    ).show()
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
                Toast.makeText(context, "Account string copied!", Toast.LENGTH_LONG)
                    .show()
            }
            is Err -> when (exportResult.error) {
                is AccountExportError.NoAccount -> Toast.makeText(
                    context,
                    "Error! No account!",
                    Toast.LENGTH_LONG
                ).show()
                is AccountExportError.UnexpectedError -> Toast.makeText(
                    context,
                    "An unexpected error has occurred!",
                    Toast.LENGTH_LONG
                ).show()
            }
        }
    }
}