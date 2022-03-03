@testable import Lockbook
import Down
import XCTest

class EditorTest: XCTestCase {

    func testEmptyString() throws {
        let indexer = IndexConverter("")
        let range = indexer.getRange(startCol: 1, endCol: 0, startLine: 1, endLine: 0)
        
        XCTAssertEqual(range, NSRange(location: 0, length: 0))
    }
    
    func testSingleCharacter() throws {
        let indexer = IndexConverter("a")
        let range = indexer.getRange(startCol: 1, endCol: 1, startLine: 1, endLine: 1)
        
        XCTAssertEqual(range, NSRange(location: 0, length: 1))
    }
}
