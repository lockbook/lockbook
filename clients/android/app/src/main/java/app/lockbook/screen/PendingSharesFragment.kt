package app.lockbook.screen

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.fragment.app.Fragment
import androidx.fragment.app.FragmentManager
import androidx.navigation.fragment.findNavController
import app.lockbook.R
import app.lockbook.databinding.FragmentPendingSharesBinding
import app.lockbook.model.*
import app.lockbook.ui.*
import app.lockbook.util.*
import com.afollestad.recyclical.datasource.emptyDataSourceTyped
import com.afollestad.recyclical.setup
import com.afollestad.recyclical.withItem
import kotlinx.coroutines.*
import net.lockbook.File
import net.lockbook.File.FileType
import net.lockbook.Lb
import net.lockbook.LbError
import java.lang.ref.WeakReference

class PendingSharesFragment : Fragment() {
    lateinit var binding: FragmentPendingSharesBinding

    private val uiScope = CoroutineScope(Dispatchers.Main + Job())

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()), view)
    }

    private val sharedFilesDataSource = emptyDataSourceTyped<File>()

    private val fragmentFinishedCallback = object : FragmentManager.FragmentLifecycleCallbacks() {
        override fun onFragmentDestroyed(fm: FragmentManager, f: Fragment) {
            populatePendingShares()
        }
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        binding = FragmentPendingSharesBinding.inflate(inflater, container, false)

        binding.sharedFilesToolbar.setNavigationOnClickListener {
            activity?.onBackPressed()
        }

        populatePendingShares()

        binding.sharedFiles.setup {
            withDataSource(sharedFilesDataSource)

            withItem<File, SharedFileViewHolder>(R.layout.pending_shares_file_item) {
                onBind(::SharedFileViewHolder) { _, item ->
                    name.text = item.name
                    owner.text = item.lastModifiedBy

                    val iconResource = when (item.type) {
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
                        FileType.Link -> R.drawable.ic_baseline_miscellaneous_services_24
                    }

                    addShared.setOnClickListener {
                        val bundle = Bundle()
                        bundle.putString(CreateLinkFragment.CREATE_LINK_FILE_ID_KEY, item.id)
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
                    if (sharedFilesDataSource.isEmpty()) {
                        alertModel.notify(getString(R.string.no_pending_shares))
                    } else {
                        DeleteSharedDialogFragment.newInstance(ArrayList(sharedFilesDataSource.toList())).show(
                            requireActivity().supportFragmentManager,
                            DeleteSharedDialogFragment.DELETE_SHARED_DIALOG_FRAGMENT
                        )
                    }
                }
            }

            true
        }

        requireActivity().supportFragmentManager.registerFragmentLifecycleCallbacks(
            fragmentFinishedCallback,
            false
        )

        return binding.root
    }

    override fun onResume() {
        super.onResume()
        populatePendingShares()
    }

    override fun onDestroy() {
        super.onDestroy()
        requireActivity().supportFragmentManager.unregisterFragmentLifecycleCallbacks(fragmentFinishedCallback)
    }

    private fun populatePendingShares() {
        uiScope.launch(Dispatchers.IO) {

            try {
                val pendingShares = Lb.getPendingShares()

                withContext(Dispatchers.Main) {
                    sharedFilesDataSource.set(FileModel.sortFiles(pendingShares.toList()))

                    if (pendingShares.isEmpty()) {
                        binding.sharedFilesNone.visibility = View.VISIBLE
                    }
                }
            } catch (err: LbError) {
                alertModel.notifyError(err) {
                    activity?.onBackPressed()
                }
            }
        }
    }
}
