package app.lockbook.screen

import android.annotation.SuppressLint
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.EditText
import androidx.appcompat.widget.AppCompatEditText
import androidx.core.view.isVisible
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.fragment.app.viewModels
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import app.lockbook.R
import app.lockbook.databinding.FragmentTextEditorBinding
import app.lockbook.model.*
import app.lockbook.util.MarkdownEditor
import timber.log.Timber
import java.lang.ref.WeakReference

class TextEditorFragment : Fragment() {
    private var _binding: FragmentTextEditorBinding? = null
    private val binding get() = _binding!!

    private val textEditorToolbar get() = binding.textEditorToolbar

    private val model: TextEditorViewModel by viewModels(
        factoryProducer = {
            object : ViewModelProvider.Factory {
                override fun <T : ViewModel> create(modelClass: Class<T>): T {
                    val detailScreen = activityModel.detailScreen as DetailScreen.TextEditor

                    if (modelClass.isAssignableFrom(TextEditorViewModel::class.java))
                        return TextEditorViewModel(requireActivity().application, detailScreen.file, detailScreen.text) as T
                    throw IllegalArgumentException("Unknown ViewModel class")
                }
            }
        }
    )

    private val activityModel: StateViewModel by activityViewModels()

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()))
    }

    @SuppressLint("ClickableViewAccessibility")
    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = FragmentTextEditorBinding.inflate(inflater, container, false)
        val name = (activityModel.detailScreen as DetailScreen.TextEditor).file.name

//        AppCompatEditText()

        textEditorToolbar.title = name
        textEditorToolbar.setOnMenuItemClickListener { item ->
            when (item.itemId) {
                R.id.menu_text_editor_redo -> {
//                    undoRedo.redo()
                }

                R.id.menu_text_editor_undo -> {
//                    undoRedo.undo()
                }
            }

            true
        }
        textEditorToolbar.setNavigationOnClickListener {
            activityModel.launchDetailScreen(null)
        }

        model.updateContent.observe(
            viewLifecycleOwner
        ) {
            Timber.e("updating content in editor!")
            binding.textEditor.setText(model.currentContent, model)
        }

        model.notifyError.observe(
            viewLifecycleOwner
        ) { error ->
            alertModel.notifyError(error)
        }

        return binding.root
    }

    fun saveOnExit() {
        if (model.editHistory.isDirty) {
            model.lastEdit = System.currentTimeMillis()
            activityModel.saveTextOnExit(model.fileMetadata.id, binding.textEditor.content)
        }
    }
}
