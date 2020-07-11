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
external fun insertDocument(path: String, fileUuid: String, document: String): Int
external fun deleteFileFolder(path: String, fileUuid: String): Int
external fun createFileFolder(path: String, parentUuid: String, fileType: String, name: String): String

fun loadLockbookCore() {
    System.loadLibrary("lockbook_core")
    initLogger()
}