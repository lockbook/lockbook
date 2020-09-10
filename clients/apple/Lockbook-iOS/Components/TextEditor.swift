//
//  TextView.swift
//  ios
//
//  Created by Raayan Pillai on 7/5/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI

struct TextEditor: UIViewRepresentable {
    @Binding var text: String

    func makeCoordinator() -> Coordinator {
        Coordinator(self)
    }

    func makeUIView(context: Context) -> UITextView {

        let myTextView = UITextView()
        myTextView.delegate = context.coordinator

        myTextView.isScrollEnabled = true
        myTextView.isEditable = true
        myTextView.isUserInteractionEnabled = true
        myTextView.backgroundColor = UIColor(white: 0.0, alpha: 0.05)

        return myTextView
    }

    func updateUIView(_ uiView: UITextView, context: Context) {
        let position = uiView.selectedRange
        
        // Styling -- Begin
        let attributes: [NSAttributedString.Key: Any] = [
            NSAttributedString.Key.foregroundColor: UIColor.label,
            NSAttributedString.Key.font: UIFont.monospacedSystemFont(ofSize: 14, weight: .regular)
        ]
        // Styling -- End
        
        // Update text!
        uiView.attributedText = NSAttributedString(string: text, attributes: attributes)
        // Set cursor and scroll to previous!
        uiView.selectedRange = position
        uiView.scrollRangeToVisible(position)
    }

    class Coordinator : NSObject, UITextViewDelegate {

        var parent: TextEditor

        init(_ uiTextView: TextEditor) {
            self.parent = uiTextView
        }

        func textView(_ textView: UITextView, shouldChangeTextIn range: NSRange, replacementText text: String) -> Bool {
            return true
        }

        func textViewDidChange(_ textView: UITextView) {
            self.parent.text = textView.text
        }
    }
}

struct TextView_Previews: PreviewProvider {
    static var previews: some View {
        TextEditor(text: Binding.constant("""
        Lorem ipsum dolor sit amet, consectetur adipiscing elit. Nulla non turpis sed metus posuere luctus a vestibulum eros. Curabitur vitae aliquet leo, eu maximus turpis. Vivamus orci orci, facilisis at orci nec, euismod gravida purus. Integer malesuada nulla quis ante finibus, a viverra tellus posuere. Nam eget enim at turpis malesuada feugiat nec ac metus. Suspendisse vulputate aliquam arcu, nec vehicula sem tincidunt in. Nulla ut lacus ut est tincidunt pulvinar a in augue.

        Suspendisse lacinia ligula vitae tortor aliquam, non faucibus leo fringilla. Duis sed pretium est. Aliquam porttitor ullamcorper pellentesque. Quisque maximus nisi vitae laoreet hendrerit. Orci varius natoque penatibus et magnis dis parturient montes, nascetur ridiculus mus. Nam volutpat quis ligula sit amet bibendum. Duis cursus accumsan metus a dictum. Phasellus rutrum vehicula placerat. Donec ut arcu eu orci fringilla scelerisque nec rutrum ex. Praesent nisi tellus, rutrum vitae nisl at, varius dignissim odio. Pellentesque gravida dolor elementum ex convallis, ac maximus turpis commodo. Nunc justo augue, sagittis quis pulvinar a, posuere sit amet lacus.

        Vivamus facilisis rhoncus tempor. Donec fringilla auctor purus, sed consectetur arcu pellentesque eu. Nulla id facilisis tellus. Vestibulum lobortis ut odio id aliquam. Ut congue consectetur sapien, eget faucibus tellus posuere a. Nam turpis metus, semper at nisl at, posuere lacinia purus. Sed posuere, felis eget efficitur elementum, justo neque tincidunt nisi, sit amet sollicitudin urna quam eu ex. Praesent sed sagittis augue. Morbi pulvinar feugiat facilisis. Phasellus finibus, risus ac malesuada malesuada, odio mi aliquam nisl, et efficitur neque diam non ante. Donec id lacus pellentesque, pharetra nunc a, venenatis nisi. Fusce libero leo, commodo fringilla ultrices fermentum, placerat et dolor. Maecenas ultrices nec sapien sed convallis. Etiam eget nibh viverra, mattis mi sit amet, placerat purus. Sed faucibus ante eros.

        Aenean dolor lorem, volutpat nec fermentum vitae, bibendum non ante. In hac habitasse platea dictumst. Cras metus enim, dignissim ac massa ac, mattis tristique magna. Duis tempor nibh mi, rutrum hendrerit nulla gravida vitae. Curabitur condimentum diam pharetra tempor cursus. Praesent id est nibh. Vestibulum accumsan purus vulputate pellentesque molestie. Pellentesque eget mauris ac erat rutrum mattis vel ac sem. Aliquam velit nisl, luctus ac rhoncus vitae, ultrices eu metus.

        Morbi tincidunt iaculis odio. Praesent accumsan varius lorem, non congue turpis bibendum vitae. Phasellus et malesuada ligula. Cras pretium commodo orci eu mattis. Mauris et ex varius, placerat nisi id, placerat sem. Fusce ut ex euismod, mollis turpis sit amet, semper nunc. Donec lectus neque, eleifend at felis et, pretium pretium diam. Integer maximus lacinia justo eu porttitor.
        """))
            .previewLayout(.sizeThatFits)
    }
}
