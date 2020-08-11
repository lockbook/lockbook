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
import io.noties.markwon.Markwon
import io.noties.markwon.editor.MarkwonEditor
import io.noties.markwon.editor.MarkwonEditorTextWatcher
import kotlinx.android.synthetic.main.activity_text_editor.*
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

        val textEditorViewModelFactory =
            TextEditorViewModelFactory(
                intent.getStringExtra("id") ?: "INVALID ID",
                filesDir.absolutePath,
                intent.getStringExtra("contents") ?: ""
            ) //TODO handle this more appropriately

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

        timer.schedule(object : TimerTask() {
            override fun run() {
                handler.post {
                    textEditorViewModel.writeNewTextToDocument(text_editor.text.toString())
                }
            }
        }, 5000, 1000)
    }

    private fun errorHasOccurred(errorText: String) {
        finish()
        Toast.makeText(applicationContext, errorText, Toast.LENGTH_LONG).show()
    }

    private fun setUpView() {
        title = intent.getStringExtra("name")
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

        text_editor.setText(intent.getStringExtra("contents"))

        text_editor.addTextChangedListener(textEditorViewModel)
    }

    private fun viewMarkdown() {
        if (text_editor_scroller.visibility == View.VISIBLE) {
            val markdown = Markwon.create(this)
            markdown.setMarkdown(markdown_viewer, text_editor.text.toString())

            text_editor_scroller.visibility = View.GONE
            markdown_viewer_scroller.visibility = View.VISIBLE
        } else {
            markdown_viewer_scroller.visibility = View.GONE
            text_editor_scroller.visibility = View.VISIBLE
        }
    }

    override fun onCreateOptionsMenu(menu: Menu?): Boolean {
        menuInflater.inflate(R.menu.menu_text_editor, menu)
        this.menu = menu
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
    }
}

class CustomPunctuationSpan internal constructor(color: Int) : ForegroundColorSpan(color)
