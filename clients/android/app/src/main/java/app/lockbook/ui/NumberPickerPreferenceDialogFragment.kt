package app.lockbook.ui

import android.app.Dialog
import android.content.DialogInterface
import android.os.Bundle
import android.view.View
import android.widget.NumberPicker
import androidx.core.os.bundleOf
import androidx.core.view.isVisible
import androidx.fragment.app.setFragmentResult
import androidx.preference.PreferenceDialogFragmentCompat
import app.lockbook.R
import app.lockbook.databinding.DialogDurationPickerBinding
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import timber.log.Timber

class NumberPickerPreferenceDialogFragment : PreferenceDialogFragmentCompat() {
    private lateinit var binding: DialogDurationPickerBinding

    private val dayNumberPicker get() = binding.durationDays
    private val hourNumberPicker get() = binding.durationHours
    private val minuteNumberPicker get() = binding.durationMinutes

    companion object {
        fun newInstance(key: String): NumberPickerPreferenceDialogFragment {
            val numberPickerPreferenceDialog = NumberPickerPreferenceDialogFragment()
            val bundle = Bundle(1)
            bundle.putString(ARG_KEY, key)
            numberPickerPreferenceDialog.arguments = bundle
            return numberPickerPreferenceDialog
        }
    }

    override fun onCreateDialog(savedInstanceState: Bundle?): Dialog = MaterialAlertDialogBuilder(requireContext())
        .apply {
            binding = DialogDurationPickerBinding.inflate(layoutInflater)
            setUpInfo()
            setView(binding.root)
        }
        .setNegativeButton(R.string.cancel, null)
        .setPositiveButton(R.string.confirm) { _: DialogInterface, _: Int -> onPositiveButton() }
        .create()

    private fun checkIfDurationTooLow(dayNumberPicker: NumberPicker?, hourNumberPicker: NumberPicker?, minuteNumberPicker: NumberPicker?) {
        val durationInMinutes = (dayNumberPicker?.value ?: 0) * 1440 + (hourNumberPicker?.value ?: 0) * 60 + (minuteNumberPicker?.value ?: 15)

        if (durationInMinutes < 15) {
            minuteNumberPicker?.value = 15
            binding.durationError.visibility = View.VISIBLE
        } else if (binding.durationError.isVisible) {
            binding.durationError.visibility = View.GONE
        }
    }

    private fun setUpInfo() {
        var durationInMinutes = 15

        val preference = preference
        if (preference is NumberPickerPreference) {
            durationInMinutes = preference.getDuration()
        } else {
            Timber.e("Unable to access preference.")
        }

        val days = durationInMinutes / 1440
        durationInMinutes -= days * 1440
        val hours = durationInMinutes / 60
        val minutes = durationInMinutes % 60

        dayNumberPicker.maxValue = 100
        dayNumberPicker.minValue = 0
        dayNumberPicker.value = days
        dayNumberPicker.setOnValueChangedListener { picker, _, _ ->
            checkIfDurationTooLow(picker, hourNumberPicker, minuteNumberPicker)
        }

        hourNumberPicker.maxValue = 59
        hourNumberPicker.minValue = 0
        hourNumberPicker.value = hours
        hourNumberPicker.setOnValueChangedListener { picker, _, _ ->
            checkIfDurationTooLow(dayNumberPicker, picker, minuteNumberPicker)
        }

        minuteNumberPicker.maxValue = 59
        minuteNumberPicker.minValue = 0
        minuteNumberPicker.value = minutes
        minuteNumberPicker.setOnValueChangedListener { picker, _, _ ->
            checkIfDurationTooLow(dayNumberPicker, hourNumberPicker, picker)
        }
    }

    override fun onDialogClosed(positiveResult: Boolean) {}

    private fun onPositiveButton() {
        val durationInMinutes = dayNumberPicker.value * 1440 + hourNumberPicker.value * 60 + minuteNumberPicker.value

        val preference = preference
        if (preference is NumberPickerPreference) {
            preference.callChangeListener(durationInMinutes)
            preference.setDuration(durationInMinutes)
        } else {
            Timber.e("Unable to access preference.")
        }

        this.setFragmentResult(preference.key, bundleOf())
    }
}
