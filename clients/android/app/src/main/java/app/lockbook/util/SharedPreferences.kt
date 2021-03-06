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

    const val VIEW_LOGS_KEY = "view_logs"
    const val CLEAR_LOGS_KEY = "clear_logs"

    const val IS_THIS_AN_IMPORT_KEY = "import"

    const val BYTE_USAGE_KEY = "usage_amount"

    const val FILE_LAYOUT_KEY = "file_layout"
    const val GRID_LAYOUT = "grid_layout"
    const val LINEAR_LAYOUT = "linear_layout"

    const val OPEN_NEW_DOC_AUTOMATICALLY_KEY = "open_new_doc_automatically"
}
