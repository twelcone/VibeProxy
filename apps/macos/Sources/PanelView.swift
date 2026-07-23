// The menubar popover. Its job is the product's core: show how much of the active account's Pro/Max
// quota is used (5-hour + weekly), and switch between accounts. Historical token/dollar analytics live
// in the separate Analytics window, reachable from the footer.

import SwiftUI

struct PanelView: View {
    @EnvironmentObject var state: AppState
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            header
            Divider()
            quota
            Divider()
            accounts
            Divider()
            footer
        }
        .padding(16)
        .frame(width: 340)
        .onAppear { state.refresh() }
    }

    private var header: some View {
        HStack(spacing: 10) {
            Image(systemName: "gauge.with.dots.needle.67percent")
                .font(.title2)
                .foregroundStyle(.tint)
            VStack(alignment: .leading, spacing: 1) {
                Text("VibeProxy").font(.headline)
                Text(state.activeProfile?.label ?? "No account")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Spacer()
            if state.loading {
                ProgressView().controlSize(.small)
            } else {
                Button { state.refresh() } label: { Image(systemName: "arrow.clockwise") }
                    .buttonStyle(.borderless)
                    .help("Refresh")
            }
        }
    }

    // MARK: Quota (the headline)

    @ViewBuilder private var quota: some View {
        switch state.activeUsage?.status {
        case .needsReauth:
            Label("Needs re-login", systemImage: "person.crop.circle.badge.exclamationmark")
                .font(.callout).foregroundStyle(.orange)
        case .error:
            Label("Usage unavailable", systemImage: "wifi.exclamationmark")
                .font(.callout).foregroundStyle(.secondary)
        case .ok:
            if let u = state.activeUsage {
                VStack(spacing: 12) {
                    QuotaRow(title: "5-hour limit", pct: u.fiveHourPct, resets: u.fiveHourResetsAt)
                    QuotaRow(title: "Weekly limit", pct: u.weeklyPct, resets: u.weeklyResetsAt)
                }
            }
        case .none:
            HStack {
                if state.loading { ProgressView().controlSize(.small) }
                Text(state.error ?? "Loading usage…")
                    .font(.callout).foregroundStyle(.secondary)
            }
        }
    }

    // MARK: Accounts

    private var accounts: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text("ACCOUNTS").font(.caption2.weight(.semibold)).foregroundStyle(.secondary)
                Spacer()
                if state.profiles.count > 1 {
                    Text("click to switch").font(.caption2).foregroundStyle(.tertiary)
                }
            }
            if state.profiles.isEmpty {
                Text("No accounts yet.").font(.caption).foregroundStyle(.secondary)
            }
            ForEach(state.profiles) { p in
                AccountRowView(
                    profile: p,
                    isActive: p.id == state.activeId,
                    usage: state.usage[p.id],
                    switchAction: { state.switchTo(p) }
                )
            }
        }
    }

    private var footer: some View {
        HStack {
            Button {
                openWindow(id: "analytics")
                NSApp.activate(ignoringOtherApps: true)
            } label: {
                Label("Analytics", systemImage: "chart.xyaxis.line")
            }
            Spacer()
            Button("Quit") { NSApp.terminate(nil) }
                .buttonStyle(.borderless)
                .foregroundStyle(.secondary)
        }
        .font(.callout)
    }
}

/// One quota limit: a big percentage, a colored progress bar, and when it resets.
struct QuotaRow: View {
    let title: String
    let pct: Double?
    let resets: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(alignment: .firstTextBaseline) {
                Text(title).font(.subheadline.weight(.medium))
                Spacer()
                Text(Fmt.pct(pct))
                    .font(.system(.title3, design: .rounded).weight(.semibold))
                    .foregroundStyle(color)
                    .contentTransition(.numericText())
            }
            ProgressView(value: min(max((pct ?? 0) / 100, 0), 1))
                .tint(color)
            let r = Fmt.resets(resets)
            if !r.isEmpty {
                Text(r).font(.caption2).foregroundStyle(.secondary)
            }
        }
    }

    /// Green under 50%, amber to 80, red past it — the "am I about to run out" signal.
    private var color: Color {
        switch pct ?? 0 {
        case ..<50: return .green
        case ..<80: return .yellow
        default: return .red
        }
    }
}

/// A switchable account: active marker, label/email, and its live 5-hour quota.
struct AccountRowView: View {
    let profile: Profile
    let isActive: Bool
    let usage: ProfileUsage?
    let switchAction: () -> Void

    var body: some View {
        Button(action: switchAction) {
            HStack(spacing: 8) {
                Image(systemName: isActive ? "largecircle.fill.circle" : "circle")
                    .foregroundStyle(isActive ? AnyShapeStyle(.tint) : AnyShapeStyle(.secondary))
                VStack(alignment: .leading, spacing: 0) {
                    Text(profile.label)
                    if let email = profile.email {
                        Text(email).font(.caption2).foregroundStyle(.secondary).lineLimit(1)
                    }
                }
                Spacer()
                if let pct = usage?.fiveHourPct {
                    Text(Fmt.pct(pct))
                        .font(.caption.monospacedDigit())
                        .foregroundStyle(pctColor(pct))
                } else if usage?.status == .needsReauth {
                    Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.orange)
                        .font(.caption)
                }
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(isActive)
    }

    private func pctColor(_ pct: Double) -> Color {
        switch pct {
        case ..<50: return .green
        case ..<80: return .yellow
        default: return .red
        }
    }
}
