package app.lockbook.screen

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.fragment.app.Fragment
import androidx.navigation.fragment.findNavController
import app.lockbook.R
import app.lockbook.databinding.FragmentSharedFilesBinding
import app.lockbook.model.*
import app.lockbook.ui.DeleteSharedDialogFragment
import app.lockbook.util.*
import com.afollestad.recyclical.datasource.emptyDataSourceTyped
import com.afollestad.recyclical.setup
import com.afollestad.recyclical.withItem
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import java.lang.ref.WeakReference

class SharedFilesFragment : Fragment() {
    lateinit var binding: FragmentSharedFilesBinding

    private val uiScope = CoroutineScope(Dispatchers.Main + Job())

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()), view)
    }

    private val sharedFilesDataSource = emptyDataSourceTyped<File>()

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        binding = FragmentSharedFilesBinding.inflate(inflater, container, false)

        populatePendingShares()

        binding.sharedFiles.setup {
            withDataSource(sharedFilesDataSource)

            withItem<File, SharedFileViewHolder>(R.layout.recent_file_item) {
                onBind(::SharedFileViewHolder) { _, item ->
                    name.text = item.name
                    owner.text = item.shares[0].sharedBy

                    val iconResource = when(item.fileType) {
                        FileType.Document -> {
                            val extensionHelper = ExtensionHelper(item.name)

                            when {
                                extensionHelper.isDrawing -> R.drawable.ic_outline_draw_24
                                extensionHelper.isImage -> R.drawable.ic_outline_image_24
                                extensionHelper.isPdf -> R.drawable.ic_outline_picture_as_pdf_24
                                else -> R.drawable.ic_outline_insert_drive_file_24
                            }
                        }
                        FileType.Folder -> R.drawable.ic_baseline_folder_24
                    }

                    addShared.setOnClickListener {
                        val bundle = Bundle()
                        bundle.putParcelable(CreateLinkFragment.CREATE_LINK_FILE_KEY, item)
                        findNavController().navigate(R.id.action_create_link, bundle)
                    }

                    deleteShared.setOnClickListener {
                        DeleteSharedDialogFragment.newInstance(arrayListOf(item)).show(
                            requireActivity().supportFragmentManager,
                            DeleteSharedDialogFragment.DELETE_SHARED_DIALOG_FRAGMENT
                        )
                    }

                    icon.setImageResource(iconResource)
                }
            }
        }

        binding.sharedFilesToolbar.setOnMenuItemClickListener { item ->
            when (item.itemId) {
                R.id.menu_shared_files_reject_all -> {
                    DeleteSharedDialogFragment.newInstance(ArrayList(sharedFilesDataSource.toList())).show(
                        requireActivity().supportFragmentManager,
                        DeleteSharedDialogFragment.DELETE_SHARED_DIALOG_FRAGMENT
                    )
                }
            }

            true
        }

        return binding.root
    }

    private fun populatePendingShares() {
        uiScope.launch(Dispatchers.IO) {
            when (val getPendingSharesResults = CoreModel.getPendingShares()) {
                is Ok -> {
                    if(getPendingSharesResults.value.isEmpty()) {
                        binding.sharedFilesNone.visibility = View.VISIBLE
                    } else {
                        sharedFilesDataSource.set(getPendingSharesResults.value)
                    }
                }
                is Err -> alertModel.notifyError(getPendingSharesResults.error.toLbError(resources)) {
                    requireActivity().finish()
                }
            }
        }
    }
}
