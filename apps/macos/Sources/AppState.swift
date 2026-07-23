// Observable app state. FFI calls read files and the Keychain, so they run off the main actor and
// publish results back. One instance is shared by the menubar panel and the analytics window.

import Foundation
import SwiftUI

/// Preset date windows, matching the web UI's range chips.
enum RangePreset: String, CaseIterable, Identifiable {
    case sevenDays = "7d"
    case thirtyDays = "30d"
    case ninetyDays = "90d"
    case all = "All"
    var id: String { rawValue }

    var label: String {
        switch self {
        case .sevenDays: return "7 days"
        case .thirtyDays: return "30 days"
        case .ninetyDays: return "90 days"
        case .all: return "All time"
        }
    }

    /// "FROM..TO" for the FFI, or nil for all time. `today` is injected so the type stays testable.
    func ffiRange(today: Date = Date()) -> String? {
        let days: Int
        switch self {
        case .sevenDays: days = 7
        case .thirtyDays: days = 30
        case .ninetyDays: days = 90
        case .all: return nil
        }
        let cal = Calendar.current
        guard let start = cal.date(byAdding: .day, value: -(days - 1), to: today) else { return nil }
        let f = DateFormatter()
        f.dateFormat = "yyyy-MM-dd"
        return "\(f.string(from: start)).."
    }
}

@MainActor
final class AppState: ObservableObject {
    @Published var profiles: [Profile] = []
    @Published var activeId: String?
    @Published var analytics: Analytics = .empty
    @Published var range: RangePreset = .thirtyDays
    @Published var loading = false
    @Published var error: String?
    @Published var coreVersion: String = Core.version

    var activeProfile: Profile? { profiles.first { $0.id == activeId } }

    init() {
        // Load once at launch so the menu-bar value is live before the popover is ever opened.
        refresh()
    }

    func refresh() {
        loading = true
        error = nil
        let range = self.range
        Task.detached(priority: .userInitiated) {
            do {
                let profiles = try Core.profiles()
                let activeId = Core.activeProfile()
                let analytics = try Core.usage(range: range.ffiRange())
                await MainActor.run {
                    self.profiles = profiles
                    self.activeId = activeId
                    self.analytics = analytics
                    self.loading = false
                }
            } catch {
                await MainActor.run {
                    self.error = String(describing: error)
                    self.loading = false
                }
            }
        }
    }

    func setRange(_ preset: RangePreset) {
        guard preset != range else { return }
        range = preset
        refresh()
    }

    func switchTo(_ profile: Profile) {
        Task.detached(priority: .userInitiated) {
            do {
                try Core.activate(profile.id)
                await MainActor.run { self.refresh() }
            } catch {
                await MainActor.run { self.error = String(describing: error) }
            }
        }
    }
}
