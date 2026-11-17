package net.lockbook;


public class Usage {
    public UsageItemMetric synced;
    public UsageItemMetric local;
    public UsageItemMetric added;
    public UsageItemMetric freed;
    public UsageItemMetric dataCap;
    
    public static class UsageItemMetric {
        public long exact;
        public String readable;
    }
}
