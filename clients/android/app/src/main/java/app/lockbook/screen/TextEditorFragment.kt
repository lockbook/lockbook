package app.lockbook.screen

import android.os.Bundle
import android.text.style.ForegroundColorSpan
import android.view.*
import androidx.core.content.res.ResourcesCompat
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.fragment.app.viewModels
import app.lockbook.R
import app.lockbook.databinding.FragmentTextEditorBinding
import app.lockbook.model.AlertModel
import app.lockbook.model.StateViewModel
import app.lockbook.model.TextEditorViewModel
import io.noties.markwon.Markwon
import io.noties.markwon.editor.MarkwonEditor
import io.noties.markwon.editor.MarkwonEditorTextWatcher
import java.lang.ref.WeakReference
import java.util.concurrent.Executors

class TextEditorFragment: Fragment() {
    private var _binding: FragmentTextEditorBinding? = null
    private val binding get() = _binding!!

    private val textEditorToolbar get() = binding.textEditorToolbar
    private val textField get() = binding.textEditorTextField

    private val model: TextEditorViewModel by viewModels()
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

        val id = arguments?.getString("id")

        if (id == null) {
            alertModel.notifyBasicError()
            return binding.root
        }

        model.content.observe(
            viewLifecycleOwner,
            { content ->
                val name = activityModel.openedFile!!.name
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
                textField.addTextChangedListener(model)
            }
        )

        model.notifyError.observe(
            viewLifecycleOwner,
            { error ->
                alertModel.notifyError(error)
            }
        )

        return super.onCreateView(inflater, container, savedInstanceState)
    }
}

class CustomPunctuationSpan internal constructor(color: Int) : ForegroundColorSpan(color)
