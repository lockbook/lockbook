import Foundation
import SwiftUI
import SwiftLockbookCore

struct SuggestedDocs: View {
    @StateObject var suggestedDocsBranchState: BranchState = BranchState(open: true)
    
    @EnvironmentObject var current: CurrentDocument
    @EnvironmentObject var fileService: FileService
    
    var body: some View {
        Group {
            if fileService.suggestedDocs?.isEmpty != true {
                Button(action: {
                    withAnimation {
                        suggestedDocsBranchState.open.toggle()
                    }
                }) {
                    HStack {
                        Text("Suggested")
                            .bold()
                            .foregroundColor(.gray)
                            .font(.subheadline)
                        Spacer()
                        if suggestedDocsBranchState.open {
                            Image(systemName: "chevron.down")
                                .foregroundColor(.gray)
                                .imageScale(.small)
                        } else {
                            Image(systemName: "chevron.right")
                                .foregroundColor(.gray)
                                .imageScale(.small)
                        }
                    }
                    .padding(.horizontal)
                    .contentShape(Rectangle())
                }
                
                if suggestedDocsBranchState.open {
                    if let suggestedDocs = fileService.suggestedDocs {
                        ForEach(suggestedDocs) { meta in
                            Button(action: {
                                current.selectedDocument = meta
                            }) {
                                macOSSuggestedDocCell(name: meta.name, duration: meta.lastModified)
                            }
                        }
                    } else {
                        ForEach(0...3, id: \.self) { index in
                            macOSSuggestedDocLoadingCell()
                        }
                    }
                }
            }
        }
    }
}

struct macOSSuggestedDocCell: View {
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
        .contentShape(Rectangle())
        .frame(height: 32)
    }
}

struct macOSSuggestedDocLoadingCell: View {
    var body: some View {
        HStack {
            RoundedRectangle(cornerSize: CGSize(width: 5, height: 5))
                .fill(.gray)
                .opacity(0.1)
                .cornerRadius(5)
                .frame(width: 25, height: 20)
            
            VStack(alignment: .leading) {
                RoundedRectangle(cornerSize: CGSize(width: 5, height: 5))
                    .fill(.gray)
                    .opacity(0.1)
                    .cornerRadius(5)
                    .frame(width: 60, height: 13)
                
                RoundedRectangle(cornerSize: CGSize(width: 5, height: 5))
                    .fill(.gray)
                    .opacity(0.1)
                    .cornerRadius(5)
                    .frame(width: 100, height: 13)
            }
            .padding(.leading, 5)
            
            Spacer()
        }
        .padding(.horizontal)
        .contentShape(Rectangle())
        .frame(height: 32)
    }
}

struct SuggestedDocLoadingCellPreview: PreviewProvider {

    static var previews: some View {
        
        macOSSuggestedDocCell(name: "Cookie monster", duration: 1000)
    }
}

