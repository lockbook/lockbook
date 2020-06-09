package app.lockbook.core

external fun initLogger()
external fun isDbPresent(path: String): Boolean
external fun createAccount(path: String, username: String): Int
external fun importAccount(path: String, accountString: String): Int

fun loadLockbookCore() {
    System.loadLibrary("lockbook_core")
    initLogger()
}