package app.lockbook.loggedin.popupinfo

import android.app.Activity
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.databinding.DataBindingUtil
import androidx.fragment.app.Fragment
import app.lockbook.R
import app.lockbook.core.deleteFileFolder
import app.lockbook.databinding.ActivityImportAccountBinding
import app.lockbook.databinding.ActivityPopupInfoBinding
import app.lockbook.utils.FileMetadata
import app.lockbook.utils.FileType
import kotlinx.coroutines.*

class PopUpInfoActivity: Activity() {

    companion object {
        const val OK: Int = 0
        const val ERR: Int = 1
    }

    var name: String = "ERROR"
    var id: String = "ERROR"
    var fileType: String = "ERROR"
    var metadataVersion: String = "ERROR"
    var contentVersion: String = "ERROR"

    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        setUpInfo()

        val binding: ActivityPopupInfoBinding = DataBindingUtil.setContentView(this,
            R.layout.activity_popup_info
        )
        binding.popUpInfoActivity = this
    }

    private fun setUpInfo() {
        name = intent.getStringExtra("name")
        id = intent.getStringExtra("id")
        fileType = intent.getStringExtra("fileType")
        metadataVersion = intent.getStringExtra("metadataVersion")
        contentVersion = intent.getStringExtra("contentVersion")
    }

    private fun delete() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                setResult(deleteFileFolder(intent.getStringExtra("path"), id))

                withContext(Dispatchers.Main) {
                    finish()
                }
            }
        }
    }

}