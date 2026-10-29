package app.lockbook.ui

import android.app.Dialog
import android.os.Bundle
import androidx.fragment.app.DialogFragment
import androidx.lifecycle.asLiveData
import app.lockbook.R
import app.lockbook.databinding.DialogFileInfoBinding
import app.lockbook.model.FileMetadataStore
import app.lockbook.util.withFileMetadata
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import net.lockbook.File
import net.lockbook.Lb

class FileInfoDialogFragment : DialogFragment() {
    private lateinit var binding: DialogFileInfoBinding

    companion object {
        const val TAG = "FileInfoDialogFragment"
        private const val FILE_ID_KEY = "file_id"

        fun newInstance(fileId: String): FileInfoDialogFragment =
            FileInfoDialogFragment().apply {
                arguments = Bundle().apply { putString(FILE_ID_KEY, fileId) }
            }
    }

    override fun onCreateDialog(savedInstanceState: Bundle?): Dialog =
        MaterialAlertDialogBuilder(requireContext(), theme)
            .setTitle(R.string.popup_info_title)
            .apply {
                binding = DialogFileInfoBinding.inflate(layoutInflater)
                FileMetadataStore.snapshots.asLiveData().observe(this@FileInfoDialogFragment) { snapshot ->
                    if (snapshot != null) {
                        val file = snapshot.idsAndFiles[requireArguments().getString(FILE_ID_KEY)]
                        if (file != null) setUpInfo(file) else dismissAllowingStateLoss()
                    }
                }
                setView(binding.root)
                setPositiveButton(R.string.done, null)
            }.create()
            .apply {
                setCanceledOnTouchOutside(false)
                withFileMetadata { setUpInfo(FileMetadataStore.requireFile(requireArguments().getString(FILE_ID_KEY)!!)) }
            }

    private fun setUpInfo(file: File) {
        binding.popupInfoLastModified.text = Lb.getTimestampHumanString(file.lastModified)
        binding.popupInfoName.text = file.name
        binding.popupInfoId.text = file.id
        binding.popupInfoFileType.text = file.type.name
    }
}
