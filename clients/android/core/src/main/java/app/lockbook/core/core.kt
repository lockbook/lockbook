package app.lockbook.core

external fun initLogger()
external fun isDbPresent(path: String): Boolean
external fun createAccount(path: String, username: String): Byte



fun loadLockbookCore() {
    System.loadLibrary("lockbook_core")
    initLogger()
}