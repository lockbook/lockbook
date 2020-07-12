package app.lockbook.loggedin.texteditor

import android.app.Activity
import android.os.Bundle
import androidx.databinding.DataBindingUtil
import app.lockbook.R
import app.lockbook.databinding.ActivityTextEditorBinding
import kotlinx.android.synthetic.main.activity_text_editor.*

class TextEditorActivity : Activity() {
    var text: String = ""

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        text += intent.getStringExtra("text")

        val binding: ActivityTextEditorBinding =
            DataBindingUtil.setContentView(this, R.layout.activity_text_editor)

        binding.textEditorActivty = this
    }

    fun submitText() {
        intent.putExtra("text", text_editor.text.toString())
        // NEXT THING YOU WERE GONNA DO IS ADD THE INTENT AND HANDEL THE INPUT CODE OR WHATEVER
    }
}