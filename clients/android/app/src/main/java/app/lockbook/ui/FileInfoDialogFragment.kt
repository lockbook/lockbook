package app.lockbook.ui

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import androidx.fragment.app.DialogFragment
import app.lockbook.R
import app.lockbook.databinding.DialogFileInfoBinding
import java.sql.Timestamp
import java.util.*

class FileInfoDialogFragment : DialogFragment() {
    private var _binding: DialogFileInfoBinding? = null
    // This property is only valid between onCreateView and
    // onDestroyView.
    private val binding get() = _binding!!

    companion object {

        const val FILE_INFO_DIALOG_TAG = "FileInfoDialogFragment"

        private const val NAME_KEY = "NAME_KEY"
        private const val ID_KEY = "ID_KEY"
        private const val METADATA_VERSION_KEY = "METADATA_VERSION_KEY"
        private const val CONTENT_VERSION_KEY = "CONTENT_VERSION_KEY"
        private const val FILE_TYPE_KEY = "FILE_TYPE_KEY"

        fun newInstance(name: String, id: String, metadataVersion: String, contentVersion: String, fileType: String): FileInfoDialogFragment {
            val args = Bundle()
            args.putString(NAME_KEY, name)
            args.putString(ID_KEY, id)
            args.putString(METADATA_VERSION_KEY, metadataVersion)
            args.putString(CONTENT_VERSION_KEY, contentVersion)
            args.putString(FILE_TYPE_KEY, fileType)

            val fragment = FileInfoDialogFragment()
            fragment.arguments = args
            return fragment
        }
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = DialogFileInfoBinding.inflate(
            inflater,
            container,
            false
        )

        return binding.root
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)
        val bundle = arguments
        if (bundle != null) {
            setUpInfo(bundle)
        }
    }

    override fun onStart() {
        super.onStart()
        dialog?.window?.setLayout(
            (resources.displayMetrics.widthPixels * 0.9).toInt(),
            WindowManager.LayoutParams.WRAP_CONTENT
        )
    }

    private fun setUpInfo(bundle: Bundle) {
        val name = bundle.getString(NAME_KEY)
        val id = bundle.getString(ID_KEY)
        val tempMetadataVersion = bundle.getString(METADATA_VERSION_KEY)
        val tempContentVersion = bundle.getString(CONTENT_VERSION_KEY)
        val fileType = bundle.getString(FILE_TYPE_KEY)
        val metadataVersion = tempMetadataVersion?.toLongOrNull()
        val contentVersionError = tempContentVersion?.toLongOrNull()
        binding.popupInfoMetadataVersion.text = if (metadataVersion == null) {
            getString(
                R.string.popup_info_metadata_version,
                "ERROR"
            )
        } else {
            val dateMetadataVersion = Date(Timestamp(metadataVersion).time)
            getString(
                R.string.popup_info_metadata_version,
                if (dateMetadataVersion.time != 0L) dateMetadataVersion else resources.getString(R.string.pop_up_info_never_synced)
            )
        }

        binding.popupInfoContentVersion.text = if (contentVersionError == null) {
            getString(
                R.string.popup_info_content_version,
                "Error"
            )
        } else {
            val dateContentVersion = Date(Timestamp(contentVersionError).time)
            getString(
                R.string.popup_info_content_version,
                if (dateContentVersion.time != 0L) dateContentVersion else resources.getString(R.string.pop_up_info_never_synced)
            )
        }

        binding.popupInfoName.text = getString(R.string.popup_info_name, name)
        binding.popupInfoId.text = getString(R.string.popup_info_id, id)
        binding.popupInfoFileType.text =
            getString(R.string.popup_info_file_type, fileType)
    }
}
