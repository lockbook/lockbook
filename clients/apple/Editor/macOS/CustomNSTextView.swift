import AppKit

public class CustomNSTextView: NSTextView {
    
    override public func insertText(_ maybeString: Any, replacementRange: NSRange) {
        guard let storage = self.textStorage as? Storage else {
            print("Unexpected storage type")
            return super.insertText(string, replacementRange: replacementRange)
        }
        
        guard let assistant = storage.parser?.typeAssist else {
            print("storage does not have a parser yet")
            return super.insertText(string, replacementRange: replacementRange)
        }
        
        guard let string = maybeString as? String else {
            print("insertText with a non String type called")
            return super.insertText(string, replacementRange: replacementRange)
        }
        
        if assistant.mightAssist(string) {
            let assist = assistant.assist(string, replacementRange)
            super.insertText(assist.0, replacementRange: assist.1)
        } else {
            super.insertText(maybeString, replacementRange: replacementRange)
        }
    }
    
    override public func doCommand(by selector: Selector) {
        if selector == #selector(insertBacktab(_:)) {
            let string = self.string as NSString
            let line = string.lineRange(for: NSMakeRange(selectedRange().location, 0))
            if string.substring(with: line).prefix(1) == "\t" {
                super.insertText("", replacementRange: NSRange(location: line.location, length: 1))
            }
            
        } else {
            super.doCommand(by: selector)
        }
    }
}
