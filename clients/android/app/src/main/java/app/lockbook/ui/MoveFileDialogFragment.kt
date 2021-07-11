package app.lockbook.ui

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import androidx.fragment.app.DialogFragment
import androidx.lifecycle.ViewModelProvider
import androidx.recyclerview.widget.LinearLayoutManager
import app.lockbook.databinding.DialogMoveFileBinding
import app.lockbook.model.AlertModel
import app.lockbook.model.MoveFileAdapter
import app.lockbook.model.MoveFileViewModel
import app.lockbook.modelfactory.MoveFileViewModelFactory
import java.lang.ref.WeakReference

data class MoveFileInfo(
    val ids: Array<String>,
    val names: Array<String>
) {
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (javaClass != other?.javaClass) return false

        other as MoveFileInfo

        if (!ids.contentEquals(other.ids)) return false
        if (!names.contentEquals(other.names)) return false

        return true
    }

    override fun hashCode(): Int {
        var result = ids.contentHashCode()
        result = 31 * result + names.contentHashCode()
        return result
    }
}

class MoveFileDialogFragment : DialogFragment() {

    private var _binding: DialogMoveFileBinding? = null
    // This property is only valid between onCreateView and
    // onDestroyView.
    private val binding get() = _binding!!

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()), view)
    }

    private lateinit var ids: Array<String>
    private lateinit var names: Array<String>
    private lateinit var moveFileViewModel: MoveFileViewModel

    companion object {

        const val MOVE_FILE_DIALOG_TAG = "MoveFileDialogFragment"

        private const val IDS_KEY = "IDS_KEY"
        private const val NAMES_KEY = "NAMES_KEY"

        fun newInstance(ids: Array<String>, names: Array<String>): MoveFileDialogFragment {
            val args = Bundle()
            args.putStringArray(IDS_KEY, ids)
            args.putStringArray(NAMES_KEY, names)

            val fragment = MoveFileDialogFragment()
            fragment.arguments = args
            return fragment
        }
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = DialogMoveFileBinding.inflate(
            inflater,
            container,
            false
        )

        return binding.root
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)
        val bundle = arguments
        val nullableIds = bundle?.getStringArray(IDS_KEY)
        val nullableNames = bundle?.getStringArray(NAMES_KEY)
        if (nullableIds != null && nullableNames != null) {
            ids = nullableIds
            names = nullableNames
        } else {
            alertModel.notifyBasicError(::dismiss)
        }

        val moveFileViewModelFactory =
            MoveFileViewModelFactory(requireActivity().application)
        moveFileViewModel =
            ViewModelProvider(this, moveFileViewModelFactory).get(MoveFileViewModel::class.java)
        val adapter =
            MoveFileAdapter(moveFileViewModel)

        binding.moveFileList.layoutManager = LinearLayoutManager(context)
        binding.moveFileList.adapter = adapter
        binding.moveFileCancel.setOnClickListener {
            dismiss()
        }
        binding.moveFileConfirm.setOnClickListener {
            binding.moveFileProgressBar.visibility = View.VISIBLE
            moveFileViewModel.moveFilesToFolder()
        }

        dialog?.setCanceledOnTouchOutside(false) ?: alertModel.notifyBasicError()

        moveFileViewModel.ids = ids
        moveFileViewModel.names = names

        moveFileViewModel.files.observe(
            viewLifecycleOwner
        ) { files ->
            adapter.files = files
        }

        moveFileViewModel.closeDialog.observe(
            viewLifecycleOwner
        ) {
            binding.moveFileProgressBar.visibility = View.GONE
            dismiss()
        }

        moveFileViewModel.notifyError.observe(
            viewLifecycleOwner
        ) { error ->
            alertModel.notifyError(error)
        }
    }

    override fun onStart() {
        super.onStart()
        dialog?.window?.setLayout(
            WindowManager.LayoutParams.MATCH_PARENT,
            WindowManager.LayoutParams.MATCH_PARENT
        )
    }
}
