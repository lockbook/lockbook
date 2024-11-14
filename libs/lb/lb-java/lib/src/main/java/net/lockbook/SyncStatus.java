package net.lockbook;

public class SyncStatus {
    public WorkUnit[] workUnits;
    public long latestServerTS;

    public static class WorkUnit {
        public boolean isLocalChange;
        public String id;
    }
}