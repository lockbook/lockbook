package app.lockbook.screen

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.fragment.app.Fragment
import androidx.fragment.app.viewModels
import androidx.navigation.fragment.findNavController
import app.lockbook.R
import app.lockbook.databinding.FragmentCreateLinkBinding
import app.lockbook.model.*
import app.lockbook.util.BasicFileItemHolder
import app.lockbook.util.ExtensionHelper
import app.lockbook.util.File
import app.lockbook.util.FileType
import com.afollestad.recyclical.setup
import com.afollestad.recyclical.withItem
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import java.lang.ref.WeakReference

class CreateLinkFragment : Fragment() {
    private val model: CreateLinkViewModel by viewModels()

    private lateinit var binding: FragmentCreateLinkBinding

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()), view)
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        binding = FragmentCreateLinkBinding.inflate(inflater, container, false)

        binding.createLinkToolbar.setNavigationOnClickListener {
            model.refreshOverParent()
        }

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
                        item.fileType == FileType.Document && extensionHelper.isPdf -> {
                            R.drawable.ic_outline_picture_as_pdf_24
                        }
                        item.fileType == FileType.Document -> {
                            R.drawable.ic_outline_insert_drive_file_24
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

        model.updateTitle.observe(
            viewLifecycleOwner
        ) { title ->
            binding.createLinkToolbar.title = title
        }

        model.closeFragment.observe(
            viewLifecycleOwner
        ) {
            findNavController().popBackStack()
        }

        model.notifyError.observe(
            viewLifecycleOwner
        ) { error ->
            alertModel.notifyError(error) {
                findNavController().popBackStack()
            }
        }

        val file = requireArguments().getParcelable<File>(CREATE_LINK_FILE_KEY)!!
        binding.createLinkFileFor.setText(getString(R.string.create_link_file_for, file.name))
        binding.createLinkName.setText(file.name)

        binding.createLinkCreate.setOnClickListener {
            val name = binding.createLinkName.text.toString()

            when (val result = CoreModel.createLink(name, file.id, model.currentParent.id)) {
                is Ok -> {
                    alertModel.notifyWithToast(getString(R.string.created_link))
                    findNavController().popBackStack()
                }
                is Err -> alertModel.notifyError(result.error.toLbError(resources))
            }
        }

        binding.createLinkCancel.setOnClickListener {
            findNavController().popBackStack()
        }

        return binding.root
    }

    fun onBackPressed() {
        if (model.currentParent.isRoot()) {
            findNavController().popBackStack()
        } else {
            model.refreshOverParent()
        }
    }

    companion object {
        const val CREATE_LINK_FILE_KEY = "create_link_file_key"
    }
}
