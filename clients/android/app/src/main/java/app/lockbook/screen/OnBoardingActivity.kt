package app.lockbook.screen

import android.annotation.SuppressLint
import android.app.Application
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.graphics.Typeface
import android.os.Build
import android.os.Bundle
import android.text.SpannableStringBuilder
import android.text.style.ForegroundColorSpan
import android.text.style.StyleSpan
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.autofill.AutofillManager
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import androidx.activity.addCallback
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.ContextCompat
import androidx.core.widget.doOnTextChanged
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.lifecycleScope
import app.lockbook.R
import app.lockbook.databinding.ActivityOnBoardingBinding
import app.lockbook.databinding.FragmentOnBoardingCopyKeyBinding
import app.lockbook.databinding.FragmentOnBoardingCreateAccountBinding
import app.lockbook.databinding.FragmentOnBoardingImportAccountBinding
import app.lockbook.databinding.FragmentOnBoardingWelcomeBinding
import app.lockbook.util.getString
import com.google.android.material.button.MaterialButton
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import com.google.android.material.textfield.TextInputEditText
import com.journeyapps.barcodescanner.CaptureActivity
import com.journeyapps.barcodescanner.ScanContract
import com.journeyapps.barcodescanner.ScanIntentResult
import com.journeyapps.barcodescanner.ScanOptions
import kotlinx.coroutines.*
import net.lockbook.Lb
import net.lockbook.LbError
import kotlin.getValue

class OnBoardingActivity : AppCompatActivity() {
    private var _binding: ActivityOnBoardingBinding? = null

    // This property is only valid between onCreateView and
    // onDestroyView.
    val binding get() = _binding!!

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
    }
}

class WelcomeFragment : Fragment() {
    private var _welcomeBinding: FragmentOnBoardingWelcomeBinding? = null

    private val welcomeBinding get() = _welcomeBinding!!

    private val model: OnboardingViewModel by activityViewModels()

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _welcomeBinding = FragmentOnBoardingWelcomeBinding.inflate(inflater, container, false)

        welcomeBinding.loginButton.setOnClickListener {
            parentFragmentManager.beginTransaction()
                .replace(R.id.on_boarding_fragment_container, ImportFragment())
                .addToBackStack(null)
                .commit()
        }

        welcomeBinding.getStartedButton.setOnClickListener {
            parentFragmentManager.beginTransaction()
                .replace(R.id.on_boarding_fragment_container, CreateFragment())
                .addToBackStack(null)
                .commit()
        }

        model.apiUrl.observe(this.viewLifecycleOwner) {
            welcomeBinding.onBoardingServerButton.text = it.substringAfter("://")
        }

        welcomeBinding.onBoardingServerButton.setOnClickListener {
            showServerDialog()
        }

        return welcomeBinding.root
    }

    private fun showServerDialog() {
        val dialogView = layoutInflater.inflate(R.layout.dialog_pick_server_url, null)
        val editText = dialogView.findViewById<TextInputEditText>(R.id.server_url_edit_text)
        val defaultUrl = getString(R.string.default_api_url)

        // Pre-populate with the current value (skip if it's the default label)
        val current = model.apiUrl.value
        if (current != defaultUrl) {
            editText.setText(current)
        }

        val dialog = MaterialAlertDialogBuilder(this.requireContext())
            .setView(dialogView)
            .create()

        dialogView.findViewById<MaterialButton>(R.id.btn_reset_default).setOnClickListener {
            model._apiUrl.value = defaultUrl
            dialog.dismiss()
        }

        dialogView.findViewById<MaterialButton>(R.id.btn_submit).setOnClickListener {
            val input = editText.text?.toString()?.trim()
            if (!input.isNullOrEmpty()) {
                model._apiUrl.value = input
            }
            dialog.dismiss()
        }

        dialog.show()
    }
}

class CreateFragment : Fragment() {
    private var _createBinding: FragmentOnBoardingCreateAccountBinding? = null

    // This property is only valid between onCreateView and
    // onDestroyView.
    private val createBinding get() = _createBinding!!

    private val uiScope = CoroutineScope(Dispatchers.Main + Job())
    private val model: OnboardingViewModel by activityViewModels()

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

