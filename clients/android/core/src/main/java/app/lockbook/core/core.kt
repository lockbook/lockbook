package app.lockbook.core

external fun init(config: String): String
external fun createAccount(username: String, apiUrl: String): String
external fun importAccount(account: String): String
external fun exportAccount(): String
external fun getAccount(): String
external fun convertToHumanDuration(metadataVersion: Long): String
external fun getUsage(): String
external fun getUncompressedUsage(): String
external fun getRoot(): String
external fun getChildren(id: String): String
external fun getFileById(id: String): String
external fun renameFile(id: String, name: String): String
external fun createFile(id: String, fileType: String, name: String): String
external fun deleteFile(id: String): String
external fun readDocument(id: String): String
external fun readDocumentBytes(id: String): ByteArray?
external fun exportDrawingToDisk(id: String, format: String, location: String): String
external fun writeDocument(id: String, content: String): String
external fun moveFile(id: String, parentId: String): String
external fun syncAll(syncModel: Any): String
external fun backgroundSync(): String
external fun calculateWork(): String
external fun exportFile(id: String, destination: String, edit: Boolean): String
external fun getAllErrorVariants(): String
external fun upgradeAccountGooglePlay(purchaseToken: String, accountId: String): String
external fun cancelSubscription(): String
external fun getSubscriptionInfo(): String
external fun getLocalChanges(): String
external fun listMetadatas(): String
external fun startSearch(searchFilesViewModel: Any): String
external fun search(query: String): String
external fun endSearch(): String
external fun stopCurrentSearch(): String
