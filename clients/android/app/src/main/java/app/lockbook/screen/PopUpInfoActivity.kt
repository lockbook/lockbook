package app.lockbook.screen

import android.app.Activity
import android.os.Bundle
import app.lockbook.R
import app.lockbook.util.RequestResultCodes.DELETE_RESULT_CODE
import app.lockbook.util.RequestResultCodes.RENAME_RESULT_CODE
import kotlinx.android.synthetic.main.activity_popup_info.*
import java.sql.Date
import java.sql.Timestamp

class PopUpInfoActivity : Activity() {
    lateinit var id: String
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_popup_info)
        setUpInfo()

        popup_info_delete.setOnClickListener {
            delete()
        }

        popup_info_rename.setOnClickListener {
            rename()
        }
    }

    private fun setUpInfo() {
        val name = intent.getStringExtra("name") ?: "ERROR"
        id = intent.getStringExtra("id") ?: "ERROR"
        val tempMetadataVersion = intent.getStringExtra("metadataVersion") ?: "ERROR"
        val tempContentVersion = intent.getStringExtra("contentVersion") ?: "ERROR"
        val fileType = intent.getStringExtra("fileType") ?: "ERROR"
        val metadataVersion = tempMetadataVersion.toLongOrNull()
        val contentVersionError = tempContentVersion.toLongOrNull()
        if (metadataVersion == null) {
            popup_info_metadata_version.text = getString(
                R.string.popup_info_metadata_version,
                "ERROR"
            )
        } else {
            val dateMetadataVersion = Date(Timestamp(metadataVersion).time)
            popup_info_metadata_version.text = getString(
                R.string.popup_info_metadata_version,
                if (dateMetadataVersion.time != 0L) dateMetadataVersion else resources.getString(R.string.pop_up_info_never_synced)
            )
        }

        if (contentVersionError == null) {
            popup_info_content_version.text = getString(
                R.string.popup_info_content_version,
                "Error"
            )
        } else {
            val dateContentVersion = Date(Timestamp(contentVersionError).time)
            popup_info_content_version.text = getString(
                R.string.popup_info_content_version,
                if (dateContentVersion.time != 0L) dateContentVersion else resources.getString(R.string.pop_up_info_never_synced)
            )
        }

        popup_info_name.text = getString(R.string.popup_info_name, name)
        popup_info_id.text = getString(R.string.popup_info_id, id)
        popup_info_file_type.text =
            getString(R.string.popup_info_file_type, fileType)
    }

    private fun rename() {
        intent.putExtra("id", id)
        intent.putExtra("new_name", new_name_text.text.toString())
        setResult(RENAME_RESULT_CODE, intent)
        finish()
    }

    private fun delete() {
        intent.putExtra("id", id)
        setResult(DELETE_RESULT_CODE, intent)
        finish()
    }
}
