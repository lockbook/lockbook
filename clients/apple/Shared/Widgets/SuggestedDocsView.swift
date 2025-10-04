import SwiftUI
import SwiftWorkspace

struct SuggestedDocsView: View {
    @Environment(\.horizontalSizeClass) var horizontalSizeClass
    
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var workspaceInput: WorkspaceInputState
    
    @StateObject var model: SuggestedDocsViewModel
    
    init(filesModel: FilesViewModel) {
        self._model = StateObject(wrappedValue: SuggestedDocsViewModel(filesModel: filesModel))
    }
    
    var body: some View {
        if model.suggestedDocs?.isEmpty == true {
            Text("You have no suggested documents.")
                .font(.caption)
                .foregroundStyle(.secondary)
                .padding(.horizontal)
        } else {
            ScrollView(.horizontal) {
                HStack {
                    if let suggestedDocs = model.suggestedDocs {
                        ForEach(suggestedDocs) { info in
                            Button(action: {
                                DispatchQueue.main.async {
                                    if homeState.isSidebarFloating {
                                        homeState.sidebarState = .closed
                                    }
                                    
                                    workspaceInput.openFile(id: info.id)
                                }
                            }) {
                                SuggestedDocCell(info: info, model: model)
                            }
                            .buttonStyle(.plain)
                        }
                    } else {
                        ForEach(0...2, id: \.self) { index in
                            SuggestedDocLoadingCell()
                        }
                    }
                }
                .frame(height: 80)
                .padding(.horizontal)
            }
            .listRowBackground(Color.clear)
            .listRowInsets(EdgeInsets())
        }
    }
}

struct SuggestedDocCell: View {
    let info: SuggestedDocInfo
    
    @Environment(\.colorScheme) var colorScheme
    
    @StateObject var model: SuggestedDocsViewModel
    
    var body: some View {
        VStack(alignment: .leading) {
            Text(info.name)
                .foregroundColor(.primary)
            
            HStack {
                Text(info.parentName)
                    .font(.caption)
                    .foregroundColor(.accentColor)
                
                Spacer()
                
                Text(info.lastModified)
                    .font(.caption)
                    .foregroundColor(.gray)
            }
            .padding(.top, 1)
        }
        .contentShape(Rectangle())
        .frame(maxWidth: 200)
        .modifier(SuggestedDocBackground())
        .contextMenu {
            Button {
                model.clearSuggestedDoc(id: info.id)
            } label: {
                Label("Remove", systemImage: "xmark.circle")
            }
            Button {
                model.clearSuggestedDocs()
            } label: {
                Label("Clear Suggestions", systemImage: "xmark.circle.fill")
            }
        }
    }
}

struct SuggestedDocLoadingCell: View {
    @Environment(\.colorScheme) var colorScheme
    
    var body: some View {
        VStack(alignment: .leading) {
            RoundedRectangle(cornerSize: CGSize(width: 5, height: 5))
                .fill(.gray)
                .opacity(0.1)
                .cornerRadius(5)
                .frame(width: 70, height: 16)
            
            HStack {
                RoundedRectangle(cornerSize: CGSize(width: 5, height: 5))
                    .fill(.gray)
                    .opacity(0.1)
                    .cornerRadius(5)
                    .frame(width: 70, height: 16)
                
                Spacer()
                
                RoundedRectangle(cornerSize: CGSize(width: 5, height: 5))
                    .fill(.gray)
                    .opacity(0.1)
                    .cornerRadius(5)
                    .frame(width: 40, height: 16)
            }
            .padding(.top, 1)
        }
        .padding(12)
        .contentShape(Rectangle())
        .modifier(SuggestedDocBackground())
    }
}

struct SuggestedDocBackground: ViewModifier {
    @Environment(\.colorScheme) var colorScheme
    
    func body(content: Content) -> some View {
        content
            .padding(12)
            .background(
                RoundedRectangle(cornerRadius: 10)
                    .fill(colorScheme == .light ? Color.accentColor.opacity(0.08) : Color.accentColor.opacity(0.19))
            )
    }
}

#Preview {
    SuggestedDocsView(filesModel: .preview)
        .withCommonPreviewEnvironment()
}
