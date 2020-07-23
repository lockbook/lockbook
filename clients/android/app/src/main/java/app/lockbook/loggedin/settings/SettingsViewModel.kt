package app.lockbook.loggedin.settings

import androidx.lifecycle.ViewModel
import app.lockbook.loggedin.listfiles.ClickInterface

class SettingsViewModel: ViewModel(), ClickInterface {
    val settings = listOf("Export Account String (QR Code)", "Export Raw Account String")

    override fun onItemClick(position: Int) {
        TODO("Not yet implemented")
    }

    override fun onLongClick(position: Int) {
        TODO("Not yet implemented")
    }

}