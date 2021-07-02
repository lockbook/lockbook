package app.lockbook.screen

import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.text.style.ForegroundColorSpan
import android.view.Menu
import android.view.MenuItem
import android.view.View
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.res.ResourcesCompat
import androidx.core.view.isVisible
import androidx.lifecycle.ViewModelProvider
import app.lockbook.R
import app.lockbook.databinding.ActivityTextEditorBinding
import app.lockbook.model.AlertModel
import app.lockbook.model.TextEditorViewModel
import app.lockbook.modelfactory.TextEditorViewModelFactory
import app.lockbook.util.exhaustive
import io.noties.markwon.Markwon
import io.noties.markwon.editor.MarkwonEditor
import io.noties.markwon.editor.MarkwonEditorTextWatcher
import timber.log.Timber
import java.lang.ref.WeakReference
import java.util.*
import java.util.concurrent.Executors

class TextEditorActivity : AppCompatActivity() {
    private var _binding: ActivityTextEditorBinding? = null
    // This property is only valid between onCreateView and
    // onDestroyView.
    private val binding get() = _binding!!

    private val alertModel by lazy {
        AlertModel(WeakReference(this))
    }

    private val textEditorToolbar get() = binding.textEditorToolbar
    private val textField get() = binding.textEditorTextField

    private lateinit var textEditorViewModel: TextEditorViewModel
    private var isFirstLaunch = true
    private var timer: Timer = Timer()
    private val handler = Handler(requireNotNull(Looper.myLooper()))
    private var menu: Menu? = null

    companion object {
        const val TEXT_EDITOR_BACKGROUND_SAVE_PERIOD: Long = 5000
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        _binding = ActivityTextEditorBinding.inflate(layoutInflater)
        setContentView(binding.root)

        val id = intent.getStringExtra("id")

        if (id == null) {
            alertModel.notifyBasicError(::finish)
            return
        }

        textEditorViewModel =
            ViewModelProvider(
                this,
                TextEditorViewModelFactory(
                    application,
                    id
                )
            ).get(TextEditorViewModel::class.java)

        textEditorViewModel.canUndo.observe(
            this,
            { canUndo ->
                menu?.findItem(R.id.menu_text_editor_undo)?.isEnabled = canUndo
            }
        )

        textEditorViewModel.canRedo.observe(
            this,
            { canRedo ->
                menu?.findItem(R.id.menu_text_editor_redo)?.isEnabled = canRedo
            }
        )

        textEditorViewModel.notifyError.observe(
            this,
            { error ->
                alertModel.notifyError(error)
            }
        )

        setUpView(id)
    }

    private fun startBackgroundSave() {
        timer.schedule(
            object : TimerTask() {
                override fun run() {
                    handler.post {
                        textEditorViewModel.saveText(textField.text.toString())
                    }
                }
            },
            1000,
            TEXT_EDITOR_BACKGROUND_SAVE_PERIOD
        )
    }

    private fun setUpView(id: String) {
        val name = intent.getStringExtra("name")
        if (name == null) {
            alertModel.notifyBasicError(::finish)
            return
        }

        textEditorToolbar.title = name
        setSupportActionBar(textEditorToolbar)

        if (textEditorToolbar.title.endsWith(".md")) {
            menu?.findItem(R.id.menu_text_editor_view_md)?.isVisible = true
            val markdownEditor = MarkwonEditor.builder(Markwon.create(this))
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

            textField.addTextChangedListener(
                MarkwonEditorTextWatcher.withPreRender(
                    markdownEditor,
                    Executors.newCachedThreadPool(),
                    textField
                )
            )
        }

        val contents = textEditorViewModel.readDocument(id)
        if (contents != null) {
            textField.setText(contents)
            textField.addTextChangedListener(textEditorViewModel)

            isFirstLaunch = false
            startBackgroundSave()
        }
    }

    override fun onStart() {
        super.onStart()
        if (textEditorToolbar.title.endsWith(".md")) {
            setMarkdownButtonListeners()
        }
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
            val markdown = Markwon.create(this)
            markdown.setMarkdown(binding.markdownViewer, textField.text.toString())
            menu?.findItem(R.id.menu_text_editor_undo)?.isVisible = false
            menu?.findItem(R.id.menu_text_editor_redo)?.isVisible = false
            binding.markdownToolbar.isVisible = false
            binding.textEditorScroller.visibility = View.GONE
            binding.markdownViewerScroller.visibility = View.VISIBLE
        } else {
            binding.markdownViewerScroller.visibility = View.GONE
            binding.textEditorScroller.visibility = View.VISIBLE
            binding.markdownToolbar.isVisible = true
            menu?.findItem(R.id.menu_text_editor_undo)?.isVisible = true
            menu?.findItem(R.id.menu_text_editor_redo)?.isVisible = true
        }
    }

    override fun onCreateOptionsMenu(menu: Menu?): Boolean {
        menuInflater.inflate(R.menu.menu_text_editor, menu)
        this.menu = menu
        menu?.findItem(R.id.menu_text_editor_undo)?.isEnabled = false
        menu?.findItem(R.id.menu_text_editor_redo)?.isEnabled = false
        if (textEditorToolbar.title.endsWith(".md")) {
            menu?.findItem(R.id.menu_text_editor_view_md)?.isVisible = true
        }
        return true
    }

    override fun onOptionsItemSelected(item: MenuItem): Boolean {
        when (item.itemId) {
            R.id.menu_text_editor_view_md -> viewMarkdown()
            R.id.menu_text_editor_redo -> handleTextRedo()
            R.id.menu_text_editor_undo -> handleTextUndo()
            else -> {
                Timber.e("Menu item not matched: ${item.itemId}")
                alertModel.notifyBasicError(::finish)
            }
        }.exhaustive

        return true
    }

    private fun handleTextRedo() {
        val selectionPosition = textField.selectionStart
        val newText = textEditorViewModel.redo()
        val diff = textField.text.toString().length - newText.length
        textEditorViewModel.ignoreChange = true
        textField.setText(newText)
        textField.setSelection(selectionPosition - diff)
    }

    private fun handleTextUndo() {
        val selectionPosition = textField.selectionStart
        val newText = textEditorViewModel.undo()
        val diff = textField.text.toString().length - newText.length
        textEditorViewModel.ignoreChange = true
        textField.setText(newText)
        textField.setSelection(selectionPosition - diff)
    }

    override fun onDestroy() {
        super.onDestroy()
        timer.cancel()
        if (!isFirstLaunch) {
            textEditorViewModel.saveText(textField.text.toString())
        }
    }
}

class CustomPunctuationSpan internal constructor(color: Int) : ForegroundColorSpan(color)
