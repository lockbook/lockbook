package net.lockbook;

public interface SearchResult {
    public static class DocumentMatch implements SearchResult {
        String id;
        String path;

        public static class ContentMatch {
            String paragraph;
            int[] matchedIndicies;
            int score;
        }
    }

    public static class PathMatch implements SearchResult {
        String id;
        String path;
    }
}
