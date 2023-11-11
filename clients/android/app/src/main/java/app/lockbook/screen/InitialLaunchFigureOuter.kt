package app.lockbook.screen

import android.content.Intent
import android.os.Bundle
import androidx.appcompat.app.AppCompatActivity
import androidx.preference.PreferenceManager
import app.lockbook.R
import app.lockbook.databinding.SplashScreenBinding
import app.lockbook.model.AlertModel
import app.lockbook.model.BiometricModel
import app.lockbook.model.CoreModel
import app.lockbook.model.VerificationItem
import app.lockbook.util.CoreError
import app.lockbook.util.GetAccountError
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import java.lang.ref.WeakReference

class InitialLaunchFigureOuter : AppCompatActivity() {
    private var _binding: SplashScreenBinding? = null
    // This property is only valid between onCreateView and
    // onDestroyView.
    private val binding get() = _binding!!

    private val alertModel by lazy {
        AlertModel(WeakReference(this))
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        _binding = SplashScreenBinding.inflate(layoutInflater)
        setContentView(binding.root)

        handleAccountState()
    }

    private fun handleAccountState() {
        when (val getAccountResult = CoreModel.getAccount()) {
            is Ok -> startFromExistingAccount()
            is Err -> when (val error = getAccountResult.error) {
                is CoreError.UiError -> when (error.content) {
                    GetAccountError.NoAccount -> {
                        startActivity(Intent(this, OnBoardingActivity::class.java))
                        finish()
                    }
                }
                is CoreError.Unexpected -> alertModel.notifyError(error.toLbError(resources))
            }
        }
    }

    private fun startFromExistingAccount() {
        val pref = PreferenceManager.getDefaultSharedPreferences(this)
        val biometricKey = getString(R.string.biometric_key)
        val biometricNoneValue = getString(R.string.biometric_none_value)

        if (!BiometricModel.isBiometricVerificationAvailable(this) && pref.getString(
                biometricKey,
                biometricNoneValue
            ) != biometricNoneValue
        ) {
            pref.edit()
                .putString(biometricKey, biometricNoneValue)
                .apply()
        }

        BiometricModel.verify(this, VerificationItem.OpenApp, ::launchListFilesActivity, ::finish)
    }

    private fun launchListFilesActivity() {
        val intent = Intent(this, MainScreenActivity::class.java)
        intent.addFlags(Intent.FLAG_ACTIVITY_NO_ANIMATION)
        overridePendingTransition(0, 0)
        startActivity(intent)
        finish()
    }
}
