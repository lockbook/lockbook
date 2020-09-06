package app.lockbook.utils

import android.os.Bundle
import android.view.View
import android.widget.NumberPicker
import androidx.preference.PreferenceDialogFragmentCompat
import app.lockbook.R
import timber.log.Timber

class NumberPickerPreferenceDialog: PreferenceDialogFragmentCompat() {
    var dayNumberPicker: NumberPicker? = null
    var hourNumberPicker : NumberPicker? = null
    var minuteNumberPicker : NumberPicker? = null


    companion object {
        fun newInstance(key: String): NumberPickerPreferenceDialog {
            val numberPickerPreferenceDialog = NumberPickerPreferenceDialog()
            val bundle = Bundle(1)
            bundle.putString(ARG_KEY, key)
            numberPickerPreferenceDialog.arguments = bundle
            return numberPickerPreferenceDialog
        }
    }

    override fun onBindDialogView(view: View?) {
        dayNumberPicker = view?.findViewById(R.id.duration_days)
        hourNumberPicker = view?.findViewById(R.id.duration_hours)
        minuteNumberPicker = view?.findViewById(R.id.duration_minutes)
        var durationInMinutes = 15

        val preference = preference
        if(preference is NumberPickerPreference) {
            durationInMinutes = preference.getDuration()
        } else {
            Timber.e("Unable to access preference.")
        }

        val days = durationInMinutes / 1440
        durationInMinutes-= days * 1440
        val hours = durationInMinutes / 60
        val minutes = durationInMinutes % 60

        dayNumberPicker?.maxValue = 100
        dayNumberPicker?.minValue = 0
        dayNumberPicker?.value = days

        hourNumberPicker?.maxValue = 59
        hourNumberPicker?.minValue = 0
        hourNumberPicker?.value = hours

        minuteNumberPicker?.maxValue = 59
        minuteNumberPicker?.minValue = 15
        minuteNumberPicker?.value = minutes
    }

    override fun onDialogClosed(positiveResult: Boolean) {
        var durationInMinutes = (dayNumberPicker?.value ?: 0) * 1440 + (hourNumberPicker?.value ?: 0) * 60 + (minuteNumberPicker?.value ?: 0)

        val preference = preference
        if(preference is NumberPickerPreference && positiveResult) {
            preference.callChangeListener(durationInMinutes)
            preference.setDuration(durationInMinutes)
        } else if(preference == null) {
            Timber.e("Unable to access preference.")
        }
    }



}