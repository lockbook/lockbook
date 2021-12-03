import SwiftUI

struct UsageBanner: View {
    
    @EnvironmentObject var settingsState: SettingsService
    
    var body: some View {
        HStack {
            VStack {
                HStack{
                    if settingsState.usageProgress >= 1 {
                        Text("You have run out of space!")
                    } else {
                        Text("You are running out of space!")
                    }
                    Spacer()
                    Button {
                        settingsState.dismissUsageDialog()
                    } label: {
                        Image(systemName: "xmark.circle")
                            .padding(.horizontal, 5)
                    }
                }

                if settingsState.usageProgress > 0.95 {
                    ProgressView(value: settingsState.usageProgress)
                        .accentColor(Color.red)
                } else if settingsState.usageProgress > 0.85 {
                    ProgressView(value: settingsState.usageProgress)
                        .accentColor(Color.orange)
                } else if settingsState.usageProgress > 0.80 {
                    ProgressView(value: settingsState.usageProgress)
                        .accentColor(Color.yellow)
                } else {
                    ProgressView(value: settingsState.usageProgress)
                        .accentColor(Color.accentColor)
                }
                
            }
            .padding()
            .background(Color.secondaryBackground)
            .cornerRadius(10)
        }
        .padding(15)
        .padding(.bottom, 0)
        
    }
}

extension Color {
    
#if os(macOS)
    static let background = Color(NSColor.windowBackgroundColor)
    static let secondaryBackground = Color(NSColor.underPageBackgroundColor)
    static let tertiaryBackground = Color(NSColor.controlBackgroundColor)
#else
    static let background = Color(UIColor.systemBackground)
    static let secondaryBackground = Color(UIColor.secondarySystemBackground)
    static let tertiaryBackground = Color(UIColor.tertiarySystemBackground)
#endif
}

struct UsageBanner_Previews: PreviewProvider {
    static var previews: some View {
        UsageBanner()
    }
}
