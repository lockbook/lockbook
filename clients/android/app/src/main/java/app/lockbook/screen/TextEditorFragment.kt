package app.lockbook.screen

import android.annotation.SuppressLint
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.PopupMenu
import androidx.core.content.res.ResourcesCompat
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.fragment.app.viewModels
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import app.lockbook.R
import app.lockbook.databinding.FragmentTextEditorBinding
import app.lockbook.model.*
import app.lockbook.util.InsertMarkdownAction
import app.lockbook.util.MarkdownEditor
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

    private val textEditor by lazy {
        MarkdownEditor(requireContext(), model)
    }

    @SuppressLint("ClickableViewAccessibility")
    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = FragmentTextEditorBinding.inflate(inflater, container, false)
        val name = (activityModel.detailScreen as DetailScreen.TextEditor).file.name

        textEditorToolbar.title = name
        textEditorToolbar.setOnMenuItemClickListener { item ->
            when (item.itemId) {
                R.id.menu_text_editor_redo -> {
                    textEditor.undoRedo(true)
                }

                R.id.menu_text_editor_undo -> {
                    textEditor.undoRedo(false)
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
            binding.textEditorScroller.addView(textEditor)
        }

        model.notifyError.observe(
            viewLifecycleOwner
        ) { error ->
            alertModel.notifyError(error)
        }

        model.editorUpdate.observe(
            viewLifecycleOwner
        ) { response ->
            val selectedBackground = ResourcesCompat.getColor(resources, R.color.selectedMarkdownButtonBackground, null)
            val clearBackground = ResourcesCompat.getColor(resources, android.R.color.transparent, null)

            binding.menuMarkdownTitle.setBackgroundColor(if (response.cursorInHeading) selectedBackground else clearBackground)
            binding.menuMarkdownBold.setBackgroundColor(if (response.cursorInBold) selectedBackground else clearBackground)
            binding.menuMarkdownItalic.setBackgroundColor(if (response.cursorInItalic) selectedBackground else clearBackground)
            binding.menuMarkdownCode.setBackgroundColor(if (response.cursorInInlineCode) selectedBackground else clearBackground)
            binding.menuMarkdownStrikethrough.setBackgroundColor(if (response.cursorInStrikethrough) selectedBackground else clearBackground)
            binding.menuMarkdownNumberList.setBackgroundColor(if (response.cursorInNumberList) selectedBackground else clearBackground)
            binding.menuMarkdownBulletList.setBackgroundColor(if (response.cursorInBulletList) selectedBackground else clearBackground)
            binding.menuMarkdownTodoList.setBackgroundColor(if (response.cursorInTodoList) selectedBackground else clearBackground)
        }

        binding.menuMarkdownTitle.setOnClickListener {
            val popupMenu = PopupMenu(requireContext(), it)

            popupMenu.menuInflater.inflate(R.menu.menu_markdown_titles, popupMenu.menu)
            popupMenu.setOnMenuItemClickListener { menuItem ->
                when (menuItem.itemId) {
                    R.id.menu_title_1 -> textEditor.insertStyling(InsertMarkdownAction.Heading(1))
                    R.id.menu_title_2 -> textEditor.insertStyling(InsertMarkdownAction.Heading(2))
                    R.id.menu_title_3 -> textEditor.insertStyling(InsertMarkdownAction.Heading(3))
                    R.id.menu_title_4 -> textEditor.insertStyling(InsertMarkdownAction.Heading(4))
                    else -> return@setOnMenuItemClickListener false
                }

                true
            }

            popupMenu.show()
        }

        binding.menuMarkdownBold.setOnClickListener {
            textEditor.insertStyling(InsertMarkdownAction.Bold)
        }

        binding.menuMarkdownItalic.setOnClickListener {
            textEditor.insertStyling(InsertMarkdownAction.Italic)
        }

        binding.menuMarkdownCode.setOnClickListener {
            textEditor.insertStyling(InsertMarkdownAction.InlineCode)
        }

        binding.menuMarkdownStrikethrough.setOnClickListener {
            textEditor.insertStyling(InsertMarkdownAction.Strikethrough)
        }

        binding.menuMarkdownNumberList.setOnClickListener {
            textEditor.insertStyling(InsertMarkdownAction.NumberList)
        }

        binding.menuMarkdownBulletList.setOnClickListener {
            textEditor.insertStyling(InsertMarkdownAction.BulletList)
        }

        binding.menuMarkdownTodoList.setOnClickListener {
            textEditor.insertStyling(InsertMarkdownAction.TodoList)
        }

        binding.menuMarkdownIndent.setOnClickListener {
            textEditor.indentAtCursor(false)
        }

        binding.menuMarkdownDeindent.setOnClickListener {
            textEditor.indentAtCursor(true)
        }

        binding.menuMarkdownCut.setOnClickListener {
            textEditor.clipboardCut()
        }

        binding.menuMarkdownCopy.setOnClickListener {
            textEditor.clipboardCopy()
        }

        binding.menuMarkdownPaste.setOnClickListener {
            textEditor.clipboardPaste()
        }

        return binding.root
    }

    override fun onDestroy() {
        super.onDestroy()
        model.savedCursorStart = textEditor.getCursorStart()
        model.savedCursorEnd = textEditor.getCursorEnd()
    }

    fun saveOnExit() {
        if (model.isDirty) {
            model.lastEdit = System.currentTimeMillis()
            activityModel.saveTextOnExit(model.fileMetadata.id, textEditor.getText() ?: return)
        }
    }
}
