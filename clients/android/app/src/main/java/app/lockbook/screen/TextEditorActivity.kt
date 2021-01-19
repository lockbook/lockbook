package app.lockbook.screen

import android.os.Bundle
import android.os.Handler
import android.text.style.ForegroundColorSpan
import android.view.Menu
import android.view.MenuItem
import android.view.View
import androidx.appcompat.app.AlertDialog
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.res.ResourcesCompat
import androidx.core.view.isVisible
import androidx.lifecycle.ViewModelProvider
import app.lockbook.R
import app.lockbook.model.TextEditorViewModel
import app.lockbook.modelfactory.TextEditorViewModelFactory
import app.lockbook.util.Messages.UNEXPECTED_CLIENT_ERROR
import app.lockbook.util.Messages.UNEXPECTED_ERROR
import app.lockbook.util.TEXT_EDITOR_BACKGROUND_SAVE_PERIOD
import app.lockbook.util.exhaustive
import com.google.android.material.snackbar.Snackbar
import io.noties.markwon.Markwon
import io.noties.markwon.editor.MarkwonEditor
import io.noties.markwon.editor.MarkwonEditorTextWatcher
import kotlinx.android.synthetic.main.activity_text_editor.*
import timber.log.Timber
import java.util.*
import java.util.concurrent.Executors

