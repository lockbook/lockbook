package app.lockbook.core

external fun initLogger()
external fun isDbPresent(path: String): Boolean
external fun createAccount(path: String, username: String): Int
external fun importAccount(path: String, accountString: String): Int

external fun getRoot(path: String): String
external fun getChildren(path: String, parentUuid: String): String

external fun getFileMetadata(path: String, fileUuid: String): String
external fun getFile(path: String, fileUuid: String): String

external fun insertFileFolder(path: String, fileMetadata: String): Int
external fun renameFileFolder(path: String, fileUuid: String, newName: String): Int
external fun createFileFolder(path: String, parentUuid: String, fileType: String, name: String): String
external fun readDocument(path: String, fileUuid: String): String
external fun writeToDocument(path: String, fileUuid: String, content: String): Int

external fun sync(path: String): Int
external fun calculateWork(path: String): String
external fun executeWork(path: String, account: String, work: String): Int

fun loadLockbookCore() {
    System.loadLibrary("lockbook_core")
    initLogger()
}