/*
 * This source file was generated by the Gradle 'init' task
 */
package net.lockbook;

public class Lb {
    public static long lb = 0;

    public static native void init(String path) throws LbError;
    public static native String getDebugInfo(String osInfo) throws LbError;
    public static native Account createAccount(String username, String apiUrl, boolean welcomeDoc) throws LbError;
    public static native Account importAccount(String key) throws LbError;
    public static native Account getAccount() throws LbError;
    public static native String exportAccountPrivateKey() throws LbError;
    public static native String exportAccountPhrase() throws LbError;
    public static native byte[] exportAccountQR() throws LbError;

    public static native File getRoot() throws LbError;
    public static native File[] listMetadatas() throws LbError;
    public static native File[] getChildren(String id) throws LbError;
    public static native File getFileById(String id) throws LbError;
    public static native void renameFile(String id, String name) throws LbError;

    public static native File createFile(String name, String parentId, boolean isDoc) throws LbError;
    public static native File createLink(String name, String targetId, String parentId) throws LbError;
    public static native void moveFile(String id, String parentId) throws LbError;
    public static native void deleteFile(String id) throws LbError;
    public static native String readDocument(String id) throws LbError;
    public static native byte[] readDocumentBytes(String id) throws LbError;
    public static native void writeDocument(String id, String content) throws LbError;
    public static native void writeDocumentBytes(String id, byte[] content) throws LbError;
    public static native void exportFile(String id, String dest, boolean edit) throws LbError;
    public static native void shareFile(String id, String username, boolean isWriteMode) throws LbError;

    public static native String getTimestampHumanString(long timestamp) throws LbError;

    public static native Usage getUsage() throws LbError;
    public static native Usage.UsageItemMetric getUncompressedUsage() throws LbError;
    public static native SyncStatus calculateWork() throws LbError;
    public static native String[] getLocalChanges() throws LbError;
    public static native void sync(SyncProgress syncProgress) throws LbError;
    public static native File[] getPendingShares() throws LbError;
    public static native void deletePendingShare(String id) throws LbError;

    public static native void upgradeAccountGooglePlay(String purchaseToken, String accountId) throws LbError;
    public static native SubscriptionInfo getSubscriptionInfo() throws LbError;
    public static native void cancelSubscription() throws LbError;

    public static native SearchResult[] search(String input) throws LbError;
    public static native String[] suggestedDocs() throws LbError;

    public static native void logout() throws LbError;
    public static native void deleteAccount() throws LbError;
}
