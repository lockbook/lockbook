package app.lockbook.loggedin.settings

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.graphics.Bitmap
import android.os.Bundle
import android.util.Log
import android.view.Gravity
import android.widget.PopupWindow
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.biometric.BiometricConstants
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricPrompt
import androidx.core.content.ContextCompat
import androidx.databinding.DataBindingUtil
import androidx.lifecycle.Observer
import androidx.lifecycle.ViewModelProvider
import androidx.recyclerview.widget.LinearLayoutManager
import app.lockbook.R
import app.lockbook.databinding.ActivitySettingsBinding
import app.lockbook.utils.SharedPreferences
import app.lockbook.utils.SharedPreferences.BIOMETRIC_NONE
import app.lockbook.utils.SharedPreferences.BIOMETRIC_RECOMMENDED
import app.lockbook.utils.SharedPreferences.BIOMETRIC_STRICT
import com.afollestad.materialdialogs.MaterialDialog
import com.afollestad.materialdialogs.list.listItemsSingleChoice
import kotlinx.android.synthetic.main.activity_account_qr_code.view.*
import kotlinx.android.synthetic.main.activity_settings.*

class SettingsActivity : AppCompatActivity() {
    private lateinit var settingsViewModel: SettingsViewModel

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val binding: ActivitySettingsBinding = DataBindingUtil.setContentView(
            this,
            R.layout.activity_settings
        )

        val settings = resources.getStringArray(R.array.settings_names).toList()
        val settingsViewModelFactory =
            SettingsViewModelFactory(application.filesDir.absolutePath)
        settingsViewModel =
            ViewModelProvider(this, settingsViewModelFactory).get(SettingsViewModel::class.java)
        val adapter = SettingsAdapter(settings, settingsViewModel, isBiometricsOptionsAvailable())

        binding.settingsViewModel = settingsViewModel
        binding.settingsList.adapter = adapter
        binding.settingsList.layoutManager = LinearLayoutManager(applicationContext)
        binding.lifecycleOwner = this

        settingsViewModel.errorHasOccurred.observe(
            this,
            Observer { errorText ->
                errorHasOccurred(errorText)
            }
        )

        settingsViewModel.navigateToAccountQRCode.observe(
            this,
            Observer { qrBitmap ->
                performBiometricFlow(
                    "Authenticate your fingerprint to view your account qr code.",
                    R.string.export_account_qr,
                    bitmap = qrBitmap
                )
            }
        )

        settingsViewModel.copyAccountString.observe(
            this,
            Observer { accountString ->
                performBiometricFlow(
                    "Authenticate your fingerprint to copy your account string.",
                    R.string.export_account_raw,
                    accountString = accountString
                )
            }
        )

        settingsViewModel.navigateToBiometricSetting.observe(
            this,
            Observer {
                performBiometricFlow(
                    "Authenticate your fingerprint to change your biometric settings.",
                    R.string.protect_account_biometric
                )
            }
        )
    }

    private fun isBiometricsOptionsAvailable(): Boolean =
        BiometricManager.from(applicationContext)
            .canAuthenticate() == BiometricManager.BIOMETRIC_SUCCESS

    private fun performBiometricFlow(
        subtitle: String,
        id: Int,
        bitmap: Bitmap? = null,
        accountString: String = ""
    ) {
        val checkedItem = getSharedPreferences(
            SharedPreferences.SHARED_PREF_FILE,
            Context.MODE_PRIVATE
        ).getInt(SharedPreferences.BIOMETRIC_OPTION_KEY, BIOMETRIC_NONE)

        when (checkedItem) {
            BIOMETRIC_RECOMMENDED, BIOMETRIC_STRICT -> {
                if (BiometricManager.from(applicationContext)
                    .canAuthenticate() != BiometricManager.BIOMETRIC_SUCCESS
                ) {
                    Toast.makeText(this, "An unexpected error has occurred!", Toast.LENGTH_LONG)
                        .show()
                    finish()
                }

                val executor = ContextCompat.getMainExecutor(this)
                val biometricPrompt = BiometricPrompt(
                    this, executor,
                    object : BiometricPrompt.AuthenticationCallback() {
                        override fun onAuthenticationError(
                            errorCode: Int,
                            errString: CharSequence
                        ) {
                            super.onAuthenticationError(errorCode, errString)
                            when (errorCode) {
                                BiometricConstants.ERROR_HW_UNAVAILABLE, BiometricConstants.ERROR_UNABLE_TO_PROCESS, BiometricConstants.ERROR_NO_BIOMETRICS, BiometricConstants.ERROR_HW_NOT_PRESENT -> {
                                    Log.i(
                                        "checkBiometricOption",
                                        "Biometric authentication error: $errString"
                                    )
                                    Toast.makeText(
                                        applicationContext,
                                        "An unexpected error has occurred!", Toast.LENGTH_SHORT
                                    )
                                        .show()
                                }
                                BiometricConstants.ERROR_LOCKOUT, BiometricConstants.ERROR_LOCKOUT_PERMANENT ->
                                    Toast.makeText(
                                        applicationContext,
                                        "Too many tries, try again later!", Toast.LENGTH_SHORT
                                    )
                                        .show()
                                else -> {
                                }
                            }
                        }

                        override fun onAuthenticationSucceeded(
                            result: BiometricPrompt.AuthenticationResult
                        ) {
                            super.onAuthenticationSucceeded(result)
                            launchSettingsItem(id, checkedItem, bitmap, accountString)
                        }
                    }
                )

                val promptInfo = BiometricPrompt.PromptInfo.Builder()
                    .setTitle("Lockbook Biometric Verification")
                    .setSubtitle(subtitle)
                    .setDeviceCredentialAllowed(true)
                    .build()

                biometricPrompt.authenticate(promptInfo)
            }
            BIOMETRIC_NONE -> {
                launchSettingsItem(id, checkedItem, bitmap, accountString)
            }
        }
    }

    private fun launchSettingsItem(
        id: Int,
        checkedItem: Int,
        bitmap: Bitmap?,
        accountString: String
    ) {
        if (id == R.string.protect_account_biometric) {
            navigateToBiometricSettings(checkedItem)
        } else if (id == R.string.export_account_qr && bitmap is Bitmap) {
            navigateToAccountQRCode(bitmap)
        } else if (id == R.string.export_account_raw && accountString.isNotEmpty()) {
            copyAccountString(accountString)
        } else {
            errorHasOccurred("An unexpected error has occurred!")
        }
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

    private fun navigateToBiometricSettings(checkedItem: Int) {
        var disabledIndices = intArrayOf()
        if (!isBiometricsOptionsAvailable()) {
            disabledIndices = intArrayOf(BIOMETRIC_RECOMMENDED, BIOMETRIC_STRICT)
        }

        MaterialDialog(this).show {
            setTheme(R.style.DarkDialog)
            title(R.string.biometrics_title)
            listItemsSingleChoice(
                R.array.settings_biometric_names,
                initialSelection = checkedItem,
                disabledIndices = disabledIndices
            ) { _, index, _ ->
                errorHasOccurred("Clicked $index")
                getSharedPreferences(
                    SharedPreferences.SHARED_PREF_FILE,
                    Context.MODE_PRIVATE
                ).edit()
                    .putInt(
                        BIOMETRIC_SERVICE,
                        index
                    )
                    .apply()
            }
            positiveButton(R.string.biometrics_save)
            negativeButton(R.string.biometrics_cancel)
        }
    }
}
