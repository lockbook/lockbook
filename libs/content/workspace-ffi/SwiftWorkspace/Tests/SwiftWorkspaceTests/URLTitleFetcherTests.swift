import XCTest
@testable import SwiftWorkspace

final class URLTitleFetcherTests: XCTestCase {
    // MARK: - parseBareURL

    func testParseBareURL_validHTTPS() {
        let url = URLTitleFetcher.parseBareURL("https://example.com")
        XCTAssertNotNil(url)
        XCTAssertEqual(url?.absoluteString, "https://example.com")
    }

    func testParseBareURL_validHTTP() {
        let url = URLTitleFetcher.parseBareURL("http://example.com")
        XCTAssertNotNil(url)
        XCTAssertEqual(url?.absoluteString, "http://example.com")
    }

    func testParseBareURL_withWhitespace() {
        let url = URLTitleFetcher.parseBareURL("  https://example.com  ")
        XCTAssertNotNil(url)
        XCTAssertEqual(url?.absoluteString, "https://example.com")
    }

    func testParseBareURL_withQueryAndFragment() {
        let url = URLTitleFetcher.parseBareURL("https://example.com/path?q=1#anchor")
        XCTAssertNotNil(url)
    }

    func testParseBareURL_rejectsSpace() {
        XCTAssertNil(URLTitleFetcher.parseBareURL("https://example.com more text"))
    }

    func testParseBareURL_rejectsEmpty() {
        XCTAssertNil(URLTitleFetcher.parseBareURL(""))
        XCTAssertNil(URLTitleFetcher.parseBareURL("   "))
    }

    func testParseBareURL_rejectsNonURL() {
        XCTAssertNil(URLTitleFetcher.parseBareURL("not a url"))
    }

    func testParseBareURL_rejectsFTP() {
        XCTAssertNil(URLTitleFetcher.parseBareURL("ftp://example.com"))
    }

    // MARK: - extractTitleFromHTML

    func testExtractTitleFromHTML_titleTag() {
        let html = "<html><head><title>My Page Title</title></head><body></body></html>"
        let title = URLTitleFetcher.extractTitleFromHTML(html)
        XCTAssertEqual(title, "My Page Title")
    }

    func testExtractTitleFromHTML_titleWithAttributes() {
        let html = "<html><head><title lang=\"en\">Page Title</title></head></html>"
        let title = URLTitleFetcher.extractTitleFromHTML(html)
        XCTAssertEqual(title, "Page Title")
    }

    func testExtractTitleFromHTML_ogTitle() {
        let html = """
        <html><head>
        <meta property="og:title" content="OG Title Here" />
        </head></html>
        """
        let title = URLTitleFetcher.extractTitleFromHTML(html)
        XCTAssertEqual(title, "OG Title Here")
    }

    func testExtractTitleFromHTML_twitterTitle() {
        let html = """
        <html><head>
        <meta name="twitter:title" content="Twitter Title" />
        </head></html>
        """
        let title = URLTitleFetcher.extractTitleFromHTML(html)
        XCTAssertEqual(title, "Twitter Title")
    }

    func testExtractTitleFromHTML_genericTitlePrefersMeta() {
        let html = """
        <html><head>
        <title>YouTube</title>
        <meta property="og:title" content="Real Video Title" />
        </head></html>
        """
        let title = URLTitleFetcher.extractTitleFromHTML(html)
        XCTAssertEqual(title, "Real Video Title")
    }

    func testExtractTitleFromHTML_emptyReturnsNil() {
        XCTAssertNil(URLTitleFetcher.extractTitleFromHTML(""))
        XCTAssertNil(URLTitleFetcher.extractTitleFromHTML("<html></html>"))
    }

    // MARK: - cleanTitle

    func testCleanTitle_removesYouTubeSuffix() {
        let result = URLTitleFetcher.cleanTitle("My Video - YouTube")
        XCTAssertEqual(result, "My Video")
    }

    func testCleanTitle_removesTwitterSuffix() {
        let result = URLTitleFetcher.cleanTitle("Tweet | Twitter")
        XCTAssertEqual(result, "Tweet")
    }

    func testCleanTitle_removesXBarSuffix() {
        let result = URLTitleFetcher.cleanTitle("Post - X")
        XCTAssertEqual(result, "Post")
    }

    func testCleanTitle_preservesNormalTitle() {
        let result = URLTitleFetcher.cleanTitle("Normal Title")
        XCTAssertEqual(result, "Normal Title")
    }

    // MARK: - formatMarkdownLink

    func testFormatMarkdownLink_basic() {
        let result = URLTitleFetcher.formatMarkdownLink(title: "Link", url: "https://example.com")
        XCTAssertEqual(result, "[Link](https://example.com)")
    }

    func testFormatMarkdownLink_escapesBrackets() {
        // Implementation escapes ] to avoid early link termination; [ can remain
        let result = URLTitleFetcher.formatMarkdownLink(title: "See [here]", url: "https://example.com")
        XCTAssertEqual(result, "[See [here\\]](https://example.com)")
    }

    func testFormatMarkdownLink_escapesBackslash() {
        let result = URLTitleFetcher.formatMarkdownLink(title: "C:\\path", url: "https://example.com")
        XCTAssertEqual(result, "[C:\\\\path](https://example.com)")
    }

    // MARK: - decodeHTMLEntities

    func testDecodeHTMLEntities_amp() {
        let result = URLTitleFetcher.decodeHTMLEntities("Fish &amp; Chips")
        XCTAssertEqual(result, "Fish & Chips")
    }

    func testDecodeHTMLEntities_numeric() {
        let result = URLTitleFetcher.decodeHTMLEntities("Don&#39;t")
        XCTAssertEqual(result, "Don't")
    }

    func testDecodeHTMLEntities_ltGt() {
        let result = URLTitleFetcher.decodeHTMLEntities("&lt;tag&gt;")
        XCTAssertEqual(result, "<tag>")
    }
}
