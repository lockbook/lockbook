package net.lockbook;

public class LbStatus {
    public boolean offline;
    public boolean syncing;
    public boolean outOfSpace;
    public boolean pendingShares;
    public boolean updateRequired;
    public String[] pushingFiles;
    public String[] dirtyLocally;
    public String[] pullingFiles;
    public Usage spaceUsed;
    public String syncStatus;
    public String unexpectedSyncProblem;
}
