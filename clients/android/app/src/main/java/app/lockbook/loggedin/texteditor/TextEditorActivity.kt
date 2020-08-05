package app.lockbook.loggedin.texteditor

import android.app.Activity
import android.os.Bundle
import android.text.Editable
import app.lockbook.R
import io.noties.markwon.*
import io.noties.markwon.editor.MarkwonEditor
import io.noties.markwon.editor.MarkwonEditorTextWatcher
import kotlinx.android.synthetic.main.activity_text_editor.*
import java.util.concurrent.Executors

class TextEditorActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_text_editor)
        setUpText()

        popup_info_rename.setOnClickListener {
            submitText()
        }
    }

    private fun setUpText() {
        text_editor.setText(intent.getStringExtra("text"))

        val markdownEditor = MarkwonEditor.create(Markwon.create(this))

        text_editor.addTextChangedListener(
            MarkwonEditorTextWatcher.withPreRender(
                markdownEditor,
                Executors.newCachedThreadPool(),
                text_editor
            )
        )
    }

    private fun submitText() {
        if (text_editor.text is Editable) {
            intent.putExtra("text", text_editor.text.toString())
        } else {
            intent.putExtra("text", "")
        }

        setResult(RESULT_OK, intent)
        finish()
    }
}
