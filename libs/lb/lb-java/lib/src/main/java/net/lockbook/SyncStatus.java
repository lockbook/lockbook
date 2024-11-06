public class SyncStatus {
    WorkUnit workUnits;
    long latestServerTS;

    public static class WorkUnit {
        boolean isLocalChange;
        String id;
    }
}