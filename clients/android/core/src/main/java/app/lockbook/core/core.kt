package app.lockbook.core

external fun isDbPresent(path: String): Boolean

fun loadLockbookCore() {
    System.loadLibrary("lockbook_core")
}