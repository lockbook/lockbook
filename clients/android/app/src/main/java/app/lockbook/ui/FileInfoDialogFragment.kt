package app.lockbook.ui

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.fragment.app.DialogFragment
import app.lockbook.R
import kotlinx.android.synthetic.main.dialog_file_info.view.*
import java.sql.Timestamp
import java.util.*

class FileInfoDialogFragment: DialogFragment() {

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View? {
        val view = inflater.inflate(R.layout.dialog_file_info, container, false)

        val currentArgs = arguments
        if(currentArgs != null) {
            setUpInfo(view, currentArgs)
        }

        return view
    }

    private fun setUpInfo(view: View, bundle: Bundle) {
        val name = bundle.getString("name")
        val id = bundle.getString("id")
        val tempMetadataVersion = bundle.getString("metadataVersion")
        val tempContentVersion = bundle.getString("contentVersion")
        val fileType = bundle.getString("fileType")
        val metadataVersion = tempMetadataVersion?.toLongOrNull()
        val contentVersionError = tempContentVersion?.toLongOrNull()
        if (metadataVersion == null) {
            view.popup_info_metadata_version.text = getString(
                R.string.popup_info_metadata_version,
                "ERROR"
            )
        } else {
            val dateMetadataVersion = Date(Timestamp(metadataVersion).time)
            view.popup_info_metadata_version.text = getString(
                R.string.popup_info_metadata_version,
                if (dateMetadataVersion.time != 0L) dateMetadataVersion else resources.getString(R.string.pop_up_info_never_synced)
            )
        }

        if (contentVersionError == null) {
            view.popup_info_content_version.text = getString(
                R.string.popup_info_content_version,
                "Error"
            )
        } else {
            val dateContentVersion = Date(Timestamp(contentVersionError).time)
            view.popup_info_content_version.text = getString(
                R.string.popup_info_content_version,
                if (dateContentVersion.time != 0L) dateContentVersion else resources.getString(R.string.pop_up_info_never_synced)
            )
        }

        view.popup_info_name.text = getString(R.string.popup_info_name, name)
        view.popup_info_id.text = getString(R.string.popup_info_id, id)
        view.popup_info_file_type.text =
            getString(R.string.popup_info_file_type, fileType)
    }
}