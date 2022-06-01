package app.lockbook.model

import android.app.Activity
import android.os.Handler
import android.os.Looper
import android.view.View
import androidx.appcompat.app.AlertDialog
import app.lockbook.R
import app.lockbook.util.LbError
import app.lockbook.util.LbErrorKind
import com.google.android.material.bottomsheet.BottomSheetDialog
import com.google.android.material.snackbar.Snackbar
import timber.log.Timber
import java.lang.ref.WeakReference

class AlertModel(private val activity: WeakReference<Activity>, view: View? = null) {

    private var view: View = view ?: activity.get()!!.findViewById(android.R.id.content)
    private var unexpectedErrorMsg = activity.get()!!.resources.getString(R.string.unexpected_error)

    fun notifyBasicError(onFinish: (() -> Unit)? = null) {
        notify(unexpectedErrorMsg, onFinish)
    }

    fun notify(msg: String, onFinish: (() -> Unit)? = null) {
        Handler(Looper.getMainLooper()).post {
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

    private fun notifyWithDialog(title: String, msg: String, onFinish: (() -> Unit)? = null) {
        Handler(Looper.getMainLooper()).post {
            val dialog = AlertDialog.Builder(activity.get()!!, R.style.Main_Widget_Dialog)
                .setTitle(title)
                .setMessage(msg)

            if (onFinish != null) {
                dialog.setOnCancelListener {
                    onFinish()
                }
            }

            dialog.show()
        }
    }

    fun notifyError(error: LbError, onFinish: (() -> Unit)? = null) {
        when (error.kind) {
            LbErrorKind.Program -> notifyWithDialog(unexpectedErrorMsg, error.msg, onFinish)
            LbErrorKind.User -> {
                Timber.e("Unexpected Error: $error.msg")
                notify(error.msg, onFinish)
            }
        }
    }

    fun notifySuccessfulPurchaseConfirm(onFinish: (() -> Unit)? = null) {
        val successfulPurchaseDialog =
            BottomSheetDialog(activity.get()!!)
        successfulPurchaseDialog.setContentView(R.layout.purchased_premium)
        successfulPurchaseDialog.show()
        successfulPurchaseDialog.setCanceledOnTouchOutside(true)

        if (onFinish != null) {
            successfulPurchaseDialog.setOnDismissListener {
                onFinish()
            }
        }
    }
}
