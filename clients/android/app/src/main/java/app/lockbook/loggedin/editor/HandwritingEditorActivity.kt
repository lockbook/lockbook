package app.lockbook.loggedin.editor

import android.os.Bundle
import android.os.Handler
import android.view.View
import android.widget.AdapterView
import android.widget.ArrayAdapter
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.lifecycle.ViewModelProvider
import app.lockbook.R
import app.lockbook.utils.TEXT_EDITOR_BACKGROUND_SAVE_PERIOD
import com.beust.klaxon.Klaxon
import com.github.nwillc.ksvg.elements.Container
import com.github.nwillc.ksvg.elements.Element
import com.github.nwillc.ksvg.elements.G
import kotlinx.android.synthetic.main.activity_handwriting_editor.*
import timber.log.Timber
import java.util.*

class HandwritingEditorActivity: AppCompatActivity() {
    private lateinit var handwritingEditorViewModel: HandwritingEditorViewModel
    private var timer: Timer = Timer()
    private val handler = Handler()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_handwriting_editor)

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

        handwritingEditorViewModel =
            ViewModelProvider(
                this,
                HandwritingEditorViewModelFactory(application, id)
            ).get(HandwritingEditorViewModel::class.java)

        handwritingEditorViewModel.errorHasOccurred.observe(
            this,
            { errorHasOccurred ->
                errorHasOccurred(errorHasOccurred)
            }
        )

        setUpHandwritingToolbar()
        Timber.e("SMAIL3: $contents")
        val g = G()
        g.body = contents
        handwriting_editor.svgObject.children.add(g)
        startBackgroundSave()
    }

    private fun startBackgroundSave() {
        timer.schedule(
            object : TimerTask() {
                override fun run() {
                    handler.post {
                        handwritingEditorViewModel.saveSVG(handwriting_editor.svgObject.toString())
                    }
                }
            },
            1000,
            TEXT_EDITOR_BACKGROUND_SAVE_PERIOD
        )
    }

    private fun setUpHandwritingToolbar() {
        ArrayAdapter.createFromResource(
            this,
            R.array.handwriting_editor_pen_size,
            android.R.layout.simple_spinner_item
        ).also { adapter ->
            adapter.setDropDownViewResource(android.R.layout.simple_spinner_dropdown_item)
            handwriting_editor_pen_size_spinner.adapter = adapter
        }

        handwriting_editor_pen_size_spinner.onItemSelectedListener =
            object : AdapterView.OnItemSelectedListener {
                override fun onItemSelected(
                    parent: AdapterView<*>?,
                    view: View?,
                    position: Int,
                    id: Long
                ) {

                }

                override fun onNothingSelected(parent: AdapterView<*>?) {}

            }

        ArrayAdapter.createFromResource(
            this,
            R.array.handwriting_editor_pallete_colors,
            android.R.layout.simple_spinner_item
        ).also { adapter ->
            adapter.setDropDownViewResource(android.R.layout.simple_spinner_dropdown_item)
            handwriting_editor_pallete_spinner.adapter = adapter
        }

        handwriting_editor_pallete_spinner.onItemSelectedListener =
            object : AdapterView.OnItemSelectedListener {
                override fun onItemSelected(
                    parent: AdapterView<*>?,
                    view: View?,
                    position: Int,
                    id: Long
                ) {

                }

                override fun onNothingSelected(parent: AdapterView<*>?) {}

            }
    }

    private fun errorHasOccurred(errorText: String) {
        finish()
        Toast.makeText(applicationContext, errorText, Toast.LENGTH_LONG).show()
    }

}