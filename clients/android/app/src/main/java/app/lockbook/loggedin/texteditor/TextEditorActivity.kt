package app.lockbook.loggedin.texteditor

import android.app.Activity
import android.os.Bundle
import android.text.Editable
import app.lockbook.R
import app.lockbook.utils.RequestResultCodes.FAILED_RESULT_CODE
import kotlinx.android.synthetic.main.activity_text_editor.*

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
        text_editor_text.setText(intent.getStringExtra("text"))
    }

    private fun submitText() {
        if(text_editor_text.text is Editable) {
            intent.putExtra("text", text_editor_text.text.toString())
        } else {
            intent.putExtra("text", "")
        }

        setResult(RESULT_OK, intent)
        finish()
    }
}