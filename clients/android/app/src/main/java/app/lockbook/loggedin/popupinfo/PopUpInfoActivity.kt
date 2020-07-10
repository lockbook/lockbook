package app.lockbook.loggedin.popupinfo

import android.app.Activity
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.databinding.DataBindingUtil
import androidx.fragment.app.Fragment
import app.lockbook.R
import app.lockbook.databinding.ActivityImportAccountBinding
import app.lockbook.databinding.ActivityPopupInfoBinding
import app.lockbook.utils.FileMetadata
import app.lockbook.utils.FileType

class PopUpInfoActivity: Activity() {

    var name: String = "Name: "
    var id: String = "ID: "
    var fileType: String = "File Type: "
    var metadataVersion: String = "Metadata Version: "
    var contentVersion: String = "Content Version: "

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        name += intent.getStringExtra("name")
        id += intent.getStringExtra("id")
        fileType += intent.getStringExtra("fileType")
        metadataVersion += intent.getStringExtra("metadataVersion")
        contentVersion += intent.getStringExtra("contentVersion")

        val binding: ActivityPopupInfoBinding = DataBindingUtil.setContentView(this,
            R.layout.activity_popup_info
        )
        binding.popUpInfoActivity = this
    }

}