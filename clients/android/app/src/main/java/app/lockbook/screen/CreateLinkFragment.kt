package app.lockbook.screen

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.fragment.app.viewModels
import androidx.navigation.fragment.findNavController
import app.lockbook.R
import app.lockbook.databinding.FragmentCreateLinkBinding
import app.lockbook.model.*
import app.lockbook.util.BasicFileItemHolder
import app.lockbook.util.getIconResource
import com.afollestad.recyclical.setup
import com.afollestad.recyclical.withItem
import net.lockbook.File
import net.lockbook.Lb
import net.lockbook.LbError
import java.lang.ref.WeakReference
import kotlin.getValue

class CreateLinkFragment : Fragment() {
    private val model: CreateLinkViewModel by viewModels()

    private lateinit var binding: FragmentCreateLinkBinding

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()), view)
    }

    private val activityModel: StateViewModel by activityViewModels()

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
                    name.text = item.getPrettyName()

                    icon.setImageResource(item.getIconResource())
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
            popBackStack()
        }

        model.notifyError.observe(
            viewLifecycleOwner
        ) { error ->
            alertModel.notifyError(error) {
                popBackStack()
            }
        }

        try {
            val file = Lb.getFileById(requireArguments().getString(CREATE_LINK_FILE_ID_KEY)!!)

            binding.createLinkFileFor.setText(getString(R.string.create_link_file_for, file.name))
            binding.createLinkName.setText(file.name)

            binding.createLinkCreate.setOnClickListener {
                val name = binding.createLinkName.text.toString()

                try {
                    Lb.createLink(name, file.id, model.currentParent.id)
                    alertModel.notifyWithToast(getString(R.string.created_link))
                    popBackStack()
                } catch (err: LbError) {
                    alertModel.notifyError(err)
                }
            }
        } catch (err: LbError) {
            alertModel.notifyError(err)
        }

        binding.createLinkCancel.setOnClickListener {
            popBackStack()
        }

        return binding.root
    }

    override fun onResume() {
        activityModel.updateMainScreenUI(UpdateMainScreenUI.ToggleBottomViewNavigation)
        super.onResume()
    }

    override fun onStop() {
        activityModel.updateMainScreenUI(UpdateMainScreenUI.ToggleBottomViewNavigation)
        super.onStop()
    }

    fun onBackPressed() {
        if (model.currentParent.isRoot()) {
            popBackStack()
        } else {
            model.refreshOverParent()
        }
    }

    private fun popBackStack() {
        findNavController().popBackStack()
    }

    companion object {
        const val CREATE_LINK_FILE_ID_KEY = "create_link_file_key"
    }
}
