package app.lockbook.model

import android.app.Application
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.os.Handler
import android.os.Looper
import androidx.lifecycle.*
import app.lockbook.App.Companion.config
import app.lockbook.getRes
import app.lockbook.ui.DrawingStrokeState
import app.lockbook.ui.DrawingView
import app.lockbook.ui.DrawingView.Tool
import app.lockbook.util.*
import app.lockbook.util.ColorAlias
import app.lockbook.util.Drawing
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Err
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

class DrawingViewModel(
    application: Application,
    val id: String,
    var persistentDrawing: Drawing,
    var persistentBitmap: Bitmap = Bitmap.createBitmap(
        DrawingView.CANVAS_WIDTH,
        DrawingView.CANVAS_HEIGHT, Bitmap.Config.ARGB_8888
    ),
    var persistentCanvas: Canvas = Canvas(persistentBitmap),
    var persistentStrokeState: DrawingStrokeState = DrawingStrokeState()
) : AndroidViewModel(application) {
    var selectedTool: Tool = Tool.Pen(ColorAlias.Black)

    private val handler = Handler(Looper.myLooper()!!)
    var lastEdit = 0L

    private val _notifyError = SingleMutableLiveData<LbError>()

    val notifyError: LiveData<LbError>
        get() = _notifyError

    init {
        setUpPaint()
        persistentDrawing.model = this
    }

    fun setUpPaint() {
        persistentStrokeState.apply {
            strokePaint.isAntiAlias = true
            strokePaint.style = Paint.Style.STROKE
            strokePaint.strokeJoin = Paint.Join.ROUND
            strokePaint.color = Color.WHITE
            strokePaint.strokeCap = Paint.Cap.ROUND

            bitmapPaint.strokeCap = Paint.Cap.ROUND
            bitmapPaint.strokeJoin = Paint.Join.ROUND

            backgroundPaint.style = Paint.Style.FILL

            strokeColor = ColorAlias.White
        }
    }

    fun waitAndSaveContents() {
        lastEdit = System.currentTimeMillis()
        val currentEdit = lastEdit

        handler.postDelayed(
            {
                viewModelScope.launch(Dispatchers.IO) {

                    if (currentEdit == lastEdit && persistentDrawing.isDirty) {
                        val writeToDocumentResult =
                            CoreModel.writeToDocument(
                                config,
                                id,
                                Klaxon().toJsonString(persistentDrawing.clone()).replace(" ", "")
                            )

                        if (writeToDocumentResult is Err) {
                            _notifyError.postValue(
                                writeToDocumentResult.error.toLbError(
                                    getRes()
                                )
                            )
                        } else {
                            persistentDrawing.isDirty = false
                        }
                    }
                }
            },
            5000
        )
    }
}
