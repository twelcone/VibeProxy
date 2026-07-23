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
    @Published var usage: [String: ProfileUsage] = [:]  // by profileId
    @Published var analytics: Analytics = .empty
    @Published var range: RangePreset = .thirtyDays
    @Published var loading = false
    @Published var error: String?
    @Published var coreVersion: String = Core.version

    var activeProfile: Profile? { profiles.first { $0.id == activeId } }
    var activeUsage: ProfileUsage? { activeId.flatMap { usage[$0] } }

    init() {
        // Adopt ~/.claude as "Main" on first run, then load live quota so the menu bar shows a real
        // percentage before the popover is ever opened.
        try? Core.bootstrap()
        refresh()
    }

    /// Poll live quota separately from the heavier analytics scan — it drives the menu bar and should
    /// stay responsive. Reused by refresh() and by a periodic tick.
    func refreshUsage() {
        Task.detached(priority: .userInitiated) {
            let usage = (try? Core.usageAll()) ?? []
            let byId = Dictionary(uniqueKeysWithValues: usage.map { ($0.profileId, $0) })
            await MainActor.run { self.usage = byId }
        }
    }

    func refresh() {
        loading = true
        error = nil
        let range = self.range
        refreshUsage()
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

    // MARK: Add / remove accounts

    @Published var adding = false
    @Published var addMessage: String?
    private var pendingDir: String?
    private var addTask: Task<Void, Never>?

    /// Start the add-account flow: open Claude's login in Terminal, then poll until it completes and
    /// register the account. Runs independent of the popover's lifetime, so closing it doesn't abort.
    func addAccount() {
        guard !adding else { return }
        adding = true
        addMessage = "Opening Claude login…"
        addTask = Task.detached { [weak self] in
            guard let self else { return }
            do {
                let dir = try Core.beginAdd()
                await MainActor.run {
                    self.pendingDir = dir
                    self.addMessage = "Complete the login in Terminal…"
                }
                // Poll ~3 minutes for the browser OAuth to land credentials in the Keychain.
                for _ in 0..<90 {
                    if Task.isCancelled { return }
                    try await Task.sleep(nanoseconds: 2_000_000_000)
                    if let status = try? Core.checkLogin(dir), status.loggedIn {
                        let count = await MainActor.run { self.profiles.count }
                        try Core.adopt(label: status.email ?? "Account \(count + 1)", dir: dir)
                        await MainActor.run { self.resetAdd(nil); self.refresh() }
                        return
                    }
                }
                try? Core.cancelAdd(dir)
                await MainActor.run { self.resetAdd("Login timed out — try again") }
            } catch {
                if let dir = await MainActor.run(body: { self.pendingDir }) {
                    try? Core.cancelAdd(dir)
                }
                await MainActor.run { self.resetAdd(String(describing: error)) }
            }
        }
    }

    func cancelAdd() {
        addTask?.cancel()
        let dir = pendingDir
        Task.detached { if let dir { try? Core.cancelAdd(dir) } }
        resetAdd(nil)
    }

    /// Open Claude on the active account so the switch takes effect immediately.
    func openClaude() {
        Task.detached(priority: .userInitiated) {
            do { try Core.openClaude() }
            catch { await MainActor.run { self.error = String(describing: error) } }
        }
    }

    func removeAccount(_ profile: Profile) {
        Task.detached(priority: .userInitiated) {
            do {
                try Core.remove(profile.id)
                await MainActor.run { self.refresh() }
            } catch {
                await MainActor.run { self.error = String(describing: error) }
            }
        }
    }

    private func resetAdd(_ message: String?) {
        adding = false
        addMessage = message
        pendingDir = nil
        addTask = nil
    }
}
