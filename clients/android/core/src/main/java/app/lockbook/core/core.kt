package app.lockbook.core

external fun initLogger()
external fun isDbPresent(path: String): Boolean
external suspend fun createAccount(path: String, username: String): Int
external suspend fun importAccount(path: String, accountString: String): Int
external suspend fun getRoot(path: String): String
external suspend fun getChildren(path: String, parentUuid: String): String
external suspend fun getFileMetadata(path: String, fileUuid: String): String

fun loadLockbookCore() {
    System.loadLibrary("lockbook_core")
    initLogger()
}