package app.lockbook.loggedin.editor

import android.graphics.Color
import android.graphics.Paint
import android.graphics.Path
import android.os.Bundle
import android.view.View
import android.widget.AdapterView
import android.widget.ArrayAdapter
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.lifecycle.ViewModelProvider
import app.lockbook.R
import kotlinx.android.synthetic.main.activity_handwriting_editor.*

class HandwritingEditorActivity: AppCompatActivity() {
    private lateinit var handwritingEditorViewModel: HandwritingEditorViewModel

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_handwriting_editor)

//        val id = intent.getStringExtra("id")
//        val contents = intent.getStringExtra("contents")
//
//        if (id == null) {
//            errorHasOccurred("Unable to retrieve id.")
//            finish()
//            return
//        }
//        if (contents == null) {
//            errorHasOccurred("Unable to retrieve contents.")
//            finish()
//            return
//        }

        handwritingEditorViewModel =
            ViewModelProvider(this, HandwritingEditorViewModelFactory(application, "id", "contents")).get(HandwritingEditorViewModel::class.java)

        ArrayAdapter.createFromResource(
            this,
            R.array.handwriting_editor_pen_size,
            android.R.layout.simple_spinner_item
        ).also { adapter ->
            adapter.setDropDownViewResource(android.R.layout.simple_spinner_dropdown_item)
            handwriting_editor_pen_size_spinner.adapter = adapter
        }

        handwriting_editor_pen_size_spinner.onItemSelectedListener = object : AdapterView.OnItemSelectedListener {
            override fun onItemSelected(
                parent: AdapterView<*>?,
                view: View?,
                position: Int,
                id: Long
            ) {
                val paint = Paint()
                paint.isAntiAlias = true
                paint.color = handwriting_editor_canvas.paints.last().color
                paint.style = Paint.Style.STROKE
                paint.strokeJoin = Paint.Join.MITER

                when(parent?.getItemAtPosition(position).toString()) {
                    getString(R.string.handwriting_editor_pen_1) ->  paint.strokeWidth = resources.getInteger(R.integer.handwriting_editor_pen_1).toFloat()
                    getString(R.string.handwriting_editor_pen_2) -> paint.strokeWidth = resources.getInteger(R.integer.handwriting_editor_pen_2).toFloat()
                    getString(R.string.handwriting_editor_pen_3) -> paint.strokeWidth = resources.getInteger(R.integer.handwriting_editor_pen_3).toFloat()
                    getString(R.string.handwriting_editor_pen_4) -> paint.strokeWidth = resources.getInteger(R.integer.handwriting_editor_pen_4).toFloat()
                }

                handwriting_editor_canvas.paints.add(paint)
                handwriting_editor_canvas.paths.add(Path())
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

        handwriting_editor_pallete_spinner.onItemSelectedListener = object : AdapterView.OnItemSelectedListener {
            override fun onItemSelected(
                parent: AdapterView<*>?,
                view: View?,
                position: Int,
                id: Long
            ) {
                val paint = Paint()
                paint.isAntiAlias = true
                paint.style = Paint.Style.STROKE
                paint.strokeJoin = Paint.Join.MITER
                paint.strokeWidth = handwriting_editor_canvas.paints.last().strokeWidth

                when(parent?.getItemAtPosition(position).toString()) {
                    getString(R.string.handwriting_editor_pallete_white) -> paint.color = Color.WHITE
                    getString(R.string.handwriting_editor_pallete_blue) -> paint.color = Color.BLUE
                    getString(R.string.handwriting_editor_pallete_red) -> paint.color = Color.RED
                    getString(R.string.handwriting_editor_pallete_yellow) -> paint.color = Color.YELLOW
                }

                handwriting_editor_canvas.paints.add(paint)
                handwriting_editor_canvas.paths.add(Path())
            }

            override fun onNothingSelected(parent: AdapterView<*>?) {}

        }
    }

    private fun errorHasOccurred(errorText: String) {
        finish()
        Toast.makeText(applicationContext, errorText, Toast.LENGTH_LONG).show()
    }

}