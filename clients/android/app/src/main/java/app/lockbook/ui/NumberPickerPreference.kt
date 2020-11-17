package app.lockbook.ui

import android.content.Context
import android.content.res.TypedArray
import android.util.AttributeSet
import androidx.preference.DialogPreference
import app.lockbook.R

class NumberPickerPreference(context: Context, attributeSet: AttributeSet?) : DialogPreference(
    context,
    attributeSet
) {
    private var durationInMinutes: Int? = null

    fun getDuration(): Int {
        return durationInMinutes ?: 15
    }

    fun setDuration(duration: Int) {
        this.durationInMinutes = duration

        persistInt(duration)
    }

    override fun onGetDefaultValue(a: TypedArray?, index: Int): Any {
        if (a != null) {
            return a.getInt(index, 15)
        }

        return 15
    }

    override fun onSetInitialValue(defaultValue: Any?) {
        val trueDefaultValue = defaultValue?.toString()?.toIntOrNull() ?: 15
        setDuration(getPersistedInt(durationInMinutes ?: trueDefaultValue))
    }

    override fun getDialogLayoutResource(): Int {
        return R.layout.dialog_duration_picker
    }
}
