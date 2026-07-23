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
            // Gauge + the active account's 5-hour quota % — the product's headline number, glanceable
            // in the bar itself. Two views (image + text) so MenuBarExtra renders the value, not just
            // the icon. Falls back to a dash when the reading is unavailable.
            Image(systemName: "gauge.with.dots.needle.67percent")
            Text(Fmt.pct(state.activeUsage?.fiveHourPct))
        }
        .menuBarExtraStyle(.window)

        Window("Usage Analytics", id: "analytics") {
            AnalyticsView().environmentObject(state)
        }
        .windowResizability(.contentMinSize)
    }
}
