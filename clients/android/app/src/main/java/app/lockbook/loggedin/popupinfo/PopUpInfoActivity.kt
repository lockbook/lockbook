package app.lockbook.loggedin.popupinfo

import android.app.Activity
import android.os.Bundle
import app.lockbook.R
import app.lockbook.utils.RequestResultCodes.DELETE_RESULT_CODE
import app.lockbook.utils.RequestResultCodes.RENAME_RESULT_CODE
import kotlinx.android.synthetic.main.activity_popup_info.*

class PopUpInfoActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        popup_info_delete.setOnClickListener {
            rename()
        }

        popup_info_rename.setOnClickListener {
            delete()
        }

        setUpInfo()
    }

    private fun setUpInfo() {
        popup_info_name.text = getString(R.string.popup_info_name, intent.getStringExtra("name"))
        popup_info_id.text = getString(R.string.popup_info_id, intent.getStringExtra("id"))
        popup_info_file_type.text = getString(R.string.popup_info_file_type, intent.getStringExtra("fileType"))
        popup_info_metadata_version.text = getString(R.string.popup_info_metadata_version, intent.getStringExtra("metadataVersion"))
        popup_info_content_version.text = getString(R.string.popup_info_content_version, intent.getStringExtra("contentVersion"))
    }

    fun rename() {
        intent.putExtra("new_name", new_name_text.text.toString())
        intent.putExtra("id", popup_info_id.text.toString())
        setResult(RENAME_RESULT_CODE, intent)
        finish()
    }

    fun delete() {
        intent.putExtra("id", popup_info_id.text.toString())
        setResult(DELETE_RESULT_CODE, intent)
        finish()
    }

}