        // open the virtual keyboard
        val imm = requireContext().getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
        imm.showSoftInput(createBinding.onBoardingCreateAccountInput, InputMethodManager.SHOW_IMPLICIT)
    }

    private fun createAccount(username: String) {

        uiScope.launch {
            try {
                Lb.createAccount(username, model.apiUrl.value, true)

                withContext(Dispatchers.Main) {
                    parentFragmentManager.beginTransaction()
                        .replace(R.id.on_boarding_fragment_container, CopyKeyFragment())
                        .commit()
                }
            } catch (err: LbError) {
                withContext(Dispatchers.Main) {
                    createBinding.onBoardingCreateAccountInputHolder.error = err.msg
                }
            } catch (err: Error) {
                withContext(Dispatchers.Main) {
                    createBinding.onBoardingCreateAccountInputHolder.error = err.message
                }
            }
        }
    }
}

class CopyKeyFragment : Fragment() {
    private var _copyKeyBinding: FragmentOnBoardingCopyKeyBinding? = null

    private val copyKeyBinding get() = _copyKeyBinding!!

    private lateinit var phrase: String

    private val model: OnboardingViewModel by activityViewModels()

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _copyKeyBinding = FragmentOnBoardingCopyKeyBinding.inflate(inflater, container, false)

        viewLifecycleOwner.lifecycleScope.launch {
            phrase = Lb.exportAccountPhrase()
        }

        copyKeyBinding.pledgeCheckbox.setOnCheckedChangeListener { _, isChecked ->
            copyKeyBinding.nextButton.isEnabled = isChecked
        }

        copyKeyBinding.copyKeyButton.setOnClickListener {
            val clipboard = requireContext().getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
            val clip = ClipData.newPlainText("account phrase", phrase)
            clipboard.setPrimaryClip(clip)
        }

        copyKeyBinding.nextButton.setOnClickListener {
            startActivity(Intent(context, MainScreenActivity::class.java))
        }

        val words = phrase.split(" ")

        copyKeyBinding.keyFirstHalf.text = createColoredNumberedList(words.take(12), 1)
        copyKeyBinding.keySecondHalf.text = createColoredNumberedList(words.drop(12), 13)

        // prevent back button, you don't want to go back to the create
        // screen after creating an account
        requireActivity().onBackPressedDispatcher.addCallback {}

        return copyKeyBinding.root
    }

    private fun createColoredNumberedList(words: List<String>, startIndex: Int = 1): SpannableStringBuilder {
        val builder = SpannableStringBuilder()
        val numberColor = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            ContextCompat.getColor(requireContext(), android.R.color.system_accent1_300)
        } else {
            ContextCompat.getColor(requireContext(), R.color.md_theme_primary)
        }

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

    private val model: OnboardingViewModel by activityViewModels()

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

                importAccount(importBinding.onBoardingImportAccountInput.text.toString(), true)
            }

            true
        }

        importBinding.onBoardingImportAccountInput.doOnTextChanged { text, _, _, _ ->
            importAccount(text.toString(), false)
            importBinding.onBoardingImportAccountHolder.error = ""
        }

        importBinding.onBoardingImportAccountInput.setOnFocusChangeListener { _, hasFocus ->
            if (hasFocus && Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
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
            importAccount(importBinding.onBoardingImportAccountInput.text.toString(), true)
        }

        return importBinding.root
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)

        importBinding.onBoardingImportAccountInput.requestFocus()

        val imm = requireContext().getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
        imm.showSoftInput(importBinding.onBoardingImportAccountInput, InputMethodManager.SHOW_IMPLICIT)
    }

    private fun forceAutoFillCheckSave() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            requireContext()
                .getSystemService(AutofillManager::class.java)
                .commit()
        }
    }

    private fun importAccount(account: String, surfaceError: Boolean) {
        val onBoardingActivity = (requireActivity() as OnBoardingActivity)

        uiScope.launch {
            try {
                Lb.importAccount(account, model.apiUrl.value)
                onBoardingActivity.startActivity(Intent(context, ImportAccountActivity::class.java))
                onBoardingActivity.finishAffinity()
            } catch (err: LbError) {
                if (surfaceError) {
                    withContext(Dispatchers.Main) {
                        importBinding.onBoardingImportAccountHolder.error = err.msg
                    }
                }
            } catch (err: Error) {
                if (surfaceError) {
                    withContext(Dispatchers.Main) {
                        importBinding.onBoardingImportAccountHolder.error = err.message
                    }
                }
            }
        }
    }
}

class OnboardingViewModel(application: Application) : AndroidViewModel(application) {

    val _apiUrl = MutableLiveData<String>(getString(R.string.default_api_url))
    val apiUrl: LiveData<String>
        get() = _apiUrl
}

class CaptureActivityAutoRotate : CaptureActivity()
