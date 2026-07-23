// VibeProxy menubar app. A MenuBarExtra popover for the at-a-glance summary and account switching,
// plus a full analytics Window. Both render the shared AppState, fed by the Rust core over uniffi.

import SwiftUI

@main
struct VibeProxyApp: App {
    @StateObject private var state = AppState()

    var body: some Scene {
        MenuBarExtra {
            PanelView().environmentObject(state)
        } label: {
            // Icon + live spend, so the bar itself is glanceable. Two views (image + text) rather
            // than a Label, so MenuBarExtra renders the value text and not just the icon.
            Image(systemName: "gauge.with.dots.needle.67percent")
            Text(Fmt.usd(state.analytics.totalValue))
        }
        .menuBarExtraStyle(.window)

        Window("Usage Analytics", id: "analytics") {
            AnalyticsView().environmentObject(state)
        }
        .windowResizability(.contentMinSize)
    }
}
