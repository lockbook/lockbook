package app.lockbook.screen

import android.content.ClipData
import android.content.Intent
import android.content.res.Configuration.*
import android.net.Uri
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.FileProvider
import androidx.fragment.app.Fragment
import androidx.fragment.app.FragmentManager
import androidx.fragment.app.viewModels
import androidx.lifecycle.ViewModelProvider
import androidx.preference.PreferenceManager
import androidx.recyclerview.widget.GridLayoutManager
import androidx.recyclerview.widget.LinearLayoutManager
import app.lockbook.R
import app.lockbook.databinding.FragmentListFilesBinding
import app.lockbook.model.*
import app.lockbook.ui.*
import app.lockbook.util.*
import com.tingyik90.snackprogressbar.SnackProgressBar
import com.tingyik90.snackprogressbar.SnackProgressBarManager
import timber.log.Timber
import java.io.File
import java.lang.ref.WeakReference
import java.util.*

class ListFilesFragment : Fragment() {
    private var _binding: FragmentListFilesBinding? = null

    // This property is only valid between onCreateView and
    // onDestroyView.
    private val binding get() = _binding!!

    private val model: ListFilesViewModel by viewModels()

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()))
    }

    private var onShareResult =
        registerForActivityResult(ActivityResultContracts.StartActivityForResult()) {
            getListFilesActivity().showHideProgressOverlay(false)
            model.shareModel.isLoadingOverlayVisible = false
        }

    private var updatedLastSyncedDescription = Timer()
    private val handler = Handler(requireNotNull(Looper.myLooper()))
    private val fragmentFinishedCallback = object : FragmentManager.FragmentLifecycleCallbacks() {
        override fun onFragmentDestroyed(fm: FragmentManager, f: Fragment) {
            if (f is CreateFileDialogFragment) {
                model.onCreateFileDialogEnded(f.newDocument)
            } else {
                model.onCreateFileDialogEnded(null)
            }
        }
    }
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

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = FragmentListFilesBinding.inflate(
            inflater,
            container,
            false
        )

        var adapter = setFileAdapter()

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
            model.collapseExpandFAB()
        }

        binding.fabsNewFile.listFilesFabFolder.setOnClickListener {
            model.onNewFolderFABClicked()
        }

        binding.fabsNewFile.listFilesFabDocument.setOnClickListener {
            model.onNewDocumentFABClicked(false)
        }

        binding.fabsNewFile.listFilesFabDrawing.setOnClickListener {
            model.onNewDocumentFABClicked(true)
        }

        model.files.observe(
            viewLifecycleOwner,
            { files ->
                updateFilesList(files, adapter)
            }
        )

        model.stopProgressSpinner.observe(
            viewLifecycleOwner,
            {
                binding.listFilesRefresh.isRefreshing = false
            }
        )

        model.showSyncSnackBar.observe(
            viewLifecycleOwner,
            {
                showSyncSnackBar()
            }
        )

        model.switchFileLayout.observe(
            viewLifecycleOwner,
            {
                adapter = setFileAdapter(adapter)
            }
        )

        model.expandCloseMenu.observe(
            viewLifecycleOwner,
            { expandOrNot ->
                moreOptionsMenu(expandOrNot)
            }
        )

        model.collapseExpandFAB.observe(
            viewLifecycleOwner,
            { isFABOpen ->
                collapseExpandFAB(isFABOpen)
            }
        )

        model.uncheckAllFiles.observe(
            viewLifecycleOwner,
            {
                unSelectAllFiles(adapter)
            }
        )

        model.updateBreadcrumbBar.observe(
            viewLifecycleOwner,
            { path ->
                binding.filesBreadcrumbBar.setBreadCrumbItems(path.toMutableList())
            }
        )

        model.notifyWithSnackbar.observe(
            viewLifecycleOwner,
            { msg ->
                if (container != null) {
                    snackProgressBarManager.dismiss()
                    alertModel.notify(msg)
                }
            }
        )

        model.shareDocument.observe(
            viewLifecycleOwner,
            { files ->
                shareDocuments(files)
            }
        )

        model.updateSyncSnackBar.observe(
            viewLifecycleOwner,
            { progressAndTotal ->
                updateProgressSnackBar(progressAndTotal.first, progressAndTotal.second)
            }
        )

        model.showHideProgressOverlay.observe(
            viewLifecycleOwner,
            { show ->
                showHideProgressOverlay(show)
            }
        )

        model.notifyError.observe(
            viewLifecycleOwner,
            { error ->
                if (container != null) {
                    alertModel.notifyError(error)
                }
            }
        )

        return binding.root
    }

    private fun showHideProgressOverlay(show: Boolean) {
        if (show) {
            model.collapseMoreOptionsMenu()
        }
        getListFilesActivity().showHideProgressOverlay(show)
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        binding.filesBreadcrumbBar.setListener(object : BreadCrumbItemClickListener {
            override fun onItemClick(breadCrumbItem: View, position: Int) {
                model.refreshAtPastParent(position)
            }
        })

        snackProgressBarManager.useRoundedCornerBackground(true)

        setUpAfterConfigChange()
    }

    override fun onDestroy() {
        super.onDestroy()
        parentFragmentManager.unregisterFragmentLifecycleCallbacks(fragmentFinishedCallback)
    }

    fun onBackPressed(): Boolean {
        return model.onBackPress()
    }

    fun onMenuItemPressed(id: Int) {
        model.onMenuItemPressed(id)
    }

    private fun setFileAdapter(oldAdapter: GeneralViewAdapter? = null): GeneralViewAdapter {
        if (binding.filesList.adapter is GeneralViewAdapter) {
            Timber.e("SET FILE ADAPTER: ${oldAdapter?.files?.map { it.name }}")

            Timber.e("CURRENT: ${(binding.filesList.adapter as GeneralViewAdapter).files.map { it.name }}")
        }
        val deviceConfig = resources.configuration

        val linearLayoutValue = getString(R.string.file_layout_linear_value)
        val gridLayoutValue = getString(R.string.file_layout_grid_value)

        val fileLayoutPreference = PreferenceManager
            .getDefaultSharedPreferences(context)
            .getString(
                getString(R.string.file_layout_key),
                if (deviceConfig.isLayoutSizeAtLeast(SCREENLAYOUT_SIZE_LARGE) || (deviceConfig.screenWidthDp >= 480 && deviceConfig.screenHeightDp >= 640)) {
                    gridLayoutValue
                } else {
                    linearLayoutValue
                }
            )

        if (fileLayoutPreference == linearLayoutValue) {
            val adapter = LinearRecyclerViewAdapter(model)
            if (oldAdapter != null) {
                adapter.files = oldAdapter.files
            }

            binding.filesList.adapter = adapter
            binding.filesList.layoutManager = LinearLayoutManager(context)
            return adapter
        } else {
            val orientation = deviceConfig.orientation
            val adapter = GridRecyclerViewAdapter(model)
            if (oldAdapter != null) {
                adapter.files = oldAdapter.files
            }
            binding.filesList.adapter = adapter

            val displayMetrics = resources.displayMetrics
            val noOfColumns = (((displayMetrics.widthPixels / displayMetrics.density) / 90)).toInt()

            if (orientation == ORIENTATION_PORTRAIT) {
                binding.filesList.layoutManager = GridLayoutManager(context, noOfColumns)
            } else {
                binding.filesList.layoutManager = GridLayoutManager(context, noOfColumns)
            }

            return adapter
        }
    }

    private fun unSelectAllFiles(adapter: GeneralViewAdapter) {
        adapter.clearSelectionMode()
    }

    private fun setUpAfterConfigChange() {
        collapseExpandFAB(model.isFABOpen)

        val syncStatus = model.syncModel.syncStatus
        if (syncStatus is SyncStatus.IsSyncing) {
            showSyncSnackBar()
            updateProgressSnackBar(syncStatus.total, syncStatus.progress)
        }

        val isLoadingOverlayVisible = model.shareModel.isLoadingOverlayVisible
        if (isLoadingOverlayVisible) {
            showHideProgressOverlay(isLoadingOverlayVisible)
        }

        parentFragmentManager.registerFragmentLifecycleCallbacks(
            fragmentFinishedCallback,
            false
        )
    }

    private fun updateProgressSnackBar(total: Int, progress: Int) {
        syncSnackProgressBar.setProgressMax(total)
        snackProgressBarManager.setProgress(progress)
        syncSnackProgressBar.setMessage(
            resources.getString(
                R.string.list_files_sync_snackbar,
                total.toString()
            )
        )
        snackProgressBarManager.updateTo(syncSnackProgressBar)
    }

    private fun showSyncSnackBar() {
        snackProgressBarManager.dismiss()
        syncSnackProgressBar.setMessage(resources.getString(R.string.list_files_sync_snackbar_default))
        snackProgressBarManager.show(
            syncSnackProgressBar,
            SnackProgressBarManager.LENGTH_INDEFINITE
        )
    }

    private fun updateFilesList(
        files: List<ClientFileMetadata>,
        adapter: GeneralViewAdapter
    ) {
        adapter.files = files
        adapter.selectedFiles = model.selectedFiles.toMutableList()
        if (adapter.selectedFiles.isNotEmpty()) {
            adapter.selectionMode = true
        }

        if (files.isEmpty()) {
            binding.listFilesEmptyFolder.visibility = View.VISIBLE
        } else if (files.isNotEmpty() && binding.listFilesEmptyFolder.visibility == View.VISIBLE) {
            binding.listFilesEmptyFolder.visibility = View.GONE
        }
    }

    private fun moreOptionsMenu(expandOrNot: Boolean) {
        getListFilesActivity().switchMenu(expandOrNot)
    }

    private fun getListFilesActivity(): ListFilesActivity {
        return activity as ListFilesActivity
    }
}
