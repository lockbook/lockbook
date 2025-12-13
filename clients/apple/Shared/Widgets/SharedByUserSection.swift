import SwiftUI
import SwiftWorkspace

struct SharedByUserSection<FileRow: View>: View {
    let username: String
    let shares: [File]
    @ViewBuilder var fileRow: (File) -> FileRow
    
    var body: some View {
        CollapsableSection(
            id: "Shared_\(username)",
            label: {
                Text(username)
                    .bold()
                    .foregroundColor(.primary)
                    .textCase(.none)
                    .font(.headline)
                    .padding(.bottom, 3)
                    .padding(.top, 8)
            },
            content: {
                VStack(spacing: 0) {
                    ForEach(
                        shares,
                        content: { file in
                            fileRow(file)
                        }
                    )
                }
                .padding(.leading)
            }
        )
    }
}
