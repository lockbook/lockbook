import UIKit

public class CustomUITextView: UITextView {
    override public func insertText(_ string: String) {
        let replacementRange = self.selectedRange
        
        guard let storage = self.textStorage as? Storage else {
            print("Unexpected storage type")
            return super.insertText(string)
        }
        
        guard let assistant = storage.parser?.typeAssist else {
            print("storage does not have a parser yet")
            return super.insertText(string)
        }
        
        if assistant.mightAssist(string) {
            let assist = assistant.assist(string, replacementRange)
            super.replace(normalRangeToStupidRange(assist.1), withText: assist.0)
            if string == "\t" {
                self.selectedRange = NSRange(location: replacementRange.location + 1, length: replacementRange.length)
            }
        } else {
            super.insertText(string)
        }
    }
    
    override public func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        if presses.count == 1 {
            let key = presses.first!
            if key.key?.charactersIgnoringModifiers == "\t" && key.key?.modifierFlags == .shift {
                let string = self.text as NSString
                let line = string.lineRange(for: NSMakeRange(self.selectedRange.location, 0))
                if string.substring(with: line).prefix(1) == "\t" {
                    let originalCursorPosition = self.selectedRange
                    super.replace(normalRangeToStupidRange(NSRange(location: line.location, length: 1)), withText: "")
                    self.selectedRange = NSRange(location: originalCursorPosition.location - 1, length: 0)
                }
            } else {
                super.pressesBegan(presses, with: event)
            }
        } else {
            super.pressesBegan(presses, with: event)
        }
    }
    
    private func normalRangeToStupidRange(_ range: NSRange) -> UITextRange {
        let head = self.beginningOfDocument
        let start = self.position(from: head, offset: range.location)!
        let end = self.position(from: start, offset: range.length)!
        
        return self.textRange(from: start, to: end)!
    }
}
