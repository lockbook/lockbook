package app.lockbook.model

import android.app.Application
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.os.Handler
import android.os.Looper
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.ui.DrawingStrokeState
import app.lockbook.ui.DrawingView
import app.lockbook.ui.DrawingView.Tool
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

class DrawingViewModel(
    application: Application,
    val id: String,
    val persistentDrawingInfo: PersistentDrawingInfo
) : AndroidViewModel(application) {
    var selectedTool: Tool = Tool.Pen(ColorAlias.Black)

    private val handler = Handler(Looper.myLooper()!!)
    var lastEdit = 0L

    private val _notifyError = SingleMutableLiveData<LbError>()

    val notifyError: LiveData<LbError>
        get() = _notifyError

    init {
        setUpPaint()
        persistentDrawingInfo.drawing.model = this
        persistentDrawingInfo.drawing.uiMode = getRes().configuration.uiMode
    }

    private fun setUpPaint() {
        persistentDrawingInfo.strokeState.apply {
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
        lastEdit = System.currentTimeMillis() // the newest edit
        val currentEdit = lastEdit // the current edit for when the coroutine is launched

        handler.postDelayed({
                viewModelScope.launch(Dispatchers.IO) {
                    if (currentEdit == lastEdit && persistentDrawingInfo.drawing.isDirty) {
                        when (
                            val saveDrawingResult =
                                CoreModel.saveDrawing(
                                    id,
                                    persistentDrawingInfo.drawing.clone()
                                )
                        ) {
                            is Ok -> {
                                persistentDrawingInfo.drawing.isDirty = false
                            }
                            is Err -> {
                                _notifyError.postValue(
                                    saveDrawingResult.error.toLbError(
                                        getRes()
                                    )
                                )
                            }
                        }.exhaustive
                    }
                }
            },
            5000
        )
    }
}

data class PersistentDrawingInfo(
    var drawing: Drawing,
    var bitmap: Bitmap = Bitmap.createBitmap(
        DrawingView.CANVAS_WIDTH,
        DrawingView.CANVAS_HEIGHT, Bitmap.Config.ARGB_8888
    ),
    var canvas: Canvas = Canvas(bitmap),
    var strokeState: DrawingStrokeState = DrawingStrokeState()
)