class TextEditorActivity : AppCompatActivity() {
    private lateinit var textEditorViewModel: TextEditorViewModel
    private var timer: Timer = Timer()
    private val handler = Handler()
    private var menu: Menu? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_text_editor)

        val id = intent.getStringExtra("id")

        if (id == null) {
            errorHasOccurred("Unable to retrieve id.")
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

        textEditorViewModel.errorHasOccurred.observe(
            this,
            { errorText ->
                errorHasOccurred(errorText)
            }
        )

        textEditorViewModel.unexpectedErrorHasOccurred.observe(
            this,
            { errorText ->
                unexpectedErrorHasOccurred(errorText)
            }
        )

        setUpView(id)
    }

    private fun startBackgroundSave() {
        timer.schedule(
            object : TimerTask() {
                override fun run() {
                    handler.post {
                        textEditorViewModel.writeNewTextToDocument(text_editor_text_field.text.toString())
                    }
                }
            },
            1000,
            TEXT_EDITOR_BACKGROUND_SAVE_PERIOD
        )
    }

    private fun errorHasOccurred(error: String) {
        Snackbar.make(text_editor_layout, error, Snackbar.LENGTH_SHORT).addCallback(object : Snackbar.Callback() {
            override fun onDismissed(transientBottomBar: Snackbar?, event: Int) {
                super.onDismissed(transientBottomBar, event)
                finish()
            }
        }).show()
    }

    private fun unexpectedErrorHasOccurred(error: String) {
        AlertDialog.Builder(this, R.style.Main_Dialog)
            .setTitle(UNEXPECTED_ERROR)
            .setMessage(error)
            .setOnCancelListener {
                finish()
            }
            .show()
    }

    private fun setUpView(id: String) {
        val name = intent.getStringExtra("name")
        if (name == null) {
            errorHasOccurred("Unable to retrieve file name.")
            return
        }

        text_editor_toolbar.title = name
        setSupportActionBar(text_editor_toolbar)

        if (text_editor_toolbar.title.endsWith(".md")) {
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

            markdown_toolbar.visibility = View.VISIBLE

            text_editor_text_field.addTextChangedListener(
                MarkwonEditorTextWatcher.withPreRender(
                    markdownEditor,
                    Executors.newCachedThreadPool(),
                    text_editor_text_field
                )
            )
        }

        val contents = textEditorViewModel.handleReadDocument(id)
        if (contents != null) {
            text_editor_text_field.setText(contents)
            text_editor_text_field.addTextChangedListener(textEditorViewModel)
            startBackgroundSave()
        }
    }

    override fun onStart() {
        super.onStart()
        if (text_editor_toolbar.title.endsWith(".md")) {
            setMarkdownButtonListeners()
        }
    }

    private fun setMarkdownButtonListeners() {
        menu_markdown_title.setOnClickListener {
            text_editor_text_field.text.replace(text_editor_text_field.selectionStart, text_editor_text_field.selectionStart, "# ")
        }

        menu_markdown_bold.setOnClickListener {
            val selectionStart = text_editor_text_field.selectionStart
            val selectionEnd = text_editor_text_field.selectionEnd
            if (selectionStart == selectionEnd) {
                text_editor_text_field.text.replace(selectionStart, selectionStart, "****")
                text_editor_text_field.setSelection(selectionStart + 2)
            } else {
                text_editor_text_field.text.replace(selectionStart, selectionStart, "**")
                val newSelectionEnd = selectionEnd + 2
                text_editor_text_field.text.replace(newSelectionEnd, newSelectionEnd, "**")
                text_editor_text_field.setSelection(newSelectionEnd)
            }
        }

        menu_markdown_italics.setOnClickListener {
            val selectionStart = text_editor_text_field.selectionStart
            val selectionEnd = text_editor_text_field.selectionEnd
            if (selectionStart == selectionEnd) {
                text_editor_text_field.text.replace(selectionStart, selectionStart, "__")
                text_editor_text_field.setSelection(selectionStart + 1)
            } else {
                text_editor_text_field.text.replace(selectionStart, selectionStart, "_")
                val newSelectionEnd = selectionEnd + 1
                text_editor_text_field.text.replace(newSelectionEnd, newSelectionEnd, "_")
                text_editor_text_field.setSelection(newSelectionEnd)
            }
        }

        menu_markdown_image.setOnClickListener {
            val selectionStart = text_editor_text_field.selectionStart
            text_editor_text_field.text.replace(selectionStart, text_editor_text_field.selectionEnd, "![]()")
            text_editor_text_field.setSelection(selectionStart + 2)
        }

        menu_markdown_link.setOnClickListener {
            val selectionStart = text_editor_text_field.selectionStart
            text_editor_text_field.text.replace(selectionStart, text_editor_text_field.selectionEnd, "[]()")
            text_editor_text_field.setSelection(selectionStart + 1)
        }

        menu_markdown_code.setOnClickListener {
            val selectionStart = text_editor_text_field.selectionStart
            val selectionEnd = text_editor_text_field.selectionEnd
            if (selectionStart == selectionEnd) {
                text_editor_text_field.text.replace(selectionStart, selectionStart, "``")
                text_editor_text_field.setSelection(selectionStart + 1)
            } else {
                text_editor_text_field.text.replace(selectionStart, selectionStart, "`")
                val newSelectionEnd = selectionEnd + 1
                text_editor_text_field.text.replace(newSelectionEnd, newSelectionEnd, "`")
                text_editor_text_field.setSelection(newSelectionEnd)
            }
        }
    }

    private fun viewMarkdown() {
        if (text_editor_scroller.visibility == View.VISIBLE) {
            val markdown = Markwon.create(this)
            markdown.setMarkdown(markdown_viewer, text_editor_text_field.text.toString())
            menu?.findItem(R.id.menu_text_editor_undo)?.isVisible = false
            menu?.findItem(R.id.menu_text_editor_redo)?.isVisible = false
            markdown_toolbar.isVisible = false
            text_editor_scroller.visibility = View.GONE
            markdown_viewer_scroller.visibility = View.VISIBLE
        } else {
            markdown_viewer_scroller.visibility = View.GONE
            text_editor_scroller.visibility = View.VISIBLE
            markdown_toolbar.isVisible = true
            menu?.findItem(R.id.menu_text_editor_undo)?.isVisible = true
            menu?.findItem(R.id.menu_text_editor_redo)?.isVisible = true
        }
    }

    override fun onCreateOptionsMenu(menu: Menu?): Boolean {
        menuInflater.inflate(R.menu.menu_text_editor, menu)
        this.menu = menu
        menu?.findItem(R.id.menu_text_editor_undo)?.isEnabled = false
        menu?.findItem(R.id.menu_text_editor_redo)?.isEnabled = false
        if (text_editor_toolbar.title.endsWith(".md")) {
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
                Snackbar.make(
                    text_editor_layout,
                    UNEXPECTED_CLIENT_ERROR,
                    Snackbar.LENGTH_SHORT
                )
                    .show()
            }
        }.exhaustive

        return true
    }

    private fun handleTextRedo() {
        val selectionPosition = text_editor_text_field.selectionStart
        val newText = textEditorViewModel.redo()
        val diff = text_editor_text_field.text.toString().length - newText.length
        textEditorViewModel.ignoreChange = true
        text_editor_text_field.setText(newText)
        text_editor_text_field.setSelection(selectionPosition - diff)
    }

    private fun handleTextUndo() {
        val selectionPosition = text_editor_text_field.selectionStart
        val newText = textEditorViewModel.undo()
        val diff = text_editor_text_field.text.toString().length - newText.length
        textEditorViewModel.ignoreChange = true
        text_editor_text_field.setText(newText)
        text_editor_text_field.setSelection(selectionPosition - diff)
    }

    override fun onDestroy() {
        super.onDestroy()
        timer.cancel()
        textEditorViewModel.writeNewTextToDocument(text_editor_text_field.text.toString())
    }
}

class CustomPunctuationSpan internal constructor(color: Int) : ForegroundColorSpan(color)
