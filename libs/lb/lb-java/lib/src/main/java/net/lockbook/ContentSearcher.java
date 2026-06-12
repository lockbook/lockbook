package net.lockbook;

public class ContentSearcher implements AutoCloseable {
    private long handle;

    ContentSearcher(long handle) {
        this.handle = handle;
    }

    public ContentSearcherResult[] query(String input) {
        requireOpen();
        return queryNative(handle, input);
    }

    public SearcherSnippet snippet(String id, ContentSearcherMatch match, int contextChars) {
        requireOpen();
        return snippetNative(handle, id, match, contextChars);
    }

    @Override
    public void close() {
        requireOpen();
        closeNative(handle);
        handle = 0;
    }

    private void requireOpen() {
        if (handle == 0) {
            throw new IllegalStateException("ContentSearcher is closed");
        }
    }

    private static native ContentSearcherResult[] queryNative(long handle, String input);
    private static native SearcherSnippet snippetNative(long handle, String id, ContentSearcherMatch match, int contextChars);
    private static native void closeNative(long handle);
}
