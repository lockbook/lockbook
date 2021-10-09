package app.lockbook.screen

import android.os.Bundle
import android.text.Editable
import android.text.TextWatcher
import android.text.style.ForegroundColorSpan
import android.view.*
import androidx.core.content.res.ResourcesCompat
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.fragment.app.viewModels
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import app.lockbook.R
import app.lockbook.databinding.FragmentTextEditorBinding
import app.lockbook.model.AlertModel
import app.lockbook.model.DetailsScreen
import app.lockbook.model.StateViewModel
import app.lockbook.model.TextEditorViewModel
import io.noties.markwon.Markwon
import io.noties.markwon.editor.MarkwonEditor
import io.noties.markwon.editor.MarkwonEditorTextWatcher
import timber.log.Timber
import java.lang.ref.WeakReference
import java.util.concurrent.Executors

class TextEditorFragment: Fragment(), TextWatcher {
    private var _binding: FragmentTextEditorBinding? = null
    private val binding get() = _binding!!

    private val textEditorToolbar get() = binding.textEditorToolbar
    private val textField get() = binding.textEditorTextField

    private val model: TextEditorViewModel by viewModels(factoryProducer = {
        object : ViewModelProvider.Factory {
            override fun <T : ViewModel?> create(modelClass: Class<T>): T {
                if (modelClass.isAssignableFrom(TextEditorViewModel::class.java))
                    return TextEditorViewModel(requireActivity().application, (activityModel.detailsScreen as DetailsScreen.TextEditor).fileMetadata.id) as T
                throw IllegalArgumentException("Unknown ViewModel class")
            }
        }
    })
    private val activityModel: StateViewModel by activityViewModels()

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()))
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View? {
        _binding = FragmentTextEditorBinding.inflate(inflater, container, false)

        model.content.observe(
            viewLifecycleOwner,
            { content ->
                val name = (activityModel.detailsScreen as DetailsScreen.TextEditor).fileMetadata.name

                if (name.endsWith(".md")) {
                    val markdownEditor = MarkwonEditor.builder(Markwon.create(requireContext()))
                        .punctuationSpan(
                            CustomPunctuationSpan::class.java
                        ) {
                            CustomPunctuationSpan(
                                ResourcesCompat.getColor(
                                    resources,
                                    R.color.blue,
                                    null
                                )
                            )
                        }
                        .build()

                    textField.addTextChangedListener(
                        MarkwonEditorTextWatcher.withPreRender(
                            markdownEditor,
                            Executors.newCachedThreadPool(),
                            textField
                        )
                    )
                }

                textField.setText(content)
                textField.addTextChangedListener(this)
            }
        )

        model.notifyError.observe(
            viewLifecycleOwner,
            { error ->
                alertModel.notifyError(error)
            }
        )

        return binding.root
    }

    override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {}

    override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {}

    override fun afterTextChanged(s: Editable?) {
        model.waitAndSaveContents(s?.toString() ?: "")
    }
}

class CustomPunctuationSpan internal constructor(color: Int) : ForegroundColorSpan(color)
