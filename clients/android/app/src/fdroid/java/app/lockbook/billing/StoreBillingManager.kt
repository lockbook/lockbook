package app.lockbook.billing

import android.app.Activity
import android.content.Context
import android.view.View
import android.view.WindowManager
import android.view.inputmethod.InputMethodManager
import androidx.appcompat.app.AppCompatActivity
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.lifecycleScope
import app.lockbook.R
import app.lockbook.util.SingleMutableLiveData
import com.google.android.material.bottomsheet.BottomSheetBehavior
import com.google.android.material.bottomsheet.BottomSheetDialog
import com.google.android.material.button.MaterialButton
import com.google.android.material.progressindicator.CircularProgressIndicator
import com.google.android.material.textfield.TextInputEditText
import com.google.android.material.textfield.TextInputLayout
import com.google.android.material.textview.MaterialTextView
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import net.lockbook.Lb
import net.lockbook.LbError
import net.lockbook.LbError.LbEC
import timber.log.Timber

class StoreBillingManager(
    applicationContext: Context,
) : BillingManager {
    private val _billingEvent = SingleMutableLiveData<BillingEvent>()
    private val _premiumPrice = MutableLiveData(applicationContext.getString(R.string.premium_price))

    override val billingEvent: LiveData<BillingEvent>
        get() = _billingEvent

    override val premiumPrice: LiveData<String>
        get() = _premiumPrice

    override fun launchBillingFlow(activity: Activity) {
        val appCompatActivity = activity as? AppCompatActivity
        if (appCompatActivity == null) {
            _billingEvent.postValue(BillingEvent.NotifyUnrecoverableError)
            return
        }

        showCardDialog(appCompatActivity)
    }

    private fun showCardDialog(activity: AppCompatActivity) {
        val view = activity.layoutInflater.inflate(R.layout.dialog_stripe_card, null)
        val numberLayout = view.findViewById<TextInputLayout>(R.id.card_number_layout)
        val number = view.findViewById<TextInputEditText>(R.id.card_number)
        val monthLayout = view.findViewById<TextInputLayout>(R.id.card_expiration_month_layout)
        val month = view.findViewById<TextInputEditText>(R.id.card_expiration_month)
        val yearLayout = view.findViewById<TextInputLayout>(R.id.card_expiration_year_layout)
        val year = view.findViewById<TextInputEditText>(R.id.card_expiration_year)
        val cvcLayout = view.findViewById<TextInputLayout>(R.id.card_cvc_layout)
        val cvc = view.findViewById<TextInputEditText>(R.id.card_cvc)
        val paymentError = view.findViewById<MaterialTextView>(R.id.card_payment_error)
        val progress = view.findViewById<CircularProgressIndicator>(R.id.card_payment_progress)

        val subscribeButton = view.findViewById<MaterialButton>(R.id.card_subscribe)
        val cancelButton = view.findViewById<MaterialButton>(R.id.card_cancel)
        val dialog = BottomSheetDialog(activity)

        dialog.setContentView(view)
        dialog.window?.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
        dialog.behavior.skipCollapsed = true
        dialog.behavior.state = BottomSheetBehavior.STATE_EXPANDED
        dialog.setOnShowListener {
            number.requestFocus()
            dialog.window?.setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_STATE_ALWAYS_VISIBLE)
            number.post {
                val inputMethodManager =
                    activity.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
                inputMethodManager.showSoftInput(number, InputMethodManager.SHOW_IMPLICIT)
            }
        }
        dialog.setOnDismissListener {
            number.text?.clear()
            month.text?.clear()
            year.text?.clear()
            cvc.text?.clear()
        }
        cancelButton.setOnClickListener {
            dialog.dismiss()
        }
        subscribeButton.setOnClickListener {
            numberLayout.error = null
            monthLayout.error = null
            yearLayout.error = null
            cvcLayout.error = null
            paymentError.visibility = View.GONE

            val cardNumber = number.text.toString().filter(Char::isDigit)
            val expirationMonth = month.text.toString().toIntOrNull()
            val expirationYear = parseExpirationYear(year.text.toString())
            val cardCvc = cvc.text.toString()

            var isValid = true
            if (cardNumber.length !in 12..19) {
                numberLayout.error = activity.getString(R.string.invalid_card_number)
                isValid = false
            }
            if (expirationMonth !in 1..12) {
                monthLayout.error = activity.getString(R.string.invalid_expiration_month)
                isValid = false
            }
            if (expirationYear == null) {
                yearLayout.error = activity.getString(R.string.invalid_expiration_year)
                isValid = false
            }
            if (cardCvc.length !in 3..4 || !cardCvc.all(Char::isDigit)) {
                cvcLayout.error = activity.getString(R.string.invalid_card_cvc)
                isValid = false
            }
            if (!isValid) {
                return@setOnClickListener
            }

            setPaymentFormEnabled(view, false)
            subscribeButton.isEnabled = false
            cancelButton.isEnabled = false
            dialog.setCancelable(false)
            dialog.setCanceledOnTouchOutside(false)
            progress.visibility = View.VISIBLE

            activity.lifecycleScope.launch {
                val error: Throwable? =
                    withContext(Dispatchers.IO) {
                        try {
                            Lb.upgradeAccountStripe(
                                cardNumber,
                                checkNotNull(expirationYear),
                                checkNotNull(expirationMonth),
                                cardCvc,
                            )
                            null
                        } catch (error: CancellationException) {
                            throw error
                        } catch (error: Throwable) {
                            error
                        }
                    }

                if (error == null) {
                    dialog.dismiss()
                    _billingEvent.value = BillingEvent.SuccessfulPurchase
                } else {
                    restorePaymentForm(view, subscribeButton, cancelButton, dialog, progress)
                    showPaymentError(
                        error,
                        paymentError,
                        numberLayout,
                        monthLayout,
                        yearLayout,
                        cvcLayout,
                    )
                }
            }
        }
        dialog.show()
    }

    private fun setPaymentFormEnabled(
        view: View,
        isEnabled: Boolean,
    ) {
        view.findViewById<TextInputEditText>(R.id.card_number).isEnabled = isEnabled
        view.findViewById<TextInputEditText>(R.id.card_expiration_month).isEnabled = isEnabled
        view.findViewById<TextInputEditText>(R.id.card_expiration_year).isEnabled = isEnabled
        view.findViewById<TextInputEditText>(R.id.card_cvc).isEnabled = isEnabled
    }

    private fun restorePaymentForm(
        view: View,
        subscribeButton: MaterialButton,
        cancelButton: MaterialButton,
        dialog: BottomSheetDialog,
        progress: CircularProgressIndicator,
    ) {
        setPaymentFormEnabled(view, true)
        subscribeButton.isEnabled = true
        cancelButton.isEnabled = true
        dialog.setCancelable(true)
        dialog.setCanceledOnTouchOutside(true)
        progress.visibility = View.GONE
    }

    private fun showPaymentError(
        error: Throwable,
        paymentError: MaterialTextView,
        numberLayout: TextInputLayout,
        monthLayout: TextInputLayout,
        yearLayout: TextInputLayout,
        cvcLayout: TextInputLayout,
    ) {
        if (error !is LbError) {
            Timber.e(error, "Unexpected Stripe payment error")
            paymentError.setText(R.string.basic_error)
            paymentError.visibility = View.VISIBLE
            return
        }

        when (error.kind) {
            LbEC.CardInvalidNumber -> {
                numberLayout.error = error.msg
            }

            LbEC.CardInvalidExpMonth -> {
                monthLayout.error = error.msg
            }

            LbEC.CardInvalidExpYear, LbEC.CardExpired -> {
                yearLayout.error = error.msg
            }

            LbEC.CardInvalidCvc -> {
                cvcLayout.error = error.msg
            }

            else -> {
                if (error.kind == LbEC.Unexpected) {
                    Timber.e(error, "Unexpected Stripe payment error")
                    paymentError.setText(R.string.basic_error)
                } else {
                    paymentError.text = error.msg
                }
                paymentError.visibility = View.VISIBLE
            }
        }
    }

    private fun parseExpirationYear(value: String): Int? =
        when (value.length) {
            2 -> value.toIntOrNull()?.plus(2000)
            4 -> value.toIntOrNull()
            else -> null
        }
}
