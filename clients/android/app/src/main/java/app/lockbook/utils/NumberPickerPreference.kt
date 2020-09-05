package app.lockbook.utils

import android.content.Context
import android.content.res.TypedArray
import androidx.preference.DialogPreference
import app.lockbook.R

class NumberPickerPreference(context: Context): DialogPreference(context) {
    private var durationInMinutes: Int? = null

    fun getDuration(): Int {
        return durationInMinutes ?: 15
    }

    fun setDuration(duration: Int) {
        this.durationInMinutes = duration
    }

    override fun onGetDefaultValue(a: TypedArray?, index: Int): Any {
        if (a != null) {
            return a.getInt(index, 15)
        }

        return 15
    }

    override fun onSetInitialValue(defaultValue: Any?) {
        setDuration(getPersistedInt(durationInMinutes ?: 15))
    }

    override fun getDialogLayoutResource(): Int {
        return R.layout.dialog_duration_picker
    }
}