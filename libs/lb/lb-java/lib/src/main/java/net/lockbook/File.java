package net.lockbook;

public class File {
    String id;
    String parent;
    String name;
    FileType fileType;
    long lastModified;
    long lastModifiedBy;
    Share[] shares;

    public enum FileType {
        Document,
        Folder
    }

    public class Share {
        ShareMode mode;
        String sharedBy;
        String sharedWith;
    }

    public enum ShareMode {
        Write,
        Read
    }
}
