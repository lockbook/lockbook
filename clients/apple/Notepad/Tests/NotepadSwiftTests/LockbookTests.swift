import XCTest
import NotepadSwift

class LockbookTests: XCTestCase {
    let engine = MarkdownEngine()

    func testMarkdownAnalyzePerformance() throws {
        let md = "`hello🥰`"

        let nodes = engine.render(md)

        XCTAssertEqual(nodes, [
            MarkdownNode(range: NSRange(location: 1, length: 8), type: .code, headingLevel: 0)
        ])
    }

}
