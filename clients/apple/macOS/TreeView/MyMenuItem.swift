import AppKit
import SwiftLockbookCore

class MyMenuItem: NSMenuItem {
    let file: DecryptedFileMetadata
    init(file: DecryptedFileMetadata) {
        self.file = file
        super.init(title: "Rename", action: #selector(rename(_:)), keyEquivalent: "")
        target = self
    }

    required init(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    @objc func rename(_ sender: AnyObject) {
        print(file)
    }

}