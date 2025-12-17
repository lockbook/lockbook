package app.lockbook.ui

import android.app.AlertDialog
import android.app.Dialog
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.fragment.app.DialogFragment
import androidx.fragment.app.activityViewModels
import androidx.fragment.app.viewModels
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import app.lockbook.R
import app.lockbook.databinding.DialogMoveFileBinding
import app.lockbook.model.*
import app.lockbook.util.BasicFileItemHolder
import app.lockbook.util.getIconResource
import com.afollestad.recyclical.setup
import com.afollestad.recyclical.withItem
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import net.lockbook.File
import java.lang.ref.WeakReference

class MoveFileDialogFragment : DialogFragment() {

    private lateinit var binding: DialogMoveFileBinding

    private val activityModel: StateViewModel by activityViewModels()
    private val model: MoveFileViewModel by viewModels(
        factoryProducer = {
            object : ViewModelProvider.Factory {
                override fun <T : ViewModel> create(modelClass: Class<T>): T {
                    if (modelClass.isAssignableFrom(MoveFileViewModel::class.java))
                        return MoveFileViewModel(
                            requireActivity().application,
                            (activityModel.transientScreen as TransientScreen.Move).files[0].parent
                        ) as T
                    throw IllegalArgumentException("Unknown ViewModel class")
                }
            }
        }
    )

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()), view)
    }

    companion object {
        const val TAG = "MoveFileDialogFragment"
    }

    override fun onCreateDialog(savedInstanceState: Bundle?): Dialog = MaterialAlertDialogBuilder(requireContext())
        .setTitle(R.string.move_file_title)
        .apply {
            binding = DialogMoveFileBinding.inflate(layoutInflater)
            setUpView()
            setView(binding.root)
        }
        .setNegativeButton(R.string.cancel, null)
        .setPositiveButton(R.string.move_file_move, null)
        .create()
        .apply {
            setOnShowListener {
                getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener { onButtonPositive() }
            }
        }

    private fun setUpView() {
        binding.moveFileList.setup {
            withDataSource(model.files)
            withItem<File, BasicFileItemHolder>(R.layout.move_file_item) {
                onBind(::BasicFileItemHolder) { _, item ->
                    name.text = item.name

                    icon.setImageResource(item.getIconResource())
                }
                onClick {
                    model.onItemClick(item)
                }
            }
        }

        model.ids = (activityModel.transientScreen as TransientScreen.Move).files.map { it.id }
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        return binding.root
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        model.closeDialog.observe(
            viewLifecycleOwner
        ) {
            dismiss()
        }

        model.notifyError.observe(
            viewLifecycleOwner
        ) { error ->
            alertModel.notifyError(error)
            dismiss()
        }
    }

    private fun onButtonPositive() {
        model.moveFilesToCurrentFolder()
    }
}
