public class SyncStatus {
    WorkUnit workUnits;
    long latestServerTS;

    public class WorkUnit {
        boolean isLocalChange;
        String id;
    }
}