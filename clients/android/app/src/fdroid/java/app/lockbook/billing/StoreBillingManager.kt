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
import app.lockbook.databinding.DialogStripeCardBinding
import app.lockbook.util.SingleMutableLiveData
import com.google.android.material.bottomsheet.BottomSheetBehavior
import com.google.android.material.bottomsheet.BottomSheetDialog
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
        val binding = DialogStripeCardBinding.inflate(activity.layoutInflater)
        val dialog = BottomSheetDialog(activity)

        dialog.setContentView(binding.root)
        dialog.window?.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
        dialog.behavior.skipCollapsed = true
        dialog.behavior.state = BottomSheetBehavior.STATE_EXPANDED
        dialog.setOnShowListener {
            binding.cardNumber.requestFocus()
            dialog.window?.setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_STATE_ALWAYS_VISIBLE)
            binding.cardNumber.post {
                val inputMethodManager =
                    activity.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
                inputMethodManager.showSoftInput(binding.cardNumber, InputMethodManager.SHOW_IMPLICIT)
            }
        }
        dialog.setOnDismissListener {
            binding.cardNumber.text?.clear()
            binding.cardExpirationMonth.text?.clear()
            binding.cardExpirationYear.text?.clear()
            binding.cardCvc.text?.clear()
        }
        binding.cardCancel.setOnClickListener {
            dialog.dismiss()
        }
        binding.cardSubscribe.setOnClickListener {
            binding.cardNumberLayout.error = null
            binding.cardExpirationMonthLayout.error = null
            binding.cardExpirationYearLayout.error = null
            binding.cardCvcLayout.error = null
            binding.cardPaymentError.visibility = View.GONE

            val cardNumber = binding.cardNumber.text.toString().filter(Char::isDigit)
            val expirationMonth = binding.cardExpirationMonth.text.toString().toIntOrNull()
            val expirationYear = parseExpirationYear(binding.cardExpirationYear.text.toString())
            val cardCvc = binding.cardCvc.text.toString()

            var isValid = true
            if (cardNumber.length !in 12..19) {
                binding.cardNumberLayout.error = activity.getString(R.string.invalid_card_number)
                isValid = false
            }
            if (expirationMonth !in 1..12) {
                binding.cardExpirationMonthLayout.error = activity.getString(R.string.invalid_expiration_month)
                isValid = false
            }
            if (expirationYear == null) {
                binding.cardExpirationYearLayout.error = activity.getString(R.string.invalid_expiration_year)
                isValid = false
            }
            if (cardCvc.length !in 3..4 || !cardCvc.all(Char::isDigit)) {
                binding.cardCvcLayout.error = activity.getString(R.string.invalid_card_cvc)
                isValid = false
            }
            if (!isValid) {
                return@setOnClickListener
            }

            setPaymentFormEnabled(binding, false)
            binding.cardSubscribe.isEnabled = false
            binding.cardCancel.isEnabled = false
            dialog.setCancelable(false)
            dialog.setCanceledOnTouchOutside(false)
            binding.cardPaymentProgress.visibility = View.VISIBLE

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
                    restorePaymentForm(binding, dialog)
                    showPaymentError(error, binding)
                }
            }
        }
        dialog.show()
    }

    private fun setPaymentFormEnabled(
        binding: DialogStripeCardBinding,
        isEnabled: Boolean,
    ) {
        binding.cardNumber.isEnabled = isEnabled
        binding.cardExpirationMonth.isEnabled = isEnabled
        binding.cardExpirationYear.isEnabled = isEnabled
        binding.cardCvc.isEnabled = isEnabled
    }

    private fun restorePaymentForm(
        binding: DialogStripeCardBinding,
        dialog: BottomSheetDialog,
    ) {
        setPaymentFormEnabled(binding, true)
        binding.cardSubscribe.isEnabled = true
        binding.cardCancel.isEnabled = true
        dialog.setCancelable(true)
        dialog.setCanceledOnTouchOutside(true)
        binding.cardPaymentProgress.visibility = View.GONE
    }

    private fun showPaymentError(
        error: Throwable,
        binding: DialogStripeCardBinding,
    ) {
        if (error !is LbError) {
            Timber.e(error, "Unexpected Stripe payment error")
            binding.cardPaymentError.setText(R.string.basic_error)
            binding.cardPaymentError.visibility = View.VISIBLE
            return
        }

        when (error.kind) {
            LbEC.CardInvalidNumber -> {
                binding.cardNumberLayout.error = error.msg
            }

            LbEC.CardInvalidExpMonth -> {
                binding.cardExpirationMonthLayout.error = error.msg
            }

            LbEC.CardInvalidExpYear, LbEC.CardExpired -> {
                binding.cardExpirationYearLayout.error = error.msg
            }

            LbEC.CardInvalidCvc -> {
                binding.cardCvcLayout.error = error.msg
            }

            else -> {
                if (error.kind == LbEC.Unexpected) {
                    Timber.e(error, "Unexpected Stripe payment error")
                    binding.cardPaymentError.setText(R.string.basic_error)
                } else {
                    binding.cardPaymentError.text = error.msg
                }
                binding.cardPaymentError.visibility = View.VISIBLE
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
