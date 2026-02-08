import SwiftUI

struct OutOfSpaceAlert: ViewModifier {
    @EnvironmentObject var homeState: HomeState
    
    @AppStorage("hideOutOfSpaceSheet") private var hideOutOfSpaceSheet: Bool = false

    func body(content: Content) -> some View {
        content
            .sheet(isPresented: Binding(
                get: { homeState.showOutOfSpaceAlert },
                set: { newValue in
                    if !newValue {
                        homeState.dismissOutOfSpaceAlert()
                    }
                }
            )) {
                OutOfSpaceSheet()
            }
    }
}

struct OutOfSpaceSheet: View {
    #if os(macOS)
        @Environment(\.openWindow) private var openWindow
    #endif

    @EnvironmentObject var homeState: HomeState

    @AppStorage("hideOutOfSpaceSheet") private var hideOutOfSpaceSheet: Bool = false

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            VStack(alignment: .leading, spacing: 8) {
                Text("You are out of storage")
                    .font(.title)
                    .fontWeight(.bold)
                    .padding(.bottom)

                Text("Syncing is paused because your account is out of storage.")

                Text("To resume syncing, clear some data or **upgrade to premium**.")

                Text("Premium gives you **30 GB of secure, encrypted storage** - perfect for all your notes, files, and memories.")
            }
            .multilineTextAlignment(.leading)

            Spacer()
            
            Toggle(isOn: $hideOutOfSpaceSheet) {
                Text("Don’t show this again")
                    .font(.callout)
                    .foregroundStyle(.primary)
            }
            .toggleStyle(iOSCheckboxToggleStyle())
            .padding(.vertical, 8)

            VStack(spacing: 12) {
                Button {
                    #if os(iOS)
                        homeState.showUpgradeAccount = true
                        homeState.sidebarState = .closed
                    #else
                        openWindow(id: "upgrade-account")
                    #endif
                    homeState.dismissOutOfSpaceAlert()
                } label: {
                    Text("Upgrade to Premium")
                        .fontWeight(.semibold)
                        .frame(maxWidth: .infinity)
                        .frame(height: 30)
                }
                .buttonStyle(.borderedProminent)
                .padding(.bottom, 6)

                Button {
                    homeState.dismissOutOfSpaceAlert()
                } label: {
                    Text("Ignore for now")
                        .fontWeight(.semibold)
                        .frame(maxWidth: .infinity)
                        .frame(height: 30)
                }
                .buttonStyle(.bordered)
                .padding(.bottom)
            }
        }
        .padding(.top, 35)
        .padding(.horizontal, 25)
        .padding(.bottom)
        .presentationDetents([.fraction(0.70)])
    }
}
