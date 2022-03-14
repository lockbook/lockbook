package app.lockbook.screen

import android.annotation.SuppressLint
import android.app.Activity
import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.text.method.LinkMovementMethod
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.autofill.AutofillManager
import android.view.inputmethod.EditorInfo
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.fragment.app.Fragment
import androidx.viewpager2.adapter.FragmentStateAdapter
import app.lockbook.App
import app.lockbook.R
import app.lockbook.databinding.ActivityOnBoardingBinding
import app.lockbook.databinding.FragmentOnBoardingCreateAccountBinding
import app.lockbook.databinding.FragmentOnBoardingImportAccountBinding
import app.lockbook.model.AlertModel
import app.lockbook.model.CoreModel
import app.lockbook.util.exhaustive
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.google.android.material.tabs.TabLayoutMediator
import com.google.zxing.integration.android.IntentIntegrator
import kotlinx.coroutines.*
import java.lang.ref.WeakReference

class OnBoardingActvity : AppCompatActivity() {
    private var _binding: ActivityOnBoardingBinding? = null

    // This property is only valid between onCreateView and
    // onDestroyView.
    val binding get() = _binding!!

    val alertModel by lazy {
        AlertModel(WeakReference(this))
    }

    @SuppressLint("SetTextI18n")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        _binding = ActivityOnBoardingBinding.inflate(layoutInflater)
        setContentView(binding.root)

        binding.onBoardingCreateImportViewPager.adapter = CreateImportFragmentAdapter(this)
        binding.onBoardingLearnMore.movementMethod = LinkMovementMethod.getInstance()

        TabLayoutMediator(
            binding.onBoardingSwitcher,
            binding.onBoardingCreateImportViewPager
        ) { tabLayout, position ->
            tabLayout.text = if (position == 0) {
                resources.getText(R.string.on_boarding_create)
            } else {
                resources.getText(R.string.on_boarding_import)
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
    private var _importBinding: FragmentOnBoardingImportAccountBinding? = null

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
                    importBinding.onBoardingAccountString.setText(account)
                    forceAutoFillCheckSave()

                    importBinding.onBoardingImportSubmit.performClick()
                }
            }
        }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _importBinding = FragmentOnBoardingImportAccountBinding.inflate(inflater, container, false)

        importBinding.onBoardingAccountString.setOnEditorActionListener { _, actionId, _ ->
            if (actionId == EditorInfo.IME_ACTION_DONE) {
                forceAutoFillCheckSave()

                importAccount(importBinding.onBoardingAccountString.text.toString())
            }

            true
        }

        importBinding.onBoardingAccountString.setOnFocusChangeListener { _, hasFocus ->
            if (Build.VERSION.SDK_INT > Build.VERSION_CODES.N_MR1 && hasFocus) {
                requireContext()
                    .getSystemService(AutofillManager::class.java)
                    .requestAutofill(importBinding.onBoardingAccountString)
            }
        }

        importBinding.onBoardingQrCodeImport.setOnClickListener {
            onQRCodeResult.launch(
                IntentIntegrator(requireActivity()).setOrientationLocked(false).createScanIntent()
            )
        }

        importBinding.onBoardingImportSubmit.setOnClickListener {
            forceAutoFillCheckSave()
            importAccount(importBinding.onBoardingAccountString.text.toString())
        }

        return importBinding.root
    }

    private fun forceAutoFillCheckSave() {
        if (Build.VERSION.SDK_INT > Build.VERSION_CODES.N_MR1) {
            requireContext()
                .getSystemService(AutofillManager::class.java)
                .commit()
        }
    }

    private fun importAccount(account: String) {
        val onBoardingActivity = (requireActivity() as OnBoardingActvity)

        onBoardingActivity.binding.onBoardingProgressBar.visibility = View.VISIBLE

        uiScope.launch {
            when (val importAccountResult = CoreModel.importAccount(App.config, account)) {
                is Ok -> {
                    onBoardingActivity.startActivity(Intent(context, ImportAccountActivity::class.java))
                    onBoardingActivity.finishAffinity()
                }
                is Err -> {
                    withContext(Dispatchers.Main) {
                        onBoardingActivity.binding.onBoardingProgressBar.visibility = View.GONE
                        importBinding.onBoardingAccountString.error = importAccountResult.error.toLbError(
                            resources
                        ).msg
                    }
                }
            }.exhaustive
        }
    }
}

class CreateFragment : Fragment() {
    private var _createBinding: FragmentOnBoardingCreateAccountBinding? = null

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
        _createBinding = FragmentOnBoardingCreateAccountBinding.inflate(inflater, container, false)

        createBinding.onBoardingUsername.setOnEditorActionListener { _, actionId, _ ->
            if (actionId == EditorInfo.IME_ACTION_DONE) {
                createAccount(createBinding.onBoardingUsername.text.toString())
            }

            true
        }

        createBinding.onBoardingCreateSubmit.setOnClickListener {
            createAccount(createBinding.onBoardingUsername.text.toString())
        }

        return createBinding.root
    }

    private fun createAccount(username: String) {
        val onBoardingActivity = (requireActivity() as OnBoardingActvity)

        onBoardingActivity.binding.onBoardingProgressBar.visibility = View.VISIBLE

        uiScope.launch {
            when (val createAccountResult = CoreModel.generateAccount(App.config, username)) {
                is Ok -> {
                    val intent = Intent(context, MainScreenActivity::class.java)
                    intent.putExtra(IS_THIS_A_NEW_ACCOUNT, true)

                    onBoardingActivity.startActivity(intent)
                    onBoardingActivity.finishAffinity()
                }
                is Err -> {
                    withContext(Dispatchers.Main) {
                        onBoardingActivity.binding.onBoardingProgressBar.visibility = View.GONE
                        createBinding.onBoardingUsername.error = createAccountResult.error.toLbError(
                            resources
                        ).msg
                    }
                }
            }.exhaustive
        }
    }
}
