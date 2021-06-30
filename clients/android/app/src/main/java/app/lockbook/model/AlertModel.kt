package app.lockbook.model

import android.app.Activity
import android.view.View
import androidx.appcompat.app.AlertDialog
import app.lockbook.App
import app.lockbook.R
import app.lockbook.util.*
import com.google.android.material.snackbar.Snackbar
import timber.log.Timber

class AlertModel(activity: Activity) {

    private var view: View = activity.findViewById(android.R.id.content)

    fun notify(msg: String, onFinish: (() -> Unit)? = null) {
        val snackBar = Snackbar.make(view, msg, Snackbar.LENGTH_SHORT)

        if (onFinish != null) {
            snackBar.addCallback(object : Snackbar.Callback() {
                override fun onDismissed(transientBottomBar: Snackbar?, event: Int) {
                    super.onDismissed(transientBottomBar, event)
                    onFinish()
                }
            })
        }

        snackBar.show()
    }

    fun notifyError(error: LbError, onFinish: (() -> Unit)? = null) {
        when(error.kind) {
            LbErrorKind.Program -> notifyProgramError(error.msg, onFinish)
            LbErrorKind.User -> notifyUserError(error.msg, onFinish)
        }
    }

    private fun notifyProgramError(msg: String, onFinish: (() -> Unit)? = null) {
        val dialog = AlertDialog.Builder(App.instance, R.style.Main_Widget_Dialog)
            .setTitle(App.instance.resources.getString(R.string.unexpected_error))
            .setMessage(msg)

        Timber.e("Unexpected Error: $msg")

        if (onFinish != null) {
            dialog.setOnCancelListener {
                onFinish()
            }
        }

        dialog.show()
    }

    private fun notifyUserError(msg: String, onFinish: (() -> Unit)? = null) {
        val snackBar = Snackbar.make(view, msg, Snackbar.LENGTH_SHORT)

        if (onFinish != null) {
            snackBar.addCallback(object : Snackbar.Callback() {
                override fun onDismissed(transientBottomBar: Snackbar?, event: Int) {
                    super.onDismissed(transientBottomBar, event)
                    onFinish()
                }
            })
        }

        snackBar.show()
    }
}

sealed class onFinishAlert {
    onFinishUserError(val onFinish: () -> Unit)
}
