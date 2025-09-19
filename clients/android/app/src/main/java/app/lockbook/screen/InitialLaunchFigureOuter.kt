package app.lockbook.screen

import android.content.Intent
import android.os.Bundle
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.edit
import androidx.preference.PreferenceManager
import app.lockbook.R
import app.lockbook.databinding.SplashScreenBinding
import app.lockbook.model.AlertModel
import app.lockbook.model.BiometricModel
import app.lockbook.model.VerificationItem
import net.lockbook.Lb
import net.lockbook.LbError
import net.lockbook.LbError.LbEC
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
        try {
            Lb.getAccount()
            startFromExistingAccount()
        } catch (err: LbError) {
            if (err.kind == LbEC.AccountNonexistent) {
                startActivity(Intent(this, OnBoardingActivity::class.java))
                finish()
            } else {
                alertModel.notifyError(err)
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
            pref.edit {
                putString(biometricKey, biometricNoneValue)
            }
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
