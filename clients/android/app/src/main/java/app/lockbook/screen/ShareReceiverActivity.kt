package app.lockbook.screen

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.provider.OpenableColumns
import android.view.View
import androidx.activity.viewModels
import androidx.appcompat.app.AppCompatActivity
import androidx.fragment.app.Fragment
import androidx.fragment.app.FragmentManager
import androidx.fragment.app.viewModels
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.observe
import app.lockbook.R
import app.lockbook.databinding.ActivityShareReceiverBinding
import app.lockbook.model.AlertModel
import app.lockbook.model.MoveFileViewModel
import app.lockbook.model.StateViewModel
import app.lockbook.model.TransientScreen
import app.lockbook.ui.CreateFileDialogFragment
import app.lockbook.util.BasicFileItemHolder
import com.afollestad.recyclical.setup
import com.afollestad.recyclical.withItem
import kotlinx.coroutines.launch
import net.lockbook.Lb
import net.lockbook.LbError
import java.io.File
import java.lang.ref.WeakReference
import kotlin.getValue

class ShareReceiverActivity : AppCompatActivity() {
    private var _binding: ActivityShareReceiverBinding? = null
    val binding get() = _binding!!

    private var uris: MutableList<Uri> = mutableListOf()

    val importedFileId = MutableLiveData<String?>()

    private val alertModel by lazy {
        AlertModel(WeakReference(this))
    }

    private val activityModel: StateViewModel by viewModels()

    companion object {
        const val IMPORTED_FILE_KEY = "imported_dest_folder"
    }

    private val model: MoveFileViewModel by viewModels(
        factoryProducer = {
            object : ViewModelProvider.Factory {
                override fun <T : ViewModel> create(modelClass: Class<T>): T {
                    if (modelClass.isAssignableFrom(MoveFileViewModel::class.java))
                        return MoveFileViewModel(
                            application,
                            Lb.getRoot().id
                        ) as T
                    throw IllegalArgumentException("Unknown ViewModel class")
                }
            }
        }
    )

    private val fragmentFinishedCallback = object : FragmentManager.FragmentLifecycleCallbacks() {
        override fun onFragmentDestroyed(fm: FragmentManager, f: Fragment) {
            if (f is CreateFileDialogFragment) {
                model.refreshOverFolder()
            }
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        _binding = ActivityShareReceiverBinding.inflate(layoutInflater)
        setContentView(binding.root)
        setUpView()

        when (intent?.action) {
            Intent.ACTION_SEND_MULTIPLE -> {
                val receivedUris = intent.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM)
                if (receivedUris != null) {
                    uris = receivedUris
                }
            }
            Intent.ACTION_SEND -> {
                val receivedUri = intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM)
                if (receivedUri != null) {
                    uris.add(receivedUri)
                }
            }
        }

        binding.importButton.setOnClickListener {
            binding.importProgress.visibility = View.VISIBLE
            binding.importButton.isEnabled = false
            lifecycleScope.launch {
                val fileId = importFromUris()
                importedFileId.value = fileId
            }
        }

        val sharedFilesCount = uris.count()
        val subTitle = if (sharedFilesCount > 1) {
            "Importing $sharedFilesCount files"
        } else if (sharedFilesCount == 1) {
            "Importing " + getUriFileName(uris[0])
        } else {
            ""
        }
        binding.toolbar.setSubtitle(subTitle)

        supportFragmentManager.registerFragmentLifecycleCallbacks(
            fragmentFinishedCallback,
            false
        )

        binding.toolbar.setOnMenuItemClickListener {
            activityModel.launchTransientScreen(TransientScreen.Create(model.currentParent.id))
            CreateFileDialogFragment().show(
                supportFragmentManager,
                CreateFileDialogFragment.TAG
            )
            true
        }

        importedFileId.observe(this) { id ->
            binding.importProgress.visibility = View.INVISIBLE
            binding.importButton.isEnabled = true

            if (id !== null) {
                startActivity(
                    Intent(this, MainScreenActivity::class.java).apply {
                        putExtra(IMPORTED_FILE_KEY, id)
                    }
                )
            }
        }

        binding.cancelButton.setOnClickListener {
            finish()
        }
    }

    private fun getUriFileName(uri: Uri): String? {
        var fileName = "untitled"
        contentResolver.query(uri, null, null, null, null)?.use { cursor ->
            if (cursor.moveToFirst()) {
                val displayNameIndex = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)

                if (displayNameIndex != -1) {
                    fileName = cursor.getString(displayNameIndex)
                }
            }
        }
        return fileName
    }

    private fun importFromUris(): String? {
        var newFileId: String? = null

        for (uri in uris) {
            try {
                val data =
                    contentResolver.openInputStream(uri)?.use { stream -> stream.readBytes() }
                val lbFile = Lb.createFile(getUriFileName(uri), model.currentParent.id, true)
                newFileId = lbFile.id
                Lb.writeDocumentBytes(lbFile.id, data)
            } catch (err: LbError) {
                alertModel.notifyError(err)
            } catch (err: Exception) {
                err.message?.let { alertModel.notify(it) }
            }
        }

        // todo: when android supports multiple tabs. open all the imported files?
        return newFileId
    }

    private fun setUpView() {
        binding.moveFileList.setup {
            withDataSource(model.files)
            withItem<net.lockbook.File, BasicFileItemHolder>(R.layout.move_file_item) {
                onBind(::BasicFileItemHolder) { _, item ->
                    name.text = item.name
                    icon.setImageResource(R.drawable.ic_baseline_folder_24)
                }
                onClick {
                    model.onItemClick(item)
                }
            }
        }

        // ids is the set of files we'd like to move per the move file mode
        // but since we're importing a fresh new file into the app, we'll
        // use different logic. This silences the late init error
        model.ids = listOf()
    }
}
