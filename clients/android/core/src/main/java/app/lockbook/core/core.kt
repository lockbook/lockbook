package app.lockbook.core

external fun helloDirect(to: String): String

fun loadRustyLib() {
    System.loadLibrary("lockbook_core")
}