package app.lockbook.utils

import android.os.Bundle
import android.view.View
import android.widget.NumberPicker
import androidx.preference.PreferenceDialogFragmentCompat
import app.lockbook.R
import kotlinx.android.synthetic.main.dialog_duration_picker.view.*
import timber.log.Timber

class NumberPickerPreferenceDialog : PreferenceDialogFragmentCompat() {
    var dayNumberPicker: NumberPicker? = null
    var hourNumberPicker: NumberPicker? = null
    var minuteNumberPicker: NumberPicker? = null

    companion object {
        fun newInstance(key: String): NumberPickerPreferenceDialog {
            val numberPickerPreferenceDialog = NumberPickerPreferenceDialog()
            val bundle = Bundle(1)
            bundle.putString(ARG_KEY, key)
            numberPickerPreferenceDialog.arguments = bundle
            return numberPickerPreferenceDialog
        }
    }

    private fun checkIfDurationTooLow(layoutView: View?, dayNumberPicker: NumberPicker?, hourNumberPicker: NumberPicker?, minuteNumberPicker: NumberPicker?) {
        val durationInMinutes = (dayNumberPicker?.value ?: 0) * 1440 + (hourNumberPicker?.value ?: 0) * 60 + (minuteNumberPicker?.value ?: 15)

        if (durationInMinutes < 15) {
            minuteNumberPicker?.value = 15
            layoutView?.duration_error?.visibility = View.VISIBLE
        }

        if (durationInMinutes >= 15 && layoutView?.duration_error?.visibility == View.VISIBLE) {
            layoutView.duration_error?.visibility = View.GONE
        }
    }

    override fun onBindDialogView(view: View?) {
        dayNumberPicker = view?.findViewById(R.id.duration_days)
        hourNumberPicker = view?.findViewById(R.id.duration_hours)
        minuteNumberPicker = view?.findViewById(R.id.duration_minutes)
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

        dayNumberPicker?.maxValue = 100
        dayNumberPicker?.minValue = 0
        dayNumberPicker?.value = days
        dayNumberPicker?.setOnValueChangedListener { picker, _, _ ->
            checkIfDurationTooLow(view, picker, hourNumberPicker, minuteNumberPicker)
        }

        hourNumberPicker?.maxValue = 59
        hourNumberPicker?.minValue = 0
        hourNumberPicker?.value = hours
        hourNumberPicker?.setOnValueChangedListener { picker, _, _ ->
            checkIfDurationTooLow(view, dayNumberPicker, picker, minuteNumberPicker)
        }

        minuteNumberPicker?.maxValue = 59
        minuteNumberPicker?.minValue = 0
        minuteNumberPicker?.value = minutes
        minuteNumberPicker?.setOnValueChangedListener { picker, _, _ ->
            checkIfDurationTooLow(view, dayNumberPicker, hourNumberPicker, picker)
        }
    }

    override fun onDialogClosed(positiveResult: Boolean) {
        val durationInMinutes = (dayNumberPicker?.value ?: 0) * 1440 + (hourNumberPicker?.value ?: 0) * 60 + (minuteNumberPicker?.value ?: 15)

        val preference = preference
        if (preference is NumberPickerPreference && positiveResult) {
            preference.callChangeListener(durationInMinutes)
            preference.setDuration(durationInMinutes)
        } else if (preference == null) {
            Timber.e("Unable to access preference.")
        }
    }
}
