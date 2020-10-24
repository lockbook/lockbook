package app.lockbook.loggedin.editor

import android.os.Bundle
import android.os.Handler
import android.text.style.ForegroundColorSpan
import android.view.Menu
import android.view.MenuItem
import android.view.View
import androidx.appcompat.app.AlertDialog
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.res.ResourcesCompat
import androidx.lifecycle.ViewModelProvider
import app.lockbook.R
import app.lockbook.utils.Messages.UNEXPECTED_CLIENT_ERROR
import app.lockbook.utils.Messages.UNEXPECTED_ERROR
import app.lockbook.utils.TEXT_EDITOR_BACKGROUND_SAVE_PERIOD
import app.lockbook.utils.exhaustive
import com.google.android.material.snackbar.Snackbar
import io.noties.markwon.Markwon
import io.noties.markwon.editor.MarkwonEditor
import io.noties.markwon.editor.MarkwonEditorTextWatcher
import kotlinx.android.synthetic.main.activity_list_files.*
import kotlinx.android.synthetic.main.activity_text_editor.*
import kotlinx.android.synthetic.main.splash_screen.*
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
                        textEditorViewModel.writeNewTextToDocument(text_editor.text.toString())
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
        AlertDialog.Builder(this, R.style.DarkBlue_Dialog)
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

        if (title.endsWith(".md")) {
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

            text_editor.addTextChangedListener(
                MarkwonEditorTextWatcher.withPreRender(
                    markdownEditor,
                    Executors.newCachedThreadPool(),
                    text_editor
                )
            )
        }

        val contents = textEditorViewModel.handleReadDocument(id)
        if (contents != null) {
            text_editor.setText(contents)
            text_editor.addTextChangedListener(textEditorViewModel)
            startBackgroundSave()
        }
    }

    private fun viewMarkdown() {
        if (text_editor_scroller.visibility == View.VISIBLE) {
            val markdown = Markwon.create(this)
            markdown.setMarkdown(markdown_viewer, text_editor.text.toString())
            menu?.findItem(R.id.menu_text_editor_undo)?.isVisible = false
            menu?.findItem(R.id.menu_text_editor_redo)?.isVisible = false
            text_editor_scroller.visibility = View.GONE
            markdown_viewer_scroller.visibility = View.VISIBLE
        } else {
            markdown_viewer_scroller.visibility = View.GONE
            text_editor_scroller.visibility = View.VISIBLE
            menu?.findItem(R.id.menu_text_editor_undo)?.isVisible = true
            menu?.findItem(R.id.menu_text_editor_redo)?.isVisible = true
        }
    }

    override fun onCreateOptionsMenu(menu: Menu?): Boolean {
        menuInflater.inflate(R.menu.menu_text_editor, menu)
        this.menu = menu
        if (title.endsWith(".md")) {
            menu?.findItem(R.id.menu_text_editor_view_md)?.isVisible = true
        }
        menu?.findItem(R.id.menu_text_editor_undo)?.isEnabled = false
        menu?.findItem(R.id.menu_text_editor_redo)?.isEnabled = false
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
                    splash_screen,
                    UNEXPECTED_CLIENT_ERROR,
                    Snackbar.LENGTH_SHORT
                )
                    .show()
            }
        }.exhaustive

        return true
    }

    private fun handleTextRedo() {
        val selectionPosition = text_editor.selectionStart
        val newText = textEditorViewModel.redo()
        val diff = text_editor.text.toString().length - newText.length
        textEditorViewModel.ignoreChange = true
        text_editor.setText(newText)
        text_editor.setSelection(selectionPosition - diff)
    }

    private fun handleTextUndo() {
        val selectionPosition = text_editor.selectionStart
        val newText = textEditorViewModel.undo()
        val diff = text_editor.text.toString().length - newText.length
        textEditorViewModel.ignoreChange = true
        text_editor.setText(newText)
        text_editor.setSelection(selectionPosition - diff)
    }

    override fun onDestroy() {
        super.onDestroy()
        timer.cancel()
        textEditorViewModel.writeNewTextToDocument(text_editor.text.toString())
    }
}

class CustomPunctuationSpan internal constructor(color: Int) : ForegroundColorSpan(color)
