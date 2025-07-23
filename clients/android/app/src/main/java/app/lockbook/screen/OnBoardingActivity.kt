package app.lockbook.screen

import android.annotation.SuppressLint
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.graphics.Typeface
import android.os.Build
import android.os.Bundle
import android.text.SpannableStringBuilder
import android.text.method.LinkMovementMethod
import android.text.style.ForegroundColorSpan
import android.text.style.StyleSpan
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.autofill.AutofillManager
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import androidx.appcompat.app.AppCompatActivity
import androidx.core.app.ActivityCompat.finishAffinity
import androidx.core.content.ContextCompat
import androidx.core.content.ContextCompat.getSystemService
import androidx.core.view.WindowCompat
import androidx.fragment.app.Fragment
import androidx.fragment.app.FragmentManager
import androidx.lifecycle.lifecycleScope
import androidx.viewpager2.adapter.FragmentStateAdapter
import app.lockbook.R
import app.lockbook.databinding.ActivityOnBoardingBinding
import app.lockbook.databinding.FragmentOnBoardingCopyKeyBinding
import app.lockbook.databinding.FragmentOnBoardingWelcomeBinding
import app.lockbook.databinding.FragmentOnBoardingCreateAccountBinding
import app.lockbook.databinding.FragmentOnBoardingImportAccountBinding
import app.lockbook.model.AlertModel
import app.lockbook.util.getApp
import com.google.android.material.tabs.TabLayoutMediator
import com.journeyapps.barcodescanner.CaptureActivity
import com.journeyapps.barcodescanner.ScanContract
import com.journeyapps.barcodescanner.ScanIntentResult
import com.journeyapps.barcodescanner.ScanOptions
import kotlinx.coroutines.*
import net.lockbook.Lb
import net.lockbook.LbError
import java.lang.ref.WeakReference

class OnBoardingActivity : AppCompatActivity() {
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

        if (savedInstanceState == null) {
            supportFragmentManager.beginTransaction()
                .replace(R.id.on_boarding_fragment_container, WelcomeFragment())
                .commit()
        }
//        binding.onBoardingCreateImportViewPager.adapter = CreateImportFragmentAdapter(this)

//        TabLayoutMediator(
//            binding.onBoardingSwitcher,
//            binding.onBoardingCreateImportViewPager
//        ) { tabLayout, position ->
//            tabLayout.text = if (position == 0) {
//                resources.getText(R.string.on_boarding_create)
//            } else {
//                resources.getText(R.string.on_boarding_import)
//            }
//        }.attach()
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


class WelcomeFragment :  Fragment(){
    private var _welcomeBinding: FragmentOnBoardingWelcomeBinding? = null

    private val welcomeBinding get() = _welcomeBinding!!

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _welcomeBinding = FragmentOnBoardingWelcomeBinding.inflate(inflater, container, false)

        welcomeBinding.loginButton.setOnClickListener{
            parentFragmentManager.beginTransaction()
                .replace(R.id.on_boarding_fragment_container, ImportFragment())
                .addToBackStack(null)
                .commit()
        }

        welcomeBinding.getStartedButton.setOnClickListener{
            parentFragmentManager.beginTransaction()
                .replace(R.id.on_boarding_fragment_container, CreateFragment())
                .addToBackStack(null)
                .commit()
        }

        return welcomeBinding.root
    }
}

class CopyKeyFragment : Fragment() {
    private var _copyKeyBinding: FragmentOnBoardingCopyKeyBinding? = null

    private val copyKeyBinding get() = _copyKeyBinding!!

    private lateinit var phrase : String

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _copyKeyBinding = FragmentOnBoardingCopyKeyBinding.inflate(inflater, container, false)

        viewLifecycleOwner.lifecycleScope.launch {
            phrase = Lb.exportAccountPhrase()
        }

        copyKeyBinding.pledgeCheckbox.setOnCheckedChangeListener { buttonView, isChecked ->
            copyKeyBinding.nextButton.isEnabled = isChecked
        }

        copyKeyBinding.copyKeyButton.setOnClickListener {
            val clipboard  = requireContext().getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
            val clip = ClipData.newPlainText("account phrase", phrase)
            clipboard.setPrimaryClip(clip)
        }

        copyKeyBinding.nextButton.setOnClickListener {
            startActivity(Intent(context, MainScreenActivity::class.java))
        }


        val words = phrase.split(" ")

        copyKeyBinding.keyFirstHalf.text = createColoredNumberedList(words.take(12), 1)
        copyKeyBinding.keySecondHalf.text = createColoredNumberedList(words.drop(12), 13)

        return copyKeyBinding.root
    }

    private fun createColoredNumberedList(words: List<String>, startIndex: Int = 1): SpannableStringBuilder {
        val builder = SpannableStringBuilder()
        val numberColor = ContextCompat.getColor(requireContext(), R.color.md_theme_secondary)

        words.forEachIndexed { i, word ->
            val numberText = "${startIndex + i}. "
            val wordText = if (i < words.size - 1) "$word\n" else word

            // Add colored number
            val numberStart = builder.length
            builder.append(numberText)
            builder.setSpan(StyleSpan(Typeface.BOLD), numberStart, builder.length, 0)
            builder.setSpan(
                ForegroundColorSpan(numberColor),
                numberStart,
                builder.length,
                0
            )

            // Add regular text
            builder.append(wordText)
        }

        return builder
    }
}

