package app.lockbook.screen

import android.os.Bundle
import android.text.style.ForegroundColorSpan
import android.view.*
import androidx.core.content.res.ResourcesCompat
import androidx.core.view.isVisible
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.fragment.app.viewModels
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import app.lockbook.R
import app.lockbook.databinding.FragmentTextEditorBinding
import app.lockbook.model.AlertModel
import app.lockbook.model.DetailsScreen
import app.lockbook.model.EditTextModel
import app.lockbook.model.StateViewModel
import app.lockbook.model.TextEditorViewModel
import io.noties.markwon.Markwon
import io.noties.markwon.editor.MarkwonEditor
import io.noties.markwon.editor.MarkwonEditorTextWatcher
import java.lang.ref.WeakReference
import java.util.concurrent.Executors

class TextEditorFragment : Fragment() {
    private var _binding: FragmentTextEditorBinding? = null
    private val binding get() = _binding!!

    private val textEditorToolbar get() = binding.textEditorToolbar
    private val textField get() = binding.textEditorTextField

    private val model: TextEditorViewModel by viewModels(
        factoryProducer = {
            object : ViewModelProvider.Factory {
                override fun <T : ViewModel> create(modelClass: Class<T>): T {
                    if (modelClass.isAssignableFrom(TextEditorViewModel::class.java))
                        return TextEditorViewModel(requireActivity().application, activityModel.detailsScreen!!.fileMetadata.id, (activityModel.detailsScreen as DetailsScreen.TextEditor).text) as T
                    throw IllegalArgumentException("Unknown ViewModel class")
                }
            }
        }
    )

    private val undoRedo by lazy {
        EditTextModel(binding.textEditorTextField, model, ::isUndoEnabled, ::isRedoEnabled)
    }

    private val activityModel: StateViewModel by activityViewModels()

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()))
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = FragmentTextEditorBinding.inflate(inflater, container, false)
        val name = (activityModel.detailsScreen as DetailsScreen.TextEditor).fileMetadata.decryptedName

        textEditorToolbar.title = name
        textEditorToolbar.inflateMenu(R.menu.menu_text_editor)
        textEditorToolbar.setOnMenuItemClickListener { item ->
            when (item.itemId) {
                R.id.menu_text_editor_view_md -> viewMarkdown()
                R.id.menu_text_editor_redo -> {
                    undoRedo.redo()
                }
                R.id.menu_text_editor_undo -> {
                    undoRedo.undo()
                }
            }

            true
        }

        undoRedo.updateUndoRedoButtons()

        model.content.observe(
            viewLifecycleOwner
        ) { content ->
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

                binding.markdownToolbar.visibility = View.VISIBLE
                textEditorToolbar.menu?.findItem(R.id.menu_text_editor_view_md)?.isVisible = true

                textField.addTextChangedListener(
                    MarkwonEditorTextWatcher.withPreRender(
                        markdownEditor,
                        Executors.newCachedThreadPool(),
                        textField
                    )
                )
            }

            textField.setText(content)
            undoRedo.addTextChangeListener()
        }

        model.notifyError.observe(
            viewLifecycleOwner
        ) { error ->
            alertModel.notifyError(error)
        }

        setMarkdownButtonListeners()

        return binding.root
    }

    private fun setMarkdownButtonListeners() {
        binding.menuMarkdownTitle.setOnClickListener {
            textField.text.replace(textField.selectionStart, textField.selectionStart, "# ")
        }

        binding.menuMarkdownBold.setOnClickListener {
            val selectionStart = textField.selectionStart
            val selectionEnd = textField.selectionEnd
            if (selectionStart == selectionEnd) {
                textField.text.replace(selectionStart, selectionStart, "****")
                textField.setSelection(selectionStart + 2)
            } else {
                textField.text.replace(selectionStart, selectionStart, "**")
                val newSelectionEnd = selectionEnd + 2
                textField.text.replace(newSelectionEnd, newSelectionEnd, "**")
                textField.setSelection(newSelectionEnd)
            }
        }

        binding.menuMarkdownItalics.setOnClickListener {
            val selectionStart = textField.selectionStart
            val selectionEnd = textField.selectionEnd
            if (selectionStart == selectionEnd) {
                textField.text.replace(selectionStart, selectionStart, "__")
                textField.setSelection(selectionStart + 1)
            } else {
                textField.text.replace(selectionStart, selectionStart, "_")
                val newSelectionEnd = selectionEnd + 1
                textField.text.replace(newSelectionEnd, newSelectionEnd, "_")
                textField.setSelection(newSelectionEnd)
            }
        }

        binding.menuMarkdownImage.setOnClickListener {
            val selectionStart = textField.selectionStart
            textField.text.replace(selectionStart, textField.selectionEnd, "![]()")
            textField.setSelection(selectionStart + 2)
        }

        binding.menuMarkdownLink.setOnClickListener {
            val selectionStart = textField.selectionStart
            textField.text.replace(selectionStart, textField.selectionEnd, "[]()")
            textField.setSelection(selectionStart + 1)
        }

        binding.menuMarkdownCode.setOnClickListener {
            val selectionStart = textField.selectionStart
            val selectionEnd = textField.selectionEnd
            if (selectionStart == selectionEnd) {
                textField.text.replace(selectionStart, selectionStart, "``")
                textField.setSelection(selectionStart + 1)
            } else {
                textField.text.replace(selectionStart, selectionStart, "`")
                val newSelectionEnd = selectionEnd + 1
                textField.text.replace(newSelectionEnd, newSelectionEnd, "`")
                textField.setSelection(newSelectionEnd)
            }
        }
    }

    private fun viewMarkdown() {
        if (binding.textEditorScroller.visibility == View.VISIBLE) {
            val markdown = Markwon.create(requireContext())
            markdown.setMarkdown(binding.markdownViewer, textField.text.toString())
            textEditorToolbar.menu?.findItem(R.id.menu_text_editor_undo)?.isVisible = false
            textEditorToolbar.menu?.findItem(R.id.menu_text_editor_redo)?.isVisible = false
            binding.markdownToolbar.isVisible = false
            binding.textEditorScroller.visibility = View.GONE
            binding.markdownViewerScroller.visibility = View.VISIBLE
        } else {
            binding.markdownViewerScroller.visibility = View.GONE
            binding.textEditorScroller.visibility = View.VISIBLE
            binding.markdownToolbar.isVisible = true
            textEditorToolbar.menu?.findItem(R.id.menu_text_editor_undo)?.isVisible = true
            textEditorToolbar.menu?.findItem(R.id.menu_text_editor_redo)?.isVisible = true
        }
    }

    private fun isUndoEnabled(canUndo: Boolean) {
        textEditorToolbar.menu!!.findItem(R.id.menu_text_editor_undo)!!.isEnabled = canUndo
    }

    private fun isRedoEnabled(canRedo: Boolean) {
        textEditorToolbar.menu!!.findItem(R.id.menu_text_editor_redo)!!.isEnabled = canRedo
    }

    fun saveOnExit() {
        if (model.editHistory.isDirty) {
            model.lastEdit = System.currentTimeMillis()
            activityModel.saveTextOnExit(model.id, textField.text.toString())
        }
    }
}

class CustomPunctuationSpan internal constructor(color: Int) : ForegroundColorSpan(color)
