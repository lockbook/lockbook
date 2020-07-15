package app.lockbook.loggedin.popupinfo

import android.app.Activity
import android.os.Bundle
import androidx.databinding.DataBindingUtil
import app.lockbook.R
import app.lockbook.core.renameFile
import app.lockbook.databinding.ActivityPopupInfoBinding
import kotlinx.android.synthetic.main.activity_popup_info.*
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

    fun rename() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                setResult(renameFile(intent.getStringExtra("path"), id, new_name_text.text.toString()))

                withContext(Dispatchers.Main) {
                    finish()
                }
            }
        }
    }

}