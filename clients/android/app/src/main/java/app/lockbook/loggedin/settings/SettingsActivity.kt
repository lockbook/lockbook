package app.lockbook.loggedin.settings

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.DialogInterface
import android.graphics.Bitmap
import android.os.Bundle
import android.util.Log
import android.view.Gravity
import android.widget.PopupWindow
import android.widget.Toast
import androidx.appcompat.app.AlertDialog
import androidx.appcompat.app.AppCompatActivity
import androidx.appcompat.view.ContextThemeWrapper
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
import kotlinx.android.synthetic.main.activity_account_qr_code.*
import kotlinx.android.synthetic.main.activity_account_qr_code.view.*
import kotlinx.android.synthetic.main.activity_import_account.*
import kotlinx.android.synthetic.main.activity_settings.*

class SettingsActivity : AppCompatActivity() {
    lateinit var settingsViewModel: SettingsViewModel

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val binding: ActivitySettingsBinding = DataBindingUtil.setContentView(
            this,
            R.layout.activity_settings
        )

        val settings = resources.getStringArray(R.array.settings_names).toList()
        val settingsViewModelFactory =
            SettingsViewModelFactory(settings, application.filesDir.absolutePath)
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
                checkBiometricOptionsQr(qrBitmap)
            }
        )

        settingsViewModel.copyAccountString.observe(
            this,
            Observer { accountString ->
                checkBiometricOptionsCopy(accountString)
            }
        )

        settingsViewModel.navigateToBiometricSetting.observe(
            this,
            Observer {
                checkBiometricOptionsSettings()
            }
        )
    }

    private fun isBiometricsOptionsAvailable(): Boolean =
        BiometricManager.from(applicationContext)
            .canAuthenticate() == BiometricManager.BIOMETRIC_SUCCESS

    private fun checkBiometricOptionsSettings() {
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
                            navigateToBiometricSettings(checkedItem)
                        }
                    }
                )

                val promptInfo = BiometricPrompt.PromptInfo.Builder()
                    .setTitle("Lockbook Biometric Verification")
                    .setSubtitle("Login to edit your biometric settings.")
                    .setDeviceCredentialAllowed(true)
                    .build()

                biometricPrompt.authenticate(promptInfo)
            }
            BIOMETRIC_NONE -> navigateToBiometricSettings(checkedItem)
        }
    }

    private fun checkBiometricOptionsCopy(accountString: String) {
        when (
            getSharedPreferences(
                SharedPreferences.SHARED_PREF_FILE,
                Context.MODE_PRIVATE
            ).getInt(SharedPreferences.BIOMETRIC_OPTION_KEY, BIOMETRIC_NONE)
        ) {
            BIOMETRIC_RECOMMENDED -> {
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
                            copyAccountString(accountString)
                        }
                    }
                )

                val promptInfo = BiometricPrompt.PromptInfo.Builder()
                    .setTitle("Lockbook Biometric Verification")
                    .setSubtitle("Login to view your account string.")
                    .setDeviceCredentialAllowed(true)
                    .build()

                biometricPrompt.authenticate(promptInfo)
            }
            BIOMETRIC_STRICT, BIOMETRIC_NONE -> copyAccountString(accountString)
        }
    }

    private fun checkBiometricOptionsQr(qrBitmap: Bitmap) {
        when (
            getSharedPreferences(
                SharedPreferences.SHARED_PREF_FILE,
                Context.MODE_PRIVATE
            ).getInt(SharedPreferences.BIOMETRIC_OPTION_KEY, BIOMETRIC_NONE)
        ) {
            BIOMETRIC_RECOMMENDED -> {
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
                            navigateToAccountQRCode(qrBitmap)
                        }
                    }
                )

                val promptInfo = BiometricPrompt.PromptInfo.Builder()
                    .setTitle("Lockbook Biometric Verification")
                    .setSubtitle("Login to view your account string.")
                    .setDeviceCredentialAllowed(true)
                    .build()

                biometricPrompt.authenticate(promptInfo)
            }
            BIOMETRIC_STRICT, BIOMETRIC_NONE -> navigateToAccountQRCode(qrBitmap)
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
        val builder = AlertDialog.Builder(
            ContextThemeWrapper(
                this,
                R.style.Theme_AppCompat_Dialog
            )
        )
        builder
            .setTitle(getString(R.string.biometrics_title))
            .setSingleChoiceItems(R.array.settings_biometric_names, checkedItem, null)
            .setPositiveButton(
                R.string.biometrics_save,
                DialogInterface.OnClickListener { dialog, _ ->
                    getSharedPreferences(
                        SharedPreferences.SHARED_PREF_FILE,
                        Context.MODE_PRIVATE
                    ).edit()
                        .putInt(
                            BIOMETRIC_SERVICE,
                            (dialog as AlertDialog).listView.checkedItemPosition
                        )
                        .apply()
                    dialog.dismiss()
                }
            )
            .setNegativeButton(
                R.string.biometrics_cancel,
                DialogInterface.OnClickListener { dialog, _ ->
                    dialog.cancel()
                }
            )
            .show()
    }
}
