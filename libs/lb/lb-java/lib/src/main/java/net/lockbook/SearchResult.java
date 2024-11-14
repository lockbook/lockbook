package net.lockbook;

public interface SearchResult {
    public static class DocumentMatch implements SearchResult {
        public String id;
        public String path;
        public ContentMatch[] contentMatches;

        public static class ContentMatch {
            public String paragraph;
            public int[] matchedIndices;
            public int score;
        }
    }

    public static class PathMatch implements SearchResult {
        public String id;
        public String path;
        public int[] matchedIndices;
        public int score;
    }
}
