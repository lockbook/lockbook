package app.lockbook.screen

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import androidx.appcompat.app.AppCompatActivity
import androidx.core.net.toFile
import net.lockbook.Lb
import java.io.File

class ShareReceiverActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val sources = when (intent?.action){
            Intent.ACTION_SEND_MULTIPLE -> {
                val uris = intent.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM)
                uris
            }
            Intent.ACTION_SEND -> {
                val uri = intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM)
                listOf(uri)
            }
            else -> {
                listOf()
            }
        }

        val sourcesPaths = sources?.map { contentResolver.query()}?.toTypedArray()
        println("paths:" + sourcesPaths)
        Lb.importFiles(sourcesPaths, Lb.getRoot().id)
    }

    fun handleSingleFile() {
        val uri = intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM)
        uri?.let { fileUri ->
            processFile(fileUri)
        }
    }

    fun handleMultipleFiles() {
        val uris = intent.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM)
        uris?.forEach { fileUri ->
            processFile(fileUri)
        }
    }

    fun processFile(uri: Uri) {
        try {
            println("sup - uri: " + uri.path)
        } catch (e: Exception) {
            e.printStackTrace()
        }
    }
}