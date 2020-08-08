package app.lockbook.loggedin.popupinfo

import android.app.Activity
import android.os.Bundle
import app.lockbook.R
import app.lockbook.utils.RequestResultCodes.DELETE_RESULT_CODE
import app.lockbook.utils.RequestResultCodes.RENAME_RESULT_CODE
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
        val id = intent.getStringExtra("id") ?: "ERROR"
        val tempMetadataVersion = intent.getStringExtra("metadataVersion") ?: "ERROR"
        val tempContentVersion = intent.getStringExtra("contentVersion") ?: "ERROR"
        val dateMetadataVersion = Date(Timestamp(tempMetadataVersion.toLongOrNull() ?: 0L).time)
        val dateContentVersion = Date(Timestamp(tempContentVersion.toLongOrNull() ?: 0L).time)

        popup_info_name.text = getString(R.string.popup_info_name, intent.getStringExtra("name"))
        popup_info_id.text = getString(R.string.popup_info_id, id)
        popup_info_file_type.text =
            getString(R.string.popup_info_file_type, intent.getStringExtra("fileType"))
        popup_info_metadata_version.text = getString(
            R.string.popup_info_metadata_version,
            dateMetadataVersion
        )
        popup_info_content_version.text =
            getString(R.string.popup_info_content_version, dateContentVersion)
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
