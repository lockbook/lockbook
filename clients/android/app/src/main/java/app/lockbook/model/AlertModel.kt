package app.lockbook.model

import android.content.Context
import android.view.View
import androidx.appcompat.app.AlertDialog
import app.lockbook.R
import app.lockbook.util.UNEXPECTED_ERROR
import com.google.android.material.snackbar.Snackbar

object AlertModel {
    fun notify(view: View, msg: String, onFinishAlert: OnFinishAlert) {
        val snackBar = Snackbar.make(view, msg, Snackbar.LENGTH_SHORT)

        if (onFinishAlert is OnFinishAlert.DoSomethingOnFinishAlert) {
            snackBar.addCallback(object : Snackbar.Callback() {
                override fun onDismissed(transientBottomBar: Snackbar?, event: Int) {
                    super.onDismissed(transientBottomBar, event)
                    onFinishAlert.onFinish()
                }
            })
        }

        snackBar.show()
    }

    fun errorHasOccurred(view: View, msg: String, onFinishAlert: OnFinishAlert) {
        val snackBar = Snackbar.make(view, msg, Snackbar.LENGTH_SHORT)

        if (onFinishAlert is OnFinishAlert.DoSomethingOnFinishAlert) {
            snackBar.addCallback(object : Snackbar.Callback() {
                override fun onDismissed(transientBottomBar: Snackbar?, event: Int) {
                    super.onDismissed(transientBottomBar, event)
                    onFinishAlert.onFinish()
                }
            })
        }

        snackBar.show()
    }

    fun unexpectedCoreErrorHasOccurred(context: Context, error: String, onFinishAlert: OnFinishAlert) {
        val dialog = AlertDialog.Builder(context, R.style.Main_Widget_Dialog)
            .setTitle(UNEXPECTED_ERROR)
            .setMessage(error)

        if (onFinishAlert is OnFinishAlert.DoSomethingOnFinishAlert) {
            dialog.setOnCancelListener {
                onFinishAlert.onFinish()
            }
        }

        dialog.show()
    }
}

sealed class OnFinishAlert {
    object DoNothingOnFinishAlert : OnFinishAlert()
    data class DoSomethingOnFinishAlert(val onFinish: () -> Unit) : OnFinishAlert()
}
