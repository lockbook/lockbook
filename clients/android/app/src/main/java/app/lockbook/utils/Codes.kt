package app.lockbook.utils

object SharedPreferences {
    const val LOGGED_IN_KEY = "loggedin"
    const val BIOMETRIC_OPTION_KEY = "biometric"

    const val BIOMETRIC_NONE = "biometric_none"
    const val BIOMETRIC_RECOMMENDED = "biometric_recommended"
    const val BIOMETRIC_STRICT = "biometric_strict"
}

object RequestResultCodes {
    const val FAILED_RESULT_CODE: Int = 2

    const val NEW_FILE_REQUEST_CODE: Int = 101
    const val TEXT_EDITOR_REQUEST_CODE: Int = 102
    const val POP_UP_INFO_REQUEST_CODE: Int = 103

    const val RENAME_RESULT_CODE: Int = 201
    const val DELETE_RESULT_CODE: Int = 202
}
