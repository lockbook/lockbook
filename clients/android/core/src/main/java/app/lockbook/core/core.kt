package app.lockbook.core

external fun initLogger(path: String): String
external fun getUsage(config: String): String
external fun getDBState(config: String): String
external fun migrateDB(config: String): String
external fun createAccount(config: String, username: String, apiUrl: String): String
external fun importAccount(config: String, account: String): String
external fun exportAccount(config: String): String
external fun getAccount(config: String): String
external fun setLastSynced(config: String, lastSynced: Long): String
external fun getLastSyncedHumanString(config: String): String
external fun getUsageHumanString(config: String, exact: Boolean): String
external fun getRoot(config: String): String
external fun getChildren(config: String, id: String): String
external fun getFileById(config: String, id: String): String
external fun insertFile(config: String, fileMetadata: String): String
external fun renameFile(config: String, id: String, name: String): String
external fun createFile(config: String, id: String, fileType: String, name: String): String
external fun deleteFile(config: String, id: String): String
external fun readDocument(config: String, id: String): String
external fun writeDocument(config: String, id: String, content: String): String
external fun moveFile(config: String, id: String, parentId: String): String
external fun syncAll(config: String): String
external fun calculateWork(config: String): String
external fun executeWork(config: String, account: String, workUnit: String): String
external fun getAllErrorVariants(): String
