package net.lockbook;

public class File {
    String id;
    String parent;
    String name;
    FileType fileType;
    long lastModified;
    long lastModifiedBy;
    Share[] shares;

    public static enum FileType {
        Document,
        Folder
    }

    public static class Share {
        ShareMode mode;
        String sharedBy;
        String sharedWith;
    }

    public static enum ShareMode {
        Write,
        Read
    }
}
