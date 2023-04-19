import SwiftUI
import SwiftLockbookCore

struct SuggestedDocumentCell: View {
    let name: String
    let duration: UInt64
    
    var body: some View {
        HStack {
            Image(systemName: "doc.circle")
                .resizable()
                .scaledToFill()
                .frame(width: 21, height: 21)
                .foregroundColor(.accentColor)
            
            VStack(alignment: .leading) {
                Text(name)
                    .font(.callout)
                
                Text(timeAgo(epoch: duration))
                    .foregroundColor(.gray)
                    .font(.callout)
            }
            .padding(.leading, 5)
            
            Spacer()
        }
        .padding(.horizontal)
        .padding(.top, 1)
            .contentShape(Rectangle()) /// https://stackoverflow.com/questions/57258371/swiftui-increase-tap-drag-area-for-user-interaction
    }
}

struct SuggestedDocumentCell_Previews: PreviewProvider {
    static var previews: some View {
        VStack {
            HStack {
                Text("Suggested Documents")
                    .bold()
                    .foregroundColor(.gray)
                    .font(.subheadline)
                Spacer()
                Image(systemName: "chevron.down")
                    .foregroundColor(.gray)
            }
            .padding(.bottom)
            .padding(.horizontal)
            .contentShape(Rectangle())
            
            VStack(alignment: .leading) {
                HStack {
                    SuggestedDocumentCell(name: "Cookie", duration: 1681873267000)
                    Spacer()
                }
            }
        }
    }
}
