package app.lockbook.screen

import android.annotation.SuppressLint
import android.app.Activity
import android.content.Intent
import android.os.Bundle
import android.text.method.LinkMovementMethod
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.EditorInfo
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.fragment.app.Fragment
import androidx.viewpager2.adapter.FragmentStateAdapter
import app.lockbook.App
import app.lockbook.R
import app.lockbook.databinding.ActivityMainBinding
import app.lockbook.databinding.FragmentWelcomeCreateAccountBinding
import app.lockbook.databinding.FragmentWelcomeImportAccountBinding
import app.lockbook.model.AlertModel
import app.lockbook.model.CoreModel
import app.lockbook.util.exhaustive
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.google.android.material.tabs.TabLayoutMediator
import com.google.zxing.integration.android.IntentIntegrator
import kotlinx.coroutines.*
import java.lang.ref.WeakReference

class WelcomeActivity : AppCompatActivity() {
    private var _binding: ActivityMainBinding? = null

    // This property is only valid between onCreateView and
    // onDestroyView.
    val binding get() = _binding!!

    val alertModel by lazy {
        AlertModel(WeakReference(this))
    }

    @SuppressLint("SetTextI18n")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        _binding = ActivityMainBinding.inflate(layoutInflater)
        setContentView(binding.root)

        binding.welcomeCreateImport.adapter = CreateImportFragmentAdapter(this)
        binding.welcomeLearnMore.movementMethod = LinkMovementMethod.getInstance()

        TabLayoutMediator(
            binding.welcomeStateSwitcher,
            binding.welcomeCreateImport
        ) { tabLayout, position ->
            tabLayout.text = if (position == 0) {
                resources.getText(R.string.welcome_create)
            } else {
                resources.getText(R.string.welcome_import)
            }
        }.attach()
    }

    inner class CreateImportFragmentAdapter(activity: AppCompatActivity) :
        FragmentStateAdapter(activity) {
        override fun getItemCount(): Int = 2

        override fun createFragment(position: Int): Fragment {
            return if (position == 0) {
                CreateFragment()
            } else {
                ImportFragment()
            }
        }
    }
}

class ImportFragment : Fragment() {
    private var _importBinding: FragmentWelcomeImportAccountBinding? = null

    // This property is only valid between onCreateView and
    // onDestroyView.
    private val importBinding get() = _importBinding!!

    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)

    private var onQRCodeResult =
        registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
            if (result.resultCode == Activity.RESULT_OK) {
                val intentResult =
                    IntentIntegrator.parseActivityResult(result.resultCode, result.data)

                intentResult?.contents?.let { account ->
                    importAccount(account)
                }
            }
        }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _importBinding = FragmentWelcomeImportAccountBinding.inflate(inflater, container, false)

        importBinding.welcomeAccountString.setOnEditorActionListener { _, actionId, _ ->
            if (actionId == EditorInfo.IME_ACTION_DONE) {
                importAccount(importBinding.welcomeAccountString.text.toString())
            }

            true
        }

        importBinding.newAccountQrImportButton.setOnClickListener {
            onQRCodeResult.launch(
                IntentIntegrator(requireActivity()).setOrientationLocked(false).createScanIntent()
            )
        }

        importBinding.welcomeSubmit.setOnClickListener {
            importAccount(importBinding.welcomeAccountString.text.toString())
        }

        return importBinding.root
    }

    private fun importAccount(account: String) {
        val welcomeActivity = (requireActivity() as WelcomeActivity)

        welcomeActivity.binding.welcomeProgressBar.visibility = View.VISIBLE

        uiScope.launch {
            when (val importAccountResult = CoreModel.importAccount(App.config, account)) {
                is Ok -> {
                    val intent = Intent(context, ImportAccountActivity::class.java)

                    welcomeActivity.startActivity(intent)
                    welcomeActivity.finishAffinity()
                }
                is Err -> {
                    withContext(Dispatchers.Main) {
                        welcomeActivity.binding.welcomeProgressBar.visibility = View.GONE
                    }

                    welcomeActivity.alertModel.notifyError(
                        importAccountResult.error.toLbError(
                            resources
                        )
                    )
                }
            }.exhaustive
        }
    }
}

class CreateFragment : Fragment() {
    private var _createBinding: FragmentWelcomeCreateAccountBinding? = null

    // This property is only valid between onCreateView and
    // onDestroyView.
    private val createBinding get() = _createBinding!!

    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _createBinding = FragmentWelcomeCreateAccountBinding.inflate(inflater, container, false)

        createBinding.welcomeAccountUsername.setOnEditorActionListener { _, actionId, _ ->
            if (actionId == EditorInfo.IME_ACTION_DONE) {
                createAccount(createBinding.welcomeAccountUsername.text.toString())
            }

            true
        }

        createBinding.welcomeSubmit.setOnClickListener {
            createAccount(createBinding.welcomeAccountUsername.text.toString())
        }

        return createBinding.root
    }

    private fun createAccount(username: String) {
        val welcomeActivity = (requireActivity() as WelcomeActivity)

        welcomeActivity.binding.welcomeProgressBar.visibility = View.VISIBLE

        uiScope.launch {
            when (val createAccountResult = CoreModel.generateAccount(App.config, username)) {
                is Ok -> {
                    val intent = Intent(context, MainScreenActivity::class.java)
                    intent.putExtra(IS_THIS_A_NEW_ACCOUNT, true)

                    welcomeActivity.startActivity(intent)
                    welcomeActivity.finishAffinity()
                }
                is Err -> {
                    withContext(Dispatchers.Main) {
                        welcomeActivity.binding.welcomeProgressBar.visibility = View.GONE
                    }

                    welcomeActivity.alertModel.notifyError(
                        createAccountResult.error.toLbError(
                            resources
                        )
                    )
                }
            }.exhaustive
        }
    }
}
