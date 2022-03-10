@testable import Lockbook
import Down
import XCTest

class EditorTest: XCTestCase {

    // MARK: tests for column lookups
    func testColumnLookup1() throws {
        let indexer = IndexConverter("")
        XCTAssertEqual(indexer.columnLookup, [0])
    }
    
    func testColumnLookup2() throws {
        let indexer = IndexConverter("test")
        XCTAssertEqual(indexer.columnLookup, [4])
    }
    
    func testColumnLookup3() throws {
        let indexer = IndexConverter("test\ntest\ntest")
        XCTAssertEqual(indexer.columnLookup, [4, 8, 12])
    }
    
    func testColumnLookup4() throws {
        let indexer = IndexConverter("test\n\ntest")
        XCTAssertEqual(indexer.columnLookup, [4, 4, 8])
    }
    
    // MARK: tests for row col index conversions
    func testRowColIndex1() throws {
        let indexer = IndexConverter("")
        
        XCTAssertEqual(indexer.getUTF8Index(utf8Row: 0, utf8Col: 0), 0)
    }
    
    func testRowColIndex2() throws {
        let indexer = IndexConverter("a")
        
        XCTAssertEqual(indexer.getUTF8Index(utf8Row: 0, utf8Col: 0), 0)
        XCTAssertEqual(indexer.getUTF8Index(utf8Row: 0, utf8Col: 1), 1)
    }
    
    func testRowColIndex3() throws {
        let indexer = IndexConverter("a\na")
        
        XCTAssertEqual(indexer.getUTF8Index(utf8Row: 0, utf8Col: 0), 0)
        XCTAssertEqual(indexer.getUTF8Index(utf8Row: 0, utf8Col: 1), 1)
        XCTAssertEqual(indexer.getUTF8Index(utf8Row: 1, utf8Col: 0), 2)
        XCTAssertEqual(indexer.getUTF8Index(utf8Row: 1, utf8Col: 1), 3)
        XCTAssertEqual(indexer.getUTF8Index(utf8Row: 1, utf8Col: 2), 4)
    }
    
    func testRowColIndex4() throws {
        let indexer = IndexConverter("012\n456\n89")
        
        XCTAssertEqual(indexer.getUTF8Index(utf8Row: 0, utf8Col: 0), 0)
        XCTAssertEqual(indexer.getUTF8Index(utf8Row: 0, utf8Col: 1), 1)
        XCTAssertEqual(indexer.getUTF8Index(utf8Row: 0, utf8Col: 2), 2)
        XCTAssertEqual(indexer.getUTF8Index(utf8Row: 0, utf8Col: 3), 3)
        XCTAssertEqual(indexer.getUTF8Index(utf8Row: 1, utf8Col: 0), 4)
        XCTAssertEqual(indexer.getUTF8Index(utf8Row: 1, utf8Col: 1), 5)
        XCTAssertEqual(indexer.getUTF8Index(utf8Row: 1, utf8Col: 2), 6)
        XCTAssertEqual(indexer.getUTF8Index(utf8Row: 1, utf8Col: 3), 7)
        XCTAssertEqual(indexer.getUTF8Index(utf8Row: 2, utf8Col: 0), 8)
        XCTAssertEqual(indexer.getUTF8Index(utf8Row: 2, utf8Col: 1), 9)
        XCTAssertEqual(indexer.getUTF8Index(utf8Row: 2, utf8Col: 2), 10)
        XCTAssertEqual(indexer.getUTF8Index(utf8Row: 2, utf8Col: 3), 11)
    }
    
    // MARK: tests for range conversions
    func testRange1() throws {
        let indexer = IndexConverter("012")
        
        let range = indexer.getRange(startCol: 1, endCol: 3, startLine: 1, endLine: 1)
        
        XCTAssertEqual(range, NSRange(location: 0, length: 3))
    }
    
    func testRange2() throws {
        let indexer = IndexConverter("012\n456\n89")
        
        let range = indexer.getRange(startCol: 1, endCol: 3, startLine: 1, endLine: 1)
        
        XCTAssertEqual(range, NSRange(location: 0, length: 3))
    }
    
    func testRange3() throws {
        let indexer = IndexConverter("012\n456\n89")
        
        let range = indexer.getRange(startCol: 1, endCol: 3, startLine: 2, endLine: 2)
        
        XCTAssertEqual(range, NSRange(location: 4, length: 3))
    }
    
    func testRange4() throws {
        let indexer = IndexConverter("# test ✌🏿test")
        
        let range = indexer.getRange(startCol: 1, endCol: 18, startLine: 1, endLine: 1)
        
        XCTAssertEqual(range, NSRange(location: 0, length: 14))
    }
    
    let testRange5Input = #"""
# test ✌🏿test

```
test
```

"""#
    func testRange5() throws {
        let indexer = IndexConverter(testRange5Input)
        
        let range = indexer.getRange(startCol: 1, endCol: 3, startLine: 3, endLine: 5)
        
        XCTAssertEqual(range, NSRange(location: 16, length: 12))
    }
}
    
