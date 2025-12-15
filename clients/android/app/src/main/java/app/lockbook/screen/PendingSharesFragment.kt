package app.lockbook.screen

import android.annotation.SuppressLint
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.core.view.setPadding
import androidx.fragment.app.Fragment
import androidx.fragment.app.FragmentManager
import androidx.lifecycle.MutableLiveData
import androidx.navigation.fragment.findNavController
import androidx.viewpager2.adapter.FragmentStateAdapter
import app.lockbook.R
import app.lockbook.databinding.FragmentPendingSharesBinding
import app.lockbook.databinding.FragmentTabBinding
import app.lockbook.model.*
import app.lockbook.ui.*
import app.lockbook.util.*
import com.afollestad.recyclical.datasource.dataSourceOf
import com.afollestad.recyclical.datasource.emptyDataSourceTyped
import com.afollestad.recyclical.setup
import com.afollestad.recyclical.withItem
import com.google.android.material.tabs.TabLayoutMediator
import kotlinx.coroutines.*
import net.lockbook.File
import net.lockbook.File.FileType
import net.lockbook.Lb
import net.lockbook.LbError
import java.lang.ref.WeakReference
import java.util.Locale
import kotlin.collections.emptyList

class PendingSharesFragment : Fragment() {
    lateinit var binding: FragmentPendingSharesBinding

    private val uiScope = CoroutineScope(Dispatchers.Main + Job())

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()), view)
    }

    val filesBySharer = MutableLiveData<List<Pair<String, List<File>>>>()

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


        populatePendingShares()

        val tabLayout = binding.tabLayout
        val viewPager = binding.viewPager


        filesBySharer.observe(this, { filesMap ->
            if (filesMap.isEmpty()){
                binding.pendingSharesEmptyState.visibility = View.VISIBLE
                binding.tabsContainer.visibility = View.GONE
            }else{
                binding.pendingSharesEmptyState.visibility = View.GONE
                binding.tabsContainer.visibility = View.VISIBLE

                val tabs = filesMap.map { (sharer, _) -> sharer }
                val adapter = TabPagerAdapter(this, tabs)
                viewPager.adapter = adapter
                TabLayoutMediator(tabLayout, viewPager) { tab, position ->
                    tab.text = tabs[position]
                }.attach()
            }

        })




//        binding.sharedFiles.setup {
//            withDataSource(sharedFilesDataSource)
//
//            withItem<File, SharedFileViewHolder>(R.layout.pending_shares_file_item) {
//                onBind(::SharedFileViewHolder) { _, item ->
//                    name.text = item.name
//                    owner.text = item.lastModifiedBy
//
//                    val iconResource = when (item.type) {
//                        FileType.Document -> {
//                            val extensionHelper = ExtensionHelper(item.name)
//
//                            when {
//                                extensionHelper.isDrawing -> R.drawable.ic_outline_draw_24
//                                extensionHelper.isImage -> R.drawable.ic_outline_image_24
//                                extensionHelper.isPdf -> R.drawable.ic_outline_picture_as_pdf_24
//                                else -> R.drawable.ic_outline_insert_drive_file_24
//                            }
//                        }
//                        FileType.Folder -> R.drawable.ic_baseline_folder_24
//                        FileType.Link -> R.drawable.ic_baseline_miscellaneous_services_24
//                    }
//
//                    addShared.setOnClickListener {
//                        val bundle = Bundle()
//                        bundle.putString(CreateLinkFragment.CREATE_LINK_FILE_ID_KEY, item.id)
//                        findNavController().navigate(R.id.action_create_link, bundle)
//                    }
//
//                    deleteShared.setOnClickListener {
//                        DeleteSharedDialogFragment.newInstance(arrayListOf(item)).show(
//                            requireActivity().supportFragmentManager,
//                            DeleteSharedDialogFragment.DELETE_SHARED_DIALOG_FRAGMENT
//                        )
//                    }
//
//                    icon.setImageResource(iconResource)
//                }
//            }
//        }

