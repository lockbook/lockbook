package app.lockbook.utils

object SharedPreferences {
    const val SHARED_PREF_FILE = "app.lockbook"
    const val LOGGED_IN_KEY = "loggedin"
    const val BIOMETRIC_OPTION_KEY = "biometric"

    const val BIOMETRIC_NONE = 0
    const val BIOMETRIC_RECOMMENDED = 1
    const val BIOMETRIC_STRICT = 2
}

object RequestResultCodes {
    const val FAILED_RESULT_CODE: Int = 2

    const val NEW_FILE_REQUEST_CODE: Int = 101
    const val TEXT_EDITOR_REQUEST_CODE: Int = 102
    const val POP_UP_INFO_REQUEST_CODE: Int = 103

    const val RENAME_RESULT_CODE: Int = 201
    const val DELETE_RESULT_CODE: Int = 202

    const val BIOMETRIC_SUCCESS_RESULT_CODE: Int = 301
    const val BIOMETRIC_FAILURE_RESULT_CODE: Int = 302
}
