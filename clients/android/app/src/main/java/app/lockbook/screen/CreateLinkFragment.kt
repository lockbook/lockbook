package app.lockbook.screen

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.fragment.app.viewModels
import app.lockbook.R
import app.lockbook.databinding.FragmentCreateLinkBinding
import app.lockbook.model.CreateLinkViewModel
import app.lockbook.model.ExtensionHelper
import app.lockbook.model.StateViewModel
import app.lockbook.util.File
import app.lockbook.util.FileType
import app.lockbook.util.BasicFileItemHolder
import com.afollestad.recyclical.setup
import com.afollestad.recyclical.withItem

class CreateLinkFragment : Fragment() {
    private val activityModel: StateViewModel by activityViewModels()
    private val model: CreateLinkViewModel by viewModels()

    private lateinit var binding: FragmentCreateLinkBinding

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        binding = FragmentCreateLinkBinding.inflate(inflater, container, false)

        binding.createLinkFiles.setup {
            withDataSource(model.files)
            withItem<File, BasicFileItemHolder>(R.layout.move_file_item) {
                onBind(::BasicFileItemHolder) { _, item ->
                    name.text = item.name
                    val extensionHelper = ExtensionHelper(item.name)

                    val imageResource = when {
                        item.fileType == FileType.Document && extensionHelper.isDrawing -> {
                            R.drawable.ic_outline_draw_24
                        }
                        item.fileType == FileType.Document && extensionHelper.isImage -> {
                            R.drawable.ic_outline_image_24
                        }
                        item.fileType == FileType.Document -> {
                            R.drawable.ic_outline_insert_drive_file_24
                        }
                        item.fileType == FileType.Document && extensionHelper.isPdf -> {
                            R.drawable.ic_outline_picture_as_pdf_24
                        }
                        else -> {
                            R.drawable.ic_baseline_folder_24
                        }
                    }

                    icon.setImageResource(imageResource)
                }
                onClick {
                    model.onItemClick(item)
                }
            }
        }

        return binding.root
    }
}