package app.lockbook.util

import android.widget.Toast
import androidx.fragment.app.DialogFragment
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.withStarted
import app.lockbook.model.FileMetadataSnapshot
import app.lockbook.model.FileMetadataStore
import kotlinx.coroutines.launch
import net.lockbook.LbError

/** Restored dialogs may exist before the initial metadata snapshot is ready. */
internal fun DialogFragment.withFileMetadata(
    owner: LifecycleOwner = this,
    bind: (FileMetadataSnapshot) -> Unit,
) {
    owner.lifecycleScope.launch {
        try {
            val snapshot = FileMetadataStore.awaitLoaded()
            owner.lifecycle.withStarted { bind(snapshot) }
        } catch (error: LbError) {
            context?.let { Toast.makeText(it, error.msg, Toast.LENGTH_LONG).show() }
            dismissAllowingStateLoss()
        }
    }
}
