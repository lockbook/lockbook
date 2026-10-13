package net.lockbook;

/** Account-wide document counts, including inherited shares and accepted incoming shares. */
public class SharingContact {
    public String username;
    public long outgoingFileCount;
    public long incomingFileCount;
    public long totalFileCount;
}
