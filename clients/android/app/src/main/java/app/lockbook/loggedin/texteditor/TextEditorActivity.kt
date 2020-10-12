package app.lockbook.loggedin.texteditor

import android.os.Bundle
import android.os.Handler
import android.text.style.ForegroundColorSpan
import android.view.Menu
import android.view.MenuItem
import android.view.View
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.res.ResourcesCompat
import androidx.lifecycle.Observer
import androidx.lifecycle.ViewModelProvider
import app.lockbook.R
import app.lockbook.utils.Messages.UNEXPECTED_ERROR
import app.lockbook.utils.TEXT_EDITOR_BACKGROUND_SAVE_PERIOD
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
    var menu: Menu? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_text_editor)

        val id = intent.getStringExtra("id")
        val contents = intent.getStringExtra("contents")

        if (id == null) {
            errorHasOccurred("Unable to retrieve id.")
            finish()
            return
        }
        if (contents == null) {
            errorHasOccurred("Unable to retrieve contents.")
            finish()
            return
        }

        val textEditorViewModelFactory =
            TextEditorViewModelFactory(
                id,
                filesDir.absolutePath,
                contents
            )

        textEditorViewModel =
            ViewModelProvider(this, textEditorViewModelFactory).get(TextEditorViewModel::class.java)

        textEditorViewModel.canUndo.observe(
            this,
            Observer { canUndo ->
                menu?.findItem(R.id.menu_text_editor_undo)?.isEnabled = canUndo
            }
        )

        textEditorViewModel.canRedo.observe(
            this,
            Observer { canRedo ->
                menu?.findItem(R.id.menu_text_editor_redo)?.isEnabled = canRedo
            }
        )

        textEditorViewModel.errorHasOccurred.observe(
            this,
            Observer { errorText ->
                errorHasOccurred(errorText)
            }
        )

        setUpView()
        startBackgroundSave()
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
        Snackbar.make(text_editor_layout, error, Snackbar.LENGTH_SHORT).show()
        finish()
    }

    private fun setUpView() {
        val name = intent.getStringExtra("name")
        if (name == null) {
            errorHasOccurred("Unable to retrieve file name.")
            finish()
            return
        }

        title = name
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

        text_editor.setText(intent.getStringExtra("contents"))

        text_editor.addTextChangedListener(textEditorViewModel)
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
//            R.id.menu_text_editor_search -> { }
            R.id.menu_text_editor_view_md -> viewMarkdown()
            R.id.menu_text_editor_redo -> handleTextRedo()
            R.id.menu_text_editor_undo -> handleTextUndo()
            else -> {
                Timber.e("Menu item not matched: ${item.itemId}")
                Toast.makeText(applicationContext, UNEXPECTED_ERROR, Toast.LENGTH_LONG).show()
            }
        }

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
