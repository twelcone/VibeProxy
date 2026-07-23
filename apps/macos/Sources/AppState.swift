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
    @Published var shellInstalled = true  // assume yes until checked, so we don't flash the hint
    @Published var coreVersion: String = Core.version

    private var pollTask: Task<Void, Never>?

    var activeProfile: Profile? { profiles.first { $0.id == activeId } }
    var activeUsage: ProfileUsage? { activeId.flatMap { usage[$0] } }

    init() {
        // Clean up any abandoned add, adopt ~/.claude as "Main" on first run, then load live quota so
        // the menu bar shows a real percentage before the popover is ever opened.
        Core.sweepOrphans()
        try? Core.bootstrap()
        shellInstalled = Core.isShellInstalled()
        refresh()
        startPolling()
    }

    /// Keep the menu-bar quota fresh without the user opening the popover. Quota-only (light); the
    /// heavier analytics scan stays on-demand.
    private func startPolling() {
        pollTask = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: 120_000_000_000)  // 2 minutes
                await MainActor.run { self?.refreshUsage() }
            }
        }
    }

    /// Install the shell integration so switches reach the user's own new terminals.
    func setUpShell() {
        Task.detached(priority: .userInitiated) {
            let ok = (try? Core.setUpShell()) != nil
            await MainActor.run { if ok { self.shellInstalled = true } }
        }
    }

    /// Poll live quota separately from the heavier analytics scan — it drives the menu bar and should
    /// stay responsive. Reused by refresh() and by a periodic tick.
    /// Minimum gap between usage polls. The endpoint rate-limits (429) and the plan calls for
    /// conservative polling, so opening the popover repeatedly reuses the cached reading rather than
    /// re-hitting the endpoint each time. The manual Refresh button and the 2-minute timer force it.
    private var lastUsagePoll: Date = .distantPast
    private static let minPollGap: TimeInterval = 60

    func refreshUsage(force: Bool = false) {
        if !force && Date().timeIntervalSince(lastUsagePoll) < Self.minPollGap { return }
        lastUsagePoll = Date()
        Task.detached(priority: .userInitiated) {
            let fresh = (try? Core.usageAll()) ?? []
            await MainActor.run {
                // Rebuild from the fresh poll (so a removed account drops out), but on a *transient*
                // error keep the last good reading rather than blanking the % — the endpoint rate-
                // limits rapid polls, and a momentary blip shouldn't erase a known-good value.
                var next: [String: ProfileUsage] = [:]
                for u in fresh {
                    if u.status == .error, let prev = self.usage[u.profileId], prev.status == .ok {
                        next[u.profileId] = prev
                    } else {
                        next[u.profileId] = u
                    }
                }
                self.usage = next
            }
        }
    }

    func refresh(force: Bool = false) {
        loading = true
        error = nil
        let range = self.range
        refreshUsage(force: force)
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
                await MainActor.run { self.refresh(force: true) }
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