class ImportFragment : Fragment() {
    private var _importBinding: FragmentOnBoardingImportAccountBinding? = null

    // This property is only valid between onCreateView and
    // onDestroyView.
    private val importBinding get() = _importBinding!!

    private val uiScope = CoroutineScope(Dispatchers.Main + Job())

    private var onQRCodeResult =
        registerForActivityResult(
            ScanContract()
        ) { result: ScanIntentResult ->
            if (result.contents != null) {
                importBinding.onBoardingImportAccountInput.setText(result.contents)
                forceAutoFillCheckSave()

                importBinding.onBoardingImportSubmit.performClick()
            }
        }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _importBinding = FragmentOnBoardingImportAccountBinding.inflate(inflater, container, false)

        importBinding.onBoardingImportAccountInput.setOnEditorActionListener { _, actionId, _ ->
            if (actionId == EditorInfo.IME_ACTION_DONE) {
                forceAutoFillCheckSave()

                importAccount(importBinding.onBoardingImportAccountInput.text.toString())
            }

            true
        }

        importBinding.onBoardingImportAccountInput.setOnFocusChangeListener { _, hasFocus ->
            if (Build.VERSION.SDK_INT > Build.VERSION_CODES.N_MR1 && hasFocus) {
                requireContext()
                    .getSystemService(AutofillManager::class.java)
                    .requestAutofill(importBinding.onBoardingImportAccountInput)
            }
        }

        importBinding.onBoardingQrCodeImport.setOnClickListener {

            onQRCodeResult.launch(
                ScanOptions()
                    .setOrientationLocked(false)
                    .setDesiredBarcodeFormats(ScanOptions.QR_CODE)
                    .setPrompt(getString(R.string.import_qr_scanner_prompt))
                    .setBarcodeImageEnabled(true)
                    .setCaptureActivity(CaptureActivityAutoRotate::class.java)
                    .setBeepEnabled(false)
            )
        }

        importBinding.onBoardingImportSubmit.setOnClickListener {
            forceAutoFillCheckSave()
            importAccount(importBinding.onBoardingImportAccountInput.text.toString())
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
        val onBoardingActivity = (requireActivity() as OnBoardingActivity)

//        onBoardingActivity.binding.onBoardingProgressBar.visibility = View.VISIBLE

        uiScope.launch {
            try {
                Lb.importAccount(account)
                onBoardingActivity.startActivity(Intent(context, ImportAccountActivity::class.java))
                onBoardingActivity.finishAffinity()
            } catch (err: LbError) {
                withContext(Dispatchers.Main) {
//                    onBoardingActivity.binding.onBoardingProgressBar.visibility = View.GONE
                    importBinding.onBoardingImportAccountHolder.error = err.msg
                }
            }
        }
    }
}

class CreateFragment : Fragment() {
    private var _createBinding: FragmentOnBoardingCreateAccountBinding? = null

    // This property is only valid between onCreateView and
    // onDestroyView.
    private val createBinding get() = _createBinding!!

    private val uiScope = CoroutineScope(Dispatchers.Main + Job())

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _createBinding = FragmentOnBoardingCreateAccountBinding.inflate(inflater, container, false)



        createBinding.onBoardingCreateAccountInput.setOnEditorActionListener { _, actionId, _ ->
            if (actionId == EditorInfo.IME_ACTION_DONE) {
                createAccount(createBinding.onBoardingCreateAccountInput.text.toString())
            }

            true
        }

        createBinding.onBoardingCreateSubmit.setOnClickListener {
            createAccount(createBinding.onBoardingCreateAccountInput.text.toString())
        }

        return createBinding.root
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)

        createBinding.onBoardingCreateAccountInput.requestFocus()


        val imm = requireContext().getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
        imm.showSoftInput(createBinding.onBoardingCreateAccountInput, InputMethodManager.SHOW_IMPLICIT)
    }

    private fun createAccount(username: String) {
        val onBoardingActivity = (requireActivity() as OnBoardingActivity)

//        onBoardingActivity.binding.onBoardingProgressBar.visibility = View.VISIBLE

        uiScope.launch {
            try {
                Lb.createAccount(username, "http://192.168.1.72:8000", true)
                getApp().isNewAccount = true

                withContext(Dispatchers.Main) {
                    parentFragmentManager.beginTransaction()
                        .replace(R.id.on_boarding_fragment_container, CopyKeyFragment())
                        .commit()
                }
                //                onBoardingActivity.startActivity(Intent(context, MainScreenActivity::class.java))
                                onBoardingActivity.finishAffinity()
            } catch (err: LbError) {
                withContext(Dispatchers.Main) {
//                    onBoardingActivity.binding.onBoardingProgressBar.visibility = View.GONE
                    createBinding.onBoardingCreateAccountInputHolder.error = err.msg
                }
            }
        }
    }
}

class CaptureActivityAutoRotate : CaptureActivity()
