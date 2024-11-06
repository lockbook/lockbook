public class Usage {
    UsageItemMetric serverUsage;
    UsageItemMetric dataCap;
    
    public static class UsageItemMetric {
        Long exact;
        String readable;
    }
}