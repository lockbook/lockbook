package app.lockbook.core

external fun initLogger()
external fun isDbPresent(path: String): Boolean
external fun createAccount(path: String, username: String): Int
external fun importAccount(path: String, accountString: String): Int
external fun getRoot(path: String): String
external fun getChildren(path: String, parentUuid: String): String
external fun getFileMetadata(path: String, fileUuid: String): String

fun loadLockbookCore() {
    System.loadLibrary("lockbook_core")
    initLogger()
}