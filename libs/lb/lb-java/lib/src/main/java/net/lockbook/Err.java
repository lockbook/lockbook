package net.lockbook;

public class Err extends Exception {
    public EKind kind;
    public String msg;
    public String trace;
}
