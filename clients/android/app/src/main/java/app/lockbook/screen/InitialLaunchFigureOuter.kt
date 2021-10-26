package app.lockbook.screen

import android.content.Intent
import android.os.Bundle
import android.view.View
import androidx.appcompat.app.AppCompatActivity
import androidx.preference.PreferenceManager
import app.lockbook.App.Companion.config
import app.lockbook.R
import app.lockbook.databinding.SplashScreenBinding
import app.lockbook.model.AlertModel
import app.lockbook.model.BiometricModel
import app.lockbook.model.CoreModel
import app.lockbook.model.VerificationItem
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.*
import timber.log.Timber
import java.lang.ref.WeakReference

class InitialLaunchFigureOuter : AppCompatActivity() {
    private var _binding: SplashScreenBinding? = null
    // This property is only valid between onCreateView and
    // onDestroyView.
    private val binding get() = _binding!!

    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)

    private val alertModel by lazy {
        AlertModel(WeakReference(this))
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        _binding = SplashScreenBinding.inflate(layoutInflater)
        setContentView(binding.root)
        Timber.plant(Timber.DebugTree())

        handleDBState()
    }

    private fun handleDBState() {
        when (val getDBStateResult = CoreModel.getDBState(config)) {
            is Ok -> {
                when (getDBStateResult.value) {
                    State.Empty -> {
                        startActivity(Intent(this, WelcomeActivity::class.java))
                        finish()
                    }
                    State.ReadyToUse -> startFromExistingAccount()
                    State.MigrationRequired -> {
                        alertModel.notify(getString(R.string.initial_figure_outer_migrate_data))
                        binding.migrateProgressBar.visibility = View.VISIBLE
                        migrateDB()
                    }
                    State.StateRequiresClearing -> {
                        Timber.e("DB state requires cleaning!")
                        alertModel.notify(getString(R.string.state_requires_cleaning))
                    }
                }
            }
            is Err -> alertModel.notifyError(getDBStateResult.error.toLbError(resources))
        }
    }

    private fun migrateDB() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                when (val migrateDBResult = CoreModel.migrateDB(Config(filesDir.absolutePath))) {
                    is Ok -> {
                        withContext(Dispatchers.Main) {
                            binding.migrateProgressBar.visibility = View.GONE
                            alertModel.notify(
                                getString(R.string.initial_figure_outer_finished_upgrading_data),
                                ::startFromExistingAccount
                            )
                        }
                    }
                    is Err -> alertModel.notifyError(migrateDBResult.error.toLbError(resources), ::finish)
                }.exhaustive
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
