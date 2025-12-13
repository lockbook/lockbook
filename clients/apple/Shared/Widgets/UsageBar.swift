import SwiftUI
import SwiftWorkspace

struct UsageBar: View {
    @EnvironmentObject var settingsModel: SettingsViewModel
    @AppStorage("usageBarMode") var usageBarMode: UsageBarDisplayMode =
        .whenHalf

    var body: some View {
        if let usage = settingsModel.usage, let usageMessage,
            let progressBarColor, shouldShow
        {
            VStack(alignment: .leading, spacing: 5) {
                #if os(iOS)
                    Text(usageMessage)
                        .font(.callout)
                        .foregroundStyle(.secondary)
                #endif

                ProgressView(
                    value: Double(usage.serverUsedExact),
                    total: Double(usage.serverCapExact)
                )
                .tint(progressBarColor)
                .padding(.bottom, 8)
                .modifier(UsageTooltip(usageMessage: usageMessage))
            }
            .modifier(MobileGlassEffectModifier())
        }
    }

    private var progressBarColor: Color? {
        guard let usage = settingsModel.usage else { return nil }

        let percentUsed =
            Double(usage.serverUsedExact) / Double(usage.serverCapExact)

        if percentUsed >= 0.7 {
            return .yellow
        }

        return .accentColor
    }

    private var usageMessage: String? {
        guard let usage = settingsModel.usage else { return nil }

        return "\(usage.serverUsedHuman) / \(usage.serverCapHuman)"
    }

    private var shouldShow: Bool {
        guard let usage = settingsModel.usage else { return false }

        switch usageBarMode {
        case .always:
            return true
        case .never:
            return false
        case .whenHalf:
            let percentUsed =
                Double(usage.serverUsedExact) / Double(usage.serverCapExact)
            return percentUsed >= 0.5
        }
    }
}

struct UsageTooltip: ViewModifier {
    let usageMessage: String

    func body(content: Content) -> some View {
        #if os(macOS)
            content
                .help(usageMessage)
        #else
            content
        #endif
    }
}

struct MobileGlassEffectModifier: ViewModifier {
    func body(content: Content) -> some View {
        #if os(iOS)
        content
            .frame(height: 40)
            .padding(.vertical, 8)
            .padding(.horizontal)
            .modifier(GlassEffectModifier())
            .padding(.horizontal)
        #else
        content
            .padding(8)
        #endif
    }
}

#Preview("UsageBar") {
    UserDefaults.standard.set("always", forKey: "usageBarMode")

    return UsageBar()
        .withCommonPreviewEnvironment()
        .withMacPreviewSize()
        .padding(.horizontal)
}
