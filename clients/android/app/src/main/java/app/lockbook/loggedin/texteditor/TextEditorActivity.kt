package app.lockbook.loggedin.texteditor

import android.app.Activity
import android.os.Bundle
import android.util.Log
import androidx.databinding.DataBindingUtil
import app.lockbook.R
import app.lockbook.databinding.ActivityTextEditorBinding
import kotlinx.android.synthetic.main.activity_text_editor.*

class TextEditorActivity : Activity() {
    companion object {
        const val OK: Int = 0
        const val ERR: Int = 1
    }

    var text: String = ""

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        text = intent.getStringExtra("text")

        val binding: ActivityTextEditorBinding =
            DataBindingUtil.setContentView(this, R.layout.activity_text_editor)

        binding.textEditorActivty = this
    }

    fun submitText() {
        text_editor.text?.let {
            intent.putExtra("text", it.toString())
            setResult(OK, intent)
            finish()
        }
        setResult(ERR)
        finish()
    }
}