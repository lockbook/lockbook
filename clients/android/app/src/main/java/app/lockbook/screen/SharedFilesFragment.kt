package app.lockbook.screen

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.recyclerview.widget.LinearLayoutManager
import app.lockbook.R
import app.lockbook.databinding.FragmentImageViewerBinding
import app.lockbook.databinding.FragmentSharedFilesBinding
import app.lockbook.model.*
import app.lockbook.util.*
import com.afollestad.recyclical.datasource.DataSource
import com.afollestad.recyclical.datasource.emptyDataSourceTyped
import com.afollestad.recyclical.setup
import com.afollestad.recyclical.withItem

class SharedFilesFragment : Fragment() {
    private var _binding: FragmentSharedFilesBinding? = null
    private val binding get() = _binding!!

    private val activityModel: StateViewModel by activityViewModels()

    val sharedFiles = emptyDataSourceTyped<File>()

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = FragmentSharedFilesBinding.inflate(inflater, container, false)

        binding.sharedFiles.setup {
            withDataSource(sharedFiles)

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

                    }

                    deleteShared.setOnClickListener {
                        activityModel.launchTransientScreen(TransientScreen.DeleteShared(listOf(item)))
                    }

                    icon.setImageResource(iconResource)
                }
            }
        }

        return binding.root
    }

}
