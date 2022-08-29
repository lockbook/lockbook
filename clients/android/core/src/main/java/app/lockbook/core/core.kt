package app.lockbook.core

external fun init(config: String): String

// account
external fun createAccount(username: String, apiUrl: String): String
external fun importAccount(account: String): String
external fun exportAccount(): String
external fun getAccount(): String

// android only
external fun convertToHumanDuration(timeStamp: Long): String

// file loc
external fun getRoot(): String
external fun listMetadatas(): String
external fun getChildren(id: String): String
external fun getFileById(id: String): String

// file ops
external fun renameFile(id: String, name: String): String
external fun createFile(id: String, fileType: String, name: String): String
external fun deleteFile(id: String): String
external fun readDocument(id: String): String
external fun readDocumentBytes(id: String): ByteArray?
external fun writeDocument(id: String, content: String): String
external fun moveFile(id: String, parentId: String): String
external fun exportDrawingToDisk(id: String, format: String, location: String): String
external fun exportFile(id: String, destination: String, edit: Boolean): String

// sync
external fun syncAll(syncModel: Any): String
external fun backgroundSync(): String
external fun calculateWork(): String
external fun getLocalChanges(): String
external fun getUsage(): String
external fun getUncompressedUsage(): String

// subscriptions
external fun upgradeAccountGooglePlay(purchaseToken: String, accountId: String): String
external fun cancelSubscription(): String
external fun getSubscriptionInfo(): String

// search
external fun startSearch(searchFilesViewModel: Any): String
external fun search(query: String): String
external fun endSearch(): String
external fun stopCurrentSearch(): String

// sharing
external fun shareFile(id: String, username: String, mode: String): String
external fun getPendingShares(): String
external fun deletePendingShares(id: String): String

// tests
external fun getAllErrorVariants(): String
