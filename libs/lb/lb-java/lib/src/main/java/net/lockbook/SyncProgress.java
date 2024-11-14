package net.lockbook;

public interface SyncProgress {
    void updateSyncProgressAndTotal(int total, int progress, String message);
}
