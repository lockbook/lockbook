package net.lockbook;

public class Usage {
    public UsageItemMetric serverUsage;
    public UsageItemMetric dataCap;
    
    public static class UsageItemMetric {
        public long exact;
        public String readable;
    }
}