//        binding.sharedFilesToolbar.setOnMenuItemClickListener { item ->
//            when (item.itemId) {
//                R.id.menu_shared_files_reject_all -> {
//                    if (sharedFilesDataSource.isEmpty()) {
//                        alertModel.notify(getString(R.string.no_pending_shares))
//                    } else {
//                        DeleteSharedDialogFragment.newInstance(ArrayList(sharedFilesDataSource.toList())).show(
//                            requireActivity().supportFragmentManager,
//                            DeleteSharedDialogFragment.DELETE_SHARED_DIALOG_FRAGMENT
//                        )
//                    }
//                }
//            }
//
//            true
//        }

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
                val pendingShares = Lb.getPendingShareFiles().toList()

                withContext(Dispatchers.Main) {
                    val newFilesBySharer = pendingShares
                        .filterNotNull()
                        .flatMap { file ->
                            file.shares.map { share -> share.sharedBy.capitalized() to file }.distinct()
                        }
                        .groupBy(
                            keySelector = { (sharer, _) -> sharer },
                            valueTransform = { (_, file) -> file }
                        ).toList()

                    if (!pendingShares.isEmpty()){
                        val allSharesEntry = "All" to pendingShares

                        filesBySharer.value = listOf(allSharesEntry) + newFilesBySharer
                    }else{
                        filesBySharer.value = emptyList()
                    }


                    if (pendingShares.isEmpty()) {
//                        binding.sharedFilesNone.visibility = View.VISIBLE
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

class TabPagerAdapter(
    activity: PendingSharesFragment,
    private val tabs: List<String>
) : FragmentStateAdapter(activity) {

    override fun getItemCount(): Int = tabs.size

    override fun createFragment(position: Int): Fragment {
        return TabFragment.newInstance(position)
    }
}

class TabFragment : Fragment() {
    private var _binding: FragmentTabBinding? = null
    private val binding get() = _binding!!

    @SuppressLint("SetTextI18n")
    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = FragmentTabBinding.inflate(inflater, container, false)

        binding.sharedFilesList.setup {
            val pendingSharesFragment = parentFragment as? PendingSharesFragment
            val tabIndex = arguments?.getInt("tab_index") ?: -1

            val files = pendingSharesFragment?.filesBySharer?.value?.get(tabIndex)?.second

            withDataSource(dataSourceOf(files ?: emptyList()))

            withItem<File, BasicFileItemHolder>(R.layout.move_file_item) {
                onBind(::BasicFileItemHolder) { _, item ->
                    name.text = item.name
                    val extensionHelper = ExtensionHelper(item.name)

                    val imageResource = when {
                        item.type == FileType.Document && extensionHelper.isDrawing -> {
                            R.drawable.ic_outline_draw_24
                        }
                        item.type == FileType.Document && extensionHelper.isImage -> {
                            R.drawable.ic_outline_image_24
                        }
                        item.type == FileType.Document -> {
                            R.drawable.ic_outline_insert_drive_file_24
                        }
                        item.type == FileType.Document && extensionHelper.isPdf -> {
                            R.drawable.ic_outline_picture_as_pdf_24
                        }
                        else -> {
                            R.drawable.ic_baseline_folder_24
                        }
                    }

                    icon.setImageResource(imageResource)
                }
                onClick {
//                    model.onItemClick(item)
                }
            }
        }

        return binding.root
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)

        val sharedFilesList = binding.sharedFilesList

        (activity as? BottomNavProvider)?.doWhenBottomNavMeasured { bottomNavHeight ->
            sharedFilesList.setPadding(
                sharedFilesList.paddingLeft,
                sharedFilesList.paddingTop,
                sharedFilesList.paddingRight,
                bottomNavHeight
            )
        }

    }

    override fun onDestroyView() {
        super.onDestroyView()
        _binding = null
    }

    companion object {
        fun newInstance(tabIndex: Int): TabFragment {
            val fragment = TabFragment()
            val args = Bundle()
            args.putInt("tab_index", tabIndex)
            fragment.arguments = args
            return fragment
        }
    }
}

fun String.capitalized(): String {
    return this.replaceFirstChar {
        if (it.isLowerCase())
            it.titlecase(Locale.getDefault())
        else it.toString()
    }
}