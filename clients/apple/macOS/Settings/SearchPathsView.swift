import Foundation
import SwiftUI

struct SearchPathsView: View {
    
    @EnvironmentObject var search: SearchService
    
    @State private var query = ""
    
    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            
            let _ = print("SHOWING THIS SHIT?")
            
            HStack {
                Image(systemName: "magnifyingglass")
                TextField("Search file path...", text: Binding(
                    get: { self.query },
                    set: { self.query = $0; self.newSearch(query: $0) }
                ))
                    .padding(7)
                    .padding(.horizontal, 25)
                    .background(Color(.systemGray))
                    .cornerRadius(8)
                    .padding(.horizontal, 10)
            }
            
            List(search.searchResults) { searchResult in
                Button(action: {
                    print("CLICKED \(searchResult.path)")
                }) {
                    VStack(alignment: .leading, spacing: 5) {
                        Text(searchResult.path)
                            .font(.title3)
                    }
                    .padding(.vertical, 5)
                    .contentShape(Rectangle()) /// https://stackoverflow.com/questions/57258371/swiftui-increase-tap-drag-area-for-user-interaction
                }
            }
        }
        
    }
    
    func newSearch(query: String) {
        search.searchFilePath(input: query)
    }
}

