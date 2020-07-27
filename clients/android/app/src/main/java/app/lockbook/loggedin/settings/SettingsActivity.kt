package app.lockbook.loggedin.settings

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.os.Bundle
import android.view.Gravity
import android.widget.PopupWindow
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.databinding.DataBindingUtil
import androidx.lifecycle.Observer
import androidx.lifecycle.ViewModelProvider
import androidx.recyclerview.widget.LinearLayoutManager
import app.lockbook.R
import app.lockbook.databinding.ActivitySettingsBinding
import kotlinx.android.synthetic.main.activity_account_qr_code.*
import kotlinx.android.synthetic.main.activity_account_qr_code.view.*
import kotlinx.android.synthetic.main.activity_settings.*

class SettingsActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val binding: ActivitySettingsBinding = DataBindingUtil.setContentView(
            this,
            R.layout.activity_settings
        )

        val settings = resources.getStringArray(R.array.settings_names).toList()
        val settingsViewModelFactory =
            SettingsViewModelFactory(settings, application.filesDir.absolutePath)
        val settingsViewModel =
            ViewModelProvider(this, settingsViewModelFactory).get(SettingsViewModel::class.java)
        val adapter = SettingsAdapter(settings, settingsViewModel)

        binding.settingsViewModel = settingsViewModel
        binding.settingsList.adapter = adapter
        binding.settingsList.layoutManager = LinearLayoutManager(applicationContext)
        binding.lifecycleOwner = this

        settingsViewModel.errorHasOccurred.observe(this, Observer { errorText ->
            errorHasOccurred(errorText)
        })

        settingsViewModel.navigateToAccountQRCode.observe(this, Observer { qrBitmap ->
            navigateToAccountQRCode(qrBitmap)
        })

        settingsViewModel.copyAccountString.observe(this, Observer {accountString ->
            copyAccountString(accountString)
        })
    }

    private fun copyAccountString(accountString: String) {
        val clipBoard = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        val clipBoardData = ClipData.newPlainText("account string", accountString)
        clipBoard.setPrimaryClip(clipBoardData)
        Toast.makeText(this, "Account string copied!", Toast.LENGTH_LONG).show()
    }

    private fun navigateToAccountQRCode(qrBitmap: Bitmap) {
        val qrCodeView = layoutInflater.inflate(R.layout.activity_account_qr_code, null)
        qrCodeView.qr_code.setImageBitmap(qrBitmap)
        val popUpWindow = PopupWindow(qrCodeView, 900, 900, true)
        popUpWindow.showAtLocation(settings_linear_layout, Gravity.CENTER, 0, 0)
    }

    private fun errorHasOccurred(errorText: String) {
        Toast.makeText(this, errorText, Toast.LENGTH_LONG).show()
    }
}
