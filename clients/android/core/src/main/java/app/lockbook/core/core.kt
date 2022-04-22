package app.lockbook.core

external fun init(config: String): String
external fun createAccount(username: String, apiUrl: String): String
external fun importAccount(account: String): String
external fun exportAccount(): String
external fun getAccount(): String
external fun convertToHumanDuration(metadataVersion: Long): String
external fun getUsage(): String
external fun getUncompressedUsage(): String
external fun makeBytesReadable(bytes: Long): String
external fun getRoot(): String
external fun getChildren(id: String): String
external fun getFileById(id: String): String
external fun renameFile(id: String, name: String): String
external fun createFile(id: String, fileType: String, name: String): String
external fun deleteFile(id: String): String
external fun readDocument(id: String): String
external fun saveDocumentToDisk(id: String, location: String): String
external fun exportDrawing(id: String, format: String): String
external fun exportDrawingToDisk(id: String, format: String, location: String): String
external fun writeDocument(id: String, content: String): String
external fun moveFile(id: String, parentId: String): String
external fun syncAll(fragment: Any): String
external fun backgroundSync(): String
external fun calculateWork(): String
external fun getAllErrorVariants(): String
