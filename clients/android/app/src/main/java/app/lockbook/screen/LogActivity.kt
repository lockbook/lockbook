package app.lockbook.screen

import android.os.Bundle
import android.view.View
import android.widget.TextView
import androidx.appcompat.app.AppCompatActivity
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.R
import com.afollestad.recyclical.ViewHolder
import com.afollestad.recyclical.datasource.emptyDataSourceTyped
import com.afollestad.recyclical.setup
import com.afollestad.recyclical.withItem
import java.io.File

class LogActivity : AppCompatActivity() {

    companion object {
        const val LOG_FILE_NAME = "lockbook.log"
    }

    var logSegments = emptyDataSourceTyped<LogTextViewHolderInfo>()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_debug)

        getDebugContent()
    }

    private fun getDebugContent() {
        val debugTextView = findViewById<RecyclerView>(R.id.debug_text)
        debugTextView.setup {
            withDataSource(logSegments)

            withItem<LogTextViewHolderInfo, LogViewHolder>(R.layout.log_segment_item) {
                onBind(::LogViewHolder) { _, item ->
                    logItem.text = item.textSegment
                }
            }
        }

        logSegments.set(File("$filesDir/$LOG_FILE_NAME").readText().split("\n").map { LogTextViewHolderInfo(it) })
    }
}

class LogViewHolder(itemView: View) : ViewHolder(itemView) {
    val logItem: TextView = itemView.findViewById(R.id.log_item_text)
}

data class LogTextViewHolderInfo(
    val textSegment: String
)
