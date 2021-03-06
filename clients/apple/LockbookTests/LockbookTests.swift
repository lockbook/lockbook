import XCTest
import Lockbook
import NotepadSwift

class LockbookTests: XCTestCase {
    let engine = MarkdownEngine()

    override func setUpWithError() throws {
        // Put setup code here. This method is called before the invocation of each test method in the class.
    }

    override func tearDownWithError() throws {
        // Put teardown code here. This method is called after the invocation of each test method in the class.
    }

    func testMarkdownAnalyzePerformance() throws {
        let md = """
# You are good
## You are great
### You are wonderful

* I am
* Jonesing
* For a cigarette
  * They are bad for you
  * Don't smoke kids

> I love writing quotes
> I really do

__I love it__

`who am I`

*I hate it*

```
if (isAwesome) {
  return true
}
```
"""
        let expectedNodes = [
            MarkdownNode(range: NSRange(location: 0, length: 14), type: .header, headingLevel: 1),
            MarkdownNode(range: NSRange(location: 15, length: 16), type: .header, headingLevel: 2),
            MarkdownNode(range: NSRange(location: 32, length: 21), type: .header, headingLevel: 3),
            MarkdownNode(range: NSRange(location: 55, length: 6), type: .list),
            MarkdownNode(range: NSRange(location: 62, length: 10), type: .list),
            MarkdownNode(range: NSRange(location: 73, length: 64), type: .list),
            MarkdownNode(range: NSRange(location: 93, length: 22), type: .list),
            MarkdownNode(range: NSRange(location: 118, length: 19), type: .list),
            MarkdownNode(range: NSRange(location: 138, length: 37), type: .quote),
            MarkdownNode(range: NSRange(location: 177, length: 13), type: .bold),
            MarkdownNode(range: NSRange(location: 193, length: 8), type: .code),
            MarkdownNode(range: NSRange(location: 204, length: 11), type: .italic),
            MarkdownNode(range: NSRange(location: 217, length: 40), type: .codeFence)
        ]

        measure {
            XCTAssertEqual(engine.render(md), expectedNodes)
        }
    }

}
