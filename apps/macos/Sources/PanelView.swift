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
                Button { state.refresh(force: true) } label: { Image(systemName: "arrow.clockwise") }
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
            Text("SWITCH ACCOUNT")
                .font(.caption2.weight(.semibold)).foregroundStyle(.secondary)
            if state.profiles.isEmpty {
                Text("No accounts yet — add one below.").font(.caption).foregroundStyle(.secondary)
            }
            ForEach(state.profiles) { p in
                AccountRowView(
                    profile: p,
                    isActive: p.id == state.activeId,
                    usage: state.usage[p.id],
                    switchAction: { state.switchTo(p) },
                    removeAction: { state.removeAccount(p) }
                )
            }
            if state.activeProfile != nil {
                Button(action: { state.openClaude() }) {
                    Label("Open Claude on \(state.activeProfile?.label ?? "active")", systemImage: "terminal")
                        .frame(maxWidth: .infinity)
                }
                .controlSize(.large)
                .buttonStyle(.borderedProminent)
                .help("Open a terminal running Claude on the active account, so the switch takes effect now")
                .padding(.top, 2)
            }
            if !state.shellInstalled {
                HStack(alignment: .top, spacing: 6) {
                    Image(systemName: "info.circle").font(.caption2).foregroundStyle(.secondary)
                    Text("Set up shell integration so switches reach your own new terminals too.")
                        .font(.caption2).foregroundStyle(.secondary)
                    Button("Set up") { state.setUpShell() }.font(.caption2).buttonStyle(.link)
                }
                .padding(.top, 2)
            }
            addControl
        }
    }

    @ViewBuilder private var addControl: some View {
        if state.adding {
            HStack(spacing: 8) {
                ProgressView().controlSize(.small)
                Text(state.addMessage ?? "Adding account…")
                    .font(.caption).foregroundStyle(.secondary).lineLimit(2)
                Spacer()
                Button("Cancel") { state.cancelAdd() }
                    .buttonStyle(.borderless).font(.caption)
            }
            .padding(.top, 2)
        } else {
            HStack {
                Button(action: { state.addAccount() }) {
                    Label("Add account", systemImage: "plus.circle")
                }
                .buttonStyle(.borderless)
                Spacer()
                if let msg = state.addMessage {
                    Text(msg).font(.caption2).foregroundStyle(.secondary).lineLimit(1)
                }
            }
            .font(.callout)
            .padding(.top, 2)
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
    let removeAction: () -> Void

    var body: some View {
        Button(action: switchAction) {
            HStack(spacing: 8) {
                Image(systemName: isActive ? "checkmark.circle.fill" : "circle")
                    .foregroundStyle(isActive ? AnyShapeStyle(.green) : AnyShapeStyle(.secondary))
                VStack(alignment: .leading, spacing: 0) {
                    Text(profile.label).fontWeight(isActive ? .semibold : .regular)
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
                // The affordance: active reads as "Active", the rest as a tappable "Switch".
                if isActive {
                    Text("Active").font(.caption2.weight(.semibold)).foregroundStyle(.green)
                } else {
                    Text("Switch").font(.caption2.weight(.semibold)).foregroundStyle(.tint)
                    Image(systemName: "chevron.right").font(.caption2).foregroundStyle(.tertiary)
                }
            }
            .padding(.vertical, 5).padding(.horizontal, 8)
            .background(isActive ? AnyShapeStyle(.green.opacity(0.10)) : AnyShapeStyle(.quaternary.opacity(0.4)),
                        in: RoundedRectangle(cornerRadius: 7))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(isActive)
        .contextMenu {
            Button("Remove account", role: .destructive, action: removeAction)
        }
    }

    private func pctColor(_ pct: Double) -> Color {
        switch pct {
        case ..<50: return .green
        case ..<80: return .yellow
        default: return .red
        }
    }
}
