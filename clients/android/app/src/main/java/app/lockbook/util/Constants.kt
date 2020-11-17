package app.lockbook.util

object SharedPreferences {
    const val LOGGED_IN_KEY = "loggedin"

    const val BIOMETRIC_OPTION_KEY = "biometric"
    const val BIOMETRIC_NONE = "biometric_none"
    const val BIOMETRIC_RECOMMENDED = "biometric_recommended"
    const val BIOMETRIC_STRICT = "biometric_strict"

    const val EXPORT_ACCOUNT_RAW_KEY = "export_account_raw"

    const val EXPORT_ACCOUNT_QR_KEY = "export_account_qr"

    const val SORT_FILES_KEY = "sort_files"
    const val SORT_FILES_A_Z = "sort_files_a_z"
    const val SORT_FILES_Z_A = "sort_files_z_a"
    const val SORT_FILES_TYPE = "sort_files_type"
    const val SORT_FILES_FIRST_CHANGED = "sort_files_first_changed"
    const val SORT_FILES_LAST_CHANGED = "sort_files_last_changed"

    const val BACKGROUND_SYNC_PERIOD_KEY = "background_sync_period"
    const val BACKGROUND_SYNC_ENABLED_KEY = "background_sync_enabled"
    const val SYNC_AUTOMATICALLY_KEY = "sync_automatically_in_app"

    const val VIEW_LOGS_TITLE_KEY = "view_logs_title"
    const val VIEW_LOGS_KEY = "view_logs"
    const val CLEAR_LOGS_KEY = "clear_logs"

    const val IS_THIS_AN_IMPORT_KEY = "import"

    const val BYTE_USAGE_KEY = "usage_amount"
}

object RequestResultCodes {
    const val TEXT_EDITOR_REQUEST_CODE: Int = 102
    const val POP_UP_INFO_REQUEST_CODE: Int = 103
    const val HANDWRITING_EDITOR_REQUEST_CODE: Int = 104

    const val RENAME_RESULT_CODE: Int = 201
    const val DELETE_RESULT_CODE: Int = 202
}

object Messages {
    const val UNEXPECTED_ERROR = "An unexpected error has occurred!"
    const val UNEXPECTED_CLIENT_ERROR = "An error has occurred."
}

object WorkManagerTags {
    const val PERIODIC_SYNC_TAG = "periodic_sync"
}

const val CANVAS_WIDTH = 2125
const val CANVAS_HEIGHT = 2750
const val TEXT_EDITOR_BACKGROUND_SAVE_PERIOD: Long = 5000
const val LOG_FILE_NAME = "lockbook.log"
