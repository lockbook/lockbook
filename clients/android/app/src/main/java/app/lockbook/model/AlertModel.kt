package app.lockbook.model

import android.app.Activity
import android.view.View
import androidx.appcompat.app.AlertDialog
import app.lockbook.App
import app.lockbook.R
import app.lockbook.util.*
import com.google.android.material.snackbar.Snackbar
import timber.log.Timber

class AlertModel(activity: Activity? = null, view: View? = null) {

    private var view: View = view ?: activity!!.findViewById(android.R.id.content)
    private var unexpectedErrorMsg = App.instance.resources.getString(R.string.unexpected_error)

    fun notifyBasicError(onFinish: (() -> Unit)? = null) = notify(resIdToString(R.string.basic_error), onFinish)

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

    private fun notifyWithDialog(title: String, msg: String, onFinish: (() -> Unit)? = null) {
        val dialog = AlertDialog.Builder(App.instance, R.style.Main_Widget_Dialog)
            .setTitle(title)
            .setMessage(msg)

        Timber.e("Unexpected Error: $msg")

        if (onFinish != null) {
            dialog.setOnCancelListener {
                onFinish()
            }
        }

        dialog.show()
    }

    fun notifyError(error: LbError, onFinish: (() -> Unit)? = null) {
        when(error.kind) {
            LbErrorKind.Program -> notifyWithDialog(unexpectedErrorMsg, error.msg, onFinish)
            LbErrorKind.User -> notify(error.msg, onFinish)
        }
    }
}
