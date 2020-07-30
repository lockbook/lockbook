package app.lockbook.loggedin.settings

import android.graphics.Bitmap
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.ViewModel
import app.lockbook.utils.AccountExportError
import app.lockbook.utils.ClickInterface
import app.lockbook.utils.Config
import app.lockbook.utils.CoreModel
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.google.zxing.BarcodeFormat
import com.journeyapps.barcodescanner.BarcodeEncoder
import kotlinx.coroutines.*

class SettingsViewModel(path: String) :
    ViewModel(),
    ClickInterface {

    private val config = Config(path)
    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)

    private val _navigateToAccountQRCode = MutableLiveData<Bitmap>()
    private val _copyAccountString = MutableLiveData<String>()
    private val _errorHasOccurred = MutableLiveData<String>()
    private val _navigateToBiometricSetting = MutableLiveData<Unit>()

    val navigateToAccountQRCode: LiveData<Bitmap>
        get() = _navigateToAccountQRCode

    val copyAccountString: LiveData<String>
        get() = _copyAccountString

    val errorHasOccurred: LiveData<String>
        get() = _errorHasOccurred

    val navigateToBiometricSetting: LiveData<Unit>
        get() = _navigateToBiometricSetting

    override fun onItemClick(position: Int) { // cleaner code is needed
        uiScope.launch {
            withContext(Dispatchers.IO) {
                when (position) {
                    0 -> exportAccountQR()
                    1 -> exportAccountString()
                    2 -> navigateToBiometricSetting()
                }
            }
        }
    }

    private fun navigateToBiometricSetting() {
        _navigateToBiometricSetting.postValue(Unit)
    }

    private fun exportAccountQR() {
        when (val exportResult = CoreModel.exportAccount(config)) {
            is Ok -> {
                val bitmap = BarcodeEncoder().encodeBitmap(
                    exportResult.value,
                    BarcodeFormat.QR_CODE,
                    400,
                    400
                )

                _navigateToAccountQRCode.postValue(bitmap)
            }
            is Err -> when (exportResult.error) {
                is AccountExportError.NoAccount -> _errorHasOccurred.postValue("Error! No account!")
                is AccountExportError.UnexpectedError -> _errorHasOccurred.postValue("An unexpected error has occurred!")
            }
        }
    }

    private fun exportAccountString() {
        when (val exportResult = CoreModel.exportAccount(config)) {
            is Ok -> _copyAccountString.postValue(exportResult.value)
            is Err -> when (exportResult.error) {
                is AccountExportError.NoAccount -> _errorHasOccurred.postValue("Error! No account!")
                is AccountExportError.UnexpectedError -> _errorHasOccurred.postValue("An unexpected error has occurred!")
            }
        }
    }
}
