import Foundation

/// Helper for auto-titling: parsing bare URLs, extracting page titles from HTML, formatting markdown links.
enum URLTitleFetcher {
        /// Returns the trimmed URL if the text is a bare HTTP/HTTPS URL (only a URL, possibly with surrounding whitespace).
        static func parseBareURL(_ text: String) -> URL? {
            let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty,
                  (trimmed.hasPrefix("http://") || trimmed.hasPrefix("https://")),
                  trimmed.firstIndex(of: " ") == nil,
                  let url = URL(string: trimmed),
                  (url.scheme?.lowercased() == "http" || url.scheme?.lowercased() == "https")
            else {
                return nil
            }
            return url
        }

        /// Extracts the title from HTML: prefers <title>, falls back to og:title and twitter:title for SPAs.
        static func extractTitleFromHTML(_ html: String) -> String? {
            let genericTitles = ["youtube", "twitter", "x.com", "instagram", "facebook", "t.co"]
            if let title = extractTag(html, pattern: "<title[^>]*>(.*?)</title>"),
               !title.isEmpty,
               !genericTitles.contains(where: { title.lowercased().hasPrefix($0) || title.lowercased() == $0 })
            {
                return cleanTitle(decodeHTMLEntities(title))
            }
            if let og = extractMetaContent(html, property: "og:title"), !og.isEmpty {
                return cleanTitle(decodeHTMLEntities(og))
            }
            if let tw = extractMetaContent(html, name: "twitter:title"), !tw.isEmpty {
                return cleanTitle(decodeHTMLEntities(tw))
            }
            if let title = extractTag(html, pattern: "<title[^>]*>(.*?)</title>"), !title.isEmpty {
                return cleanTitle(decodeHTMLEntities(title))
            }
            return nil
        }

        /// Strips common site suffixes like " - YouTube", " | X" from titles.
        static func cleanTitle(_ title: String) -> String {
            let suffixes = [" - youtube", " | youtube", " – youtube", " - twitter", " | twitter", " - x", " | x", " – x"]
            var t = title.trimmingCharacters(in: .whitespacesAndNewlines)
            for suffix in suffixes {
                if t.lowercased().hasSuffix(suffix) {
                    t = String(t.dropLast(suffix.count)).trimmingCharacters(in: .whitespaces)
                    break
                }
            }
            return t
        }

        static func extractTag(_ html: String, pattern: String) -> String? {
            guard let regex = try? NSRegularExpression(pattern: pattern, options: [.caseInsensitive, .dotMatchesLineSeparators]),
                  let match = regex.firstMatch(in: html, range: NSRange(html.startIndex..., in: html)),
                  let range = Range(match.range(at: 1), in: html)
            else { return nil }
            return String(html[range]).trimmingCharacters(in: .whitespacesAndNewlines)
        }

        static func extractMetaContent(_ html: String, property: String? = nil, name: String? = nil) -> String? {
            let attr: String
            let attrValue: String
            if let p = property {
                attr = "property"
                attrValue = p
            } else if let n = name {
                attr = "name"
                attrValue = n
            } else {
                return nil
            }
            let escaped = NSRegularExpression.escapedPattern(for: attrValue)
            let patterns = [
                "<meta[^>]+\(attr)=[\"']\(escaped)[\"'][^>]+content=[\"']([^\"']*)[\"']",
                "<meta[^>]+content=[\"']([^\"']*)[\"'][^>]+\(attr)=[\"']\(escaped)[\"']",
            ]
            for pattern in patterns {
                if let regex = try? NSRegularExpression(pattern: pattern, options: [.caseInsensitive, .dotMatchesLineSeparators]),
                   let match = regex.firstMatch(in: html, range: NSRange(html.startIndex..., in: html)),
                   let range = Range(match.range(at: 1), in: html)
                {
                    return String(html[range]).trimmingCharacters(in: .whitespacesAndNewlines)
                }
            }
            return nil
        }

        static func decodeHTMLEntities(_ str: String) -> String {
            var s = str
            let entities: [(String, String)] = [
                ("&amp;", "&"), ("&lt;", "<"), ("&gt;", ">"), ("&quot;", "\""), ("&#39;", "'"),
                ("&apos;", "'"), ("&nbsp;", "\u{00A0}"),
            ]
            for (entity, replacement) in entities {
                s = s.replacingOccurrences(of: entity, with: replacement)
            }
            return s
        }

        /// Formats title and URL as [title](url), escaping brackets in the title.
        static func formatMarkdownLink(title: String, url: String) -> String {
            let escaped = title
                .replacingOccurrences(of: "\\", with: "\\\\")
                .replacingOccurrences(of: "]", with: "\\]")
            return "[\(escaped)](\(url))"
        }
    }
