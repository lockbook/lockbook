import XCTest
import Lockbook

class LockbookTests: XCTestCase {
    let engine = MarkdownEngine()

    override func setUpWithError() throws {
        // Put setup code here. This method is called before the invocation of each test method in the class.
    }

    override func tearDownWithError() throws {
        // Put teardown code here. This method is called after the invocation of each test method in the class.
    }

    func testExample() throws {
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
if (isAwesome){
  return true
}
```
"""
        let attr = engine.render(md)

        print(attr)

    }

    func testPerformanceExample() throws {
        // This is an example of a performance test case.
        measure {
            // Put the code you want to measure the time of here.
        }
    }

}
