package net.lockbook;

public class PathSearcher implements AutoCloseable {
    private long handle;

    PathSearcher(long handle) {
        this.handle = handle;
    }

    public PathSearcherResult[] query(String input) {
        requireOpen();
        return queryNative(handle, input);
    }

    @Override
    public void close() {
        requireOpen();
        closeNative(handle);
        handle = 0;
    }

    private void requireOpen() {
        if (handle == 0) {
            throw new IllegalStateException("PathSearcher is closed");
        }
    }

    private static native PathSearcherResult[] queryNative(long handle, String input);
    private static native void closeNative(long handle);
}
