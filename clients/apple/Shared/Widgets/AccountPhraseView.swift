import SwiftUI

struct AccountPhraseView: View {
    
    let accountPhrasePart1: [String]
    let accountPhrasePart2: [String]
    let error: String?
    let includeBackground: Bool
    
    init(includeBackground: Bool = true) {
        self.includeBackground = includeBackground
        
        switch AppState.lb.exportAccountPhrase() {
        case .success(let accountPhrase):
            let accountPhrase = accountPhrase.split(separator: " ")
            let first12 = Array(accountPhrase.prefix(12)).enumerated().map { (index, item) in
                return "\(index + 1). \(item)"
            }.joined(separator: "\n")
            
            let last12 = Array(accountPhrase.suffix(12)).enumerated().map { (index, item) in
                return "\(index + 13). \(item)"
            }.joined(separator: "\n")
            
            accountPhrasePart1 = first12.components(separatedBy: "\n")
            accountPhrasePart2 = last12.components(separatedBy: "\n")
            error = nil
        case .failure(let err):
            error = err.msg
            accountPhrasePart1 = []
            accountPhrasePart2 = []
        }
    }
    
    var body: some View {
        HStack {
            VStack(alignment: .leading) {
                ForEach(accountPhrasePart1, id: \.self) { phrase in
                    keyText(from: phrase)
                }
            }
            .padding(.leading, 30)
            
            Spacer()
            
            VStack(alignment: .leading) {
                ForEach(accountPhrasePart2, id: \.self) { phrase in
                    keyText(from: phrase)
                }
            }
            .padding(.trailing, 30)
        }
        .frame(maxWidth: 350)
        .padding()
        .modifier(AccountPhraseIncludeBackgroundViewModifier(includeBackground: includeBackground))
    }
    
    @ViewBuilder
    func keyText(from phrase: String) -> some View {
        let components = phrase.split(separator: " ", maxSplits: 1)
        
        if components.count == 2 {
            let number = components[0]
            let word = components[1]
            
            HStack {
                Text("\(number)")
                    .foregroundColor(.accentColor)
                
                Text(word)
                    .foregroundColor(.primary)
                    .font(.system(.callout, design: .monospaced))
            }
        }
    }
}

struct AccountPhraseIncludeBackgroundViewModifier: ViewModifier {
    let includeBackground: Bool
    
    func body(content: Content) -> some View {
        if includeBackground {
            content
                .background(RoundedRectangle(cornerRadius: 6).foregroundStyle(.gray).opacity(0.1))
        } else {
            content
        }
    }
}

#Preview("Account Phrase") {
    AccountPhraseView()
}

#Preview("Account Phrase - Exclude Background") {
    AccountPhraseView(includeBackground: false)
}

