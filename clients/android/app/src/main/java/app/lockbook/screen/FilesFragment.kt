package app.lockbook.screen

import android.os.Bundle
import android.os.Handler
import android.os.Looper
import androidx.activity.result.contract.ActivityResultContracts
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.fragment.app.viewModels
import app.lockbook.R
import app.lockbook.databinding.FragmentFilesBinding
import app.lockbook.model.*
import app.lockbook.util.ClientFileMetadata
import app.lockbook.util.FileType
import app.lockbook.util.HorizontalViewHolder
import com.afollestad.recyclical.setup
import com.afollestad.recyclical.withItem
import com.tingyik90.snackprogressbar.SnackProgressBar
import com.tingyik90.snackprogressbar.SnackProgressBarManager
import java.lang.ref.WeakReference
import java.util.*

class FilesFragment: Fragment() {
    private var _binding: FragmentFilesBinding? = null
    private val binding get() = _binding!!

    private val model: FilesViewModel by viewModels()
    private val activityModel: StateViewModel by activityViewModels()

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()))
    }

    private var updatedLastSyncedDescription = Timer()
    private val handler = Handler(requireNotNull(Looper.myLooper()))

    private val snackProgressBarManager by lazy {
        SnackProgressBarManager(
            requireView(),
            lifecycleOwner = this
        ).setViewToMove(binding.listFilesFrameLayout)
    }

    private val syncSnackProgressBar by lazy {
        SnackProgressBar(
            SnackProgressBar.TYPE_HORIZONTAL,
            resources.getString(R.string.list_files_sync_snackbar_default)
        )
            .setIsIndeterminate(false)
            .setSwipeToDismiss(false)
            .setAllowUserInput(true)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        binding.filesList.setup {
            withItem<ClientFileMetadata, HorizontalViewHolder>(R.layout.linear_layout_file_item) {
                onBind(::HorizontalViewHolder) { _, item ->
                    name.text = item.name
                    description.text = resources.getString(
                        R.string.last_synced,
                        CoreModel.convertToHumanDuration(item.metadataVersion)
                    )

                    when {
                        item.fileType == FileType.Document && item.name.endsWith(".draw") -> {
                            icon.setImageResource(R.drawable.ic_baseline_border_color_24)
                        }
                        item.fileType == FileType.Document -> {
                            icon.setImageResource(R.drawable.ic_baseline_insert_drive_file_24)
                        }
                        else -> {
                            icon.setImageResource(R.drawable.round_folder_white_18dp)
                        }
                    }
                }
                onClick {
                    val detailsScreen = when(item.fileType) {
                        FileType.Document -> {
                            if(file.name.endsWith(".draw")) {
                                DetailsScreen.Drawing
                            } else {
                                DetailsScreen.TextEditor
                            }
                        }
                        FileType.Folder -> {
                            model.ent
                        }
                        null -> {
                            DetailsScreen.Blank
                        }
                    }

                    activityModel._launchDetailsScreen.value = detailsScreen
                }
            }
        }

        binding.listFilesRefresh.setOnRefreshListener {
            model.onSwipeToRefresh()
        }

        updatedLastSyncedDescription.schedule(
            object : TimerTask() {
                override fun run() {
                    handler.post {
                        adapter.notifyDataSetChanged()
                    }
                }
            },
            30000,
            30000
        )

        binding.fabsNewFile.listFilesFab.setOnClickListener {
            collapseExpandFAB()
        }

        binding.fabsNewFile.listFilesFabFolder.setOnClickListener {
            onFolderFabClicked()
        }

        binding.fabsNewFile.listFilesFabDocument.setOnClickListener {
            onDocumentFabClicked(false)
        }

        binding.fabsNewFile.listFilesFabDrawing.setOnClickListener {
            onDocumentFabClicked(true)
        }
    }

    private fun onDocumentFabClicked(isDrawing: Boolean) {
        val ts = TransientScreen.Create(CreateFileInfo())
    }

    private fun onFolderFabClicked() {

    }

    private fun collapseExpandFAB() {
        if (binding.fabsNewFile.listFilesFabDocument.isOrWillBeHidden) {
            showFABMenu()
        } else {
            closeFABMenu()
        }
    }

    private fun closeFABMenu() {
        val fabsNewFile = binding.fabsNewFile
        fabsNewFile.listFilesFab.animate().setDuration(200L).rotation(90f)
        fabsNewFile.listFilesFab.setImageResource(R.drawable.ic_baseline_add_24)
        fabsNewFile.listFilesFabFolder.hide()
        fabsNewFile.listFilesFabDocument.hide()
        fabsNewFile.listFilesFabDrawing.hide()
        binding.listFilesRefresh.alpha = 1f
        binding.listFilesFrameLayout.isClickable = false
    }

    private fun showFABMenu() {
        val fabsNewFile = binding.fabsNewFile
        fabsNewFile.listFilesFab.animate().setDuration(200L).rotation(-90f)
        fabsNewFile.listFilesFabFolder.show()
        fabsNewFile.listFilesFabDocument.show()
        fabsNewFile.listFilesFabDrawing.show()
        binding.listFilesRefresh.alpha = 0.7f
        binding.listFilesFrameLayout.isClickable = true
        binding.listFilesFrameLayout.setOnClickListener {
            closeFABMenu()
        }
    }
}

sealed class UpdateFilesUI {
    data class Files(val files: List<ClientFileMetadata>): UpdateFilesUI()
    object StopProgressSpinner: UpdateFilesUI()
}