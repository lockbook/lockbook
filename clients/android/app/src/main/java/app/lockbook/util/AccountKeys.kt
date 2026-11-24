package app.lockbook.util

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.view.Gravity
import android.view.ViewGroup
import android.widget.ImageView
import android.widget.PopupWindow
import androidx.fragment.app.Fragment
import app.lockbook.R
import com.google.zxing.BarcodeFormat
import com.journeyapps.barcodescanner.BarcodeEncoder
import net.lockbook.LbError
import net.lockbook.LbError.LbEC

fun Fragment.copyToClipboard(
    label: String,
    text: String,
) {
    val clipBoard = requireContext().getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
    clipBoard.setPrimaryClip(ClipData.newPlainText(label, text))
}

fun Fragment.showAccountKeyQR(key: String) {
    val bitmap = BarcodeEncoder().encodeBitmap(key, BarcodeFormat.QR_CODE, 400, 400)
    val root = requireView() as ViewGroup
    val qrCodeView = layoutInflater.inflate(R.layout.popup_window_qr_code, root, false)
    qrCodeView.findViewById<ImageView>(R.id.qr_code).setImageBitmap(bitmap)
    PopupWindow(qrCodeView, 900, 900, true).showAtLocation(root, Gravity.CENTER, 0, 0)
}

fun LbError.inlineMessage(context: Context): String = if (kind == LbEC.Unexpected) context.getString(R.string.unexpected_error) else msg
