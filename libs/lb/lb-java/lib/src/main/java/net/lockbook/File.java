package net.lockbook;

import javax.annotation.Nonnull;

public class File {
    @Nonnull
    public String id;
    @Nonnull
    public String parent;
    @Nonnull
    public String name;
    @Nonnull
    public FileType type;
    @Nonnull
    public long lastModified;
    @Nonnull
    public String lastModifiedBy;
    @Nonnull
    public Share[] shares;

    public boolean isRoot() {
        return id.equals(parent);
    }

    public enum FileType {
        Folder,
        Document,
        Link
    }

    public static class Share {
        @Nonnull
        public ShareMode mode;
        @Nonnull
        public String sharedBy;
        @Nonnull
        public String sharedWith;
    }

    public enum ShareMode {
        Write,
        Read
    }


}
