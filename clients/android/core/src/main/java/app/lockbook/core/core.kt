package app.lockbook.core

external fun initLogger()
external fun isDbPresent(path: String): Boolean
external fun createAccount(config: String, username: String): String
external fun importAccount(config: String, accountString: String): String
external fun getRoot(config: String): String
external fun getChildren(config: String, id: String): String
external fun getFileById(config: String, id: String): String
external fun insertFile(config: String, fileMetadata: String): String
external fun renameFile(config: String, id: String, name: String): String
external fun createFile(config: String, id: String, fileType: String, name: String): String
external fun deleteFile(config: String, id: String): String
external fun readDocument(config: String, id: String): String
external fun writeDocument(config: String, id: String, content: String): String

fun loadLockbookCore() {
    System.loadLibrary("lockbook_core")
    initLogger()
}