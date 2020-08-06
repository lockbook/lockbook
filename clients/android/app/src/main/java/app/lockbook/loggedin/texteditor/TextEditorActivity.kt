package app.lockbook.loggedin.texteditor

import android.os.Bundle
import android.text.Editable
import android.view.Menu
import android.view.MenuItem
import android.view.View
import androidx.appcompat.app.AppCompatActivity
import app.lockbook.R
import io.noties.markwon.*
import io.noties.markwon.editor.MarkwonEditor
import io.noties.markwon.editor.MarkwonEditorTextWatcher
import kotlinx.android.synthetic.main.activity_text_editor.*
import timber.log.Timber
import java.util.concurrent.Executors

class TextEditorActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_text_editor)
        setUpView()
    }

    private fun setUpView() {
        supportActionBar?.setDisplayHomeAsUpEnabled(true)
        title = intent.getStringExtra("name")
        val markdownEditor = MarkwonEditor.create(Markwon.create(this))

        text_editor.addTextChangedListener(
            MarkwonEditorTextWatcher.withPreRender(
                markdownEditor,
                Executors.newCachedThreadPool(),
                text_editor
            )
        )

        text_editor.setText(intent.getStringExtra("contents"))
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

    private fun submitText() {
        intent.putExtra("contents", text_editor.text.toString())

        setResult(RESULT_OK, intent)
        finish()
    }


    override fun onCreateOptionsMenu(menu: Menu?): Boolean {
        menuInflater.inflate(R.menu.menu_text_editor, menu)
        return true
    }

    override fun onOptionsItemSelected(item: MenuItem): Boolean {
        Timber.i("lmao: ${item.itemId == R.id.menu_text_editor_done}")
        when (item.itemId) {
            R.id.menu_text_editor_done -> submitText()
            R.id.menu_text_editor_search -> {}
            R.id.menu_text_editor_view_md -> viewMarkdown()
            R.id.menu_text_editor_redo -> {}
            R.id.menu_text_editor_undo -> {}
        }

        return true
    }

    override fun onSupportNavigateUp(): Boolean {
        finish()
        return true
    }
}
