package app.lockbook.ui

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import androidx.fragment.app.DialogFragment
import androidx.lifecycle.ViewModelProvider
import androidx.recyclerview.widget.LinearLayoutManager
import app.lockbook.R
import app.lockbook.model.AlertModel
import app.lockbook.model.MoveFileAdapter
import app.lockbook.model.MoveFileViewModel
import app.lockbook.model.OnFinishAlert
import app.lockbook.modelfactory.MoveFileViewModelFactory
import app.lockbook.util.BASIC_ERROR
import kotlinx.android.synthetic.main.dialog_move_file.*

data class MoveFileInfo(
    val ids: Array<String>,
    val names: Array<String>
)

class MoveFileDialogFragment : DialogFragment() {

    lateinit var ids: Array<String>
    lateinit var names: Array<String>
    lateinit var moveFileViewModel: MoveFileViewModel

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
    ): View? = inflater.inflate(
        R.layout.dialog_move_file,
        container,
        false
    )

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)
        val bundle = arguments
        val nullableIds = bundle?.getStringArray(IDS_KEY)
        val nullableNames = bundle?.getStringArray(NAMES_KEY)
        if (nullableIds != null && nullableNames != null) {
            ids = nullableIds
            names = nullableNames
        } else {
            AlertModel.errorHasOccurred(move_file_dialog, BASIC_ERROR, OnFinishAlert.DoSomethingOnFinishAlert(::dismiss))
        }

        val application = requireNotNull(this.activity).application
        val moveFileViewModelFactory =
            MoveFileViewModelFactory(application.filesDir.absolutePath)
        moveFileViewModel =
            ViewModelProvider(this, moveFileViewModelFactory).get(MoveFileViewModel::class.java)
        val adapter =
            MoveFileAdapter(moveFileViewModel)

        move_file_list.layoutManager = LinearLayoutManager(context)
        move_file_list.adapter = adapter
        move_file_cancel.setOnClickListener {
            dismiss()
        }
        move_file_confirm.setOnClickListener {
            move_file_progress_bar.visibility = View.VISIBLE
            moveFileViewModel.moveFilesToFolder()
        }

        dialog?.setCanceledOnTouchOutside(false) ?: AlertModel.errorHasOccurred(move_file_dialog, BASIC_ERROR, OnFinishAlert.DoNothingOnFinishAlert)

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
            move_file_progress_bar.visibility = View.GONE
            dismiss()
        }

        moveFileViewModel.errorHasOccurred.observe(
            viewLifecycleOwner
        ) { errorText ->
            AlertModel.errorHasOccurred(move_file_dialog, errorText, OnFinishAlert.DoNothingOnFinishAlert)
        }

        moveFileViewModel.unexpectedErrorHasOccurred.observe(
            viewLifecycleOwner
        ) { errorText ->
            AlertModel.unexpectedCoreErrorHasOccurred(requireContext(), errorText, OnFinishAlert.DoSomethingOnFinishAlert(::dismiss))
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
