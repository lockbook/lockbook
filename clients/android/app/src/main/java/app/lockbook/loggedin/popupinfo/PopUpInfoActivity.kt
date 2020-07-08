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

class PopUpInfoActivity: Activity() {

    lateinit var name: String
    lateinit var id: String
    lateinit var fileType: String
    lateinit var metadataVersion: String
    lateinit var contentVersion: String

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val binding: ActivityPopupInfoBinding = DataBindingUtil.setContentView(this,
            R.layout.activity_popup_info
        )
        binding.popUpInfoActivity = this
    }
}