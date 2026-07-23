// The menubar popover. A compact summary: active account, spend for the range, a token breakdown,
// the account switcher, and an entry point to the full analytics window.

import SwiftUI

struct PanelView: View {
    @EnvironmentObject var state: AppState
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            header
            rangePicker
            Divider()
            summary
            if !state.profiles.isEmpty {
                Divider()
                accounts
            }
            Divider()
            footer
        }
        .padding(16)
        .frame(width: 340)
        .onAppear { if state.analytics.messageCount == 0 { state.refresh() } }
    }

    private var header: some View {
        HStack(spacing: 10) {
            Image(systemName: "gauge.with.dots.needle.67percent")
                .font(.title2)
                .foregroundStyle(.tint)
            VStack(alignment: .leading, spacing: 1) {
                Text("VibeProxy").font(.headline)
                Text(state.activeProfile?.label ?? "Default (~/.claude)")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Spacer()
            if state.loading {
                ProgressView().controlSize(.small)
            } else {
                Button {
                    state.refresh()
                } label: {
                    Image(systemName: "arrow.clockwise")
                }
                .buttonStyle(.borderless)
                .help("Refresh")
            }
        }
    }

    private var rangePicker: some View {
        Picker("Range", selection: Binding(get: { state.range }, set: { state.setRange($0) })) {
            ForEach(RangePreset.allCases) { p in Text(p.rawValue).tag(p) }
        }
        .pickerStyle(.segmented)
        .labelsHidden()
    }

    private var summary: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .firstTextBaseline) {
                Text(Fmt.usd(state.analytics.totalValue))
                    .font(.system(size: 30, weight: .semibold, design: .rounded))
                    .contentTransition(.numericText())
                Spacer()
                VStack(alignment: .trailing, spacing: 1) {
                    Text("\(Fmt.count(state.analytics.messageCount)) msgs")
                    Text("\(Fmt.tokens(state.analytics.totals.total)) tokens")
                }
                .font(.caption)
                .foregroundStyle(.secondary)
            }
            if let e = state.error {
                Label(e, systemImage: "exclamationmark.triangle")
                    .font(.caption)
                    .foregroundStyle(.orange)
                    .lineLimit(2)
            }
            TokenBar(tokens: state.analytics.totals)
        }
    }

    private var accounts: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("ACCOUNTS")
                .font(.caption2.weight(.semibold))
                .foregroundStyle(.secondary)
            ForEach(state.profiles) { p in
                Button {
                    state.switchTo(p)
                } label: {
                    HStack(spacing: 8) {
                        Image(systemName: p.id == state.activeId ? "largecircle.fill.circle" : "circle")
                            .foregroundStyle(p.id == state.activeId ? AnyShapeStyle(.tint) : AnyShapeStyle(.secondary))
                        VStack(alignment: .leading, spacing: 0) {
                            Text(p.label)
                            if let email = p.email {
                                Text(email).font(.caption2).foregroundStyle(.secondary)
                            }
                        }
                        Spacer()
                        if let sub = p.subscriptionType {
                            Text(sub).font(.caption2).foregroundStyle(.secondary)
                        }
                    }
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
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

/// A single stacked bar of the four token classes — the panel's at-a-glance token mix.
struct TokenBar: View {
    let tokens: Tokens

    private var segments: [(String, UInt64, Color)] {
        [
            ("Input", tokens.input, .blue),
            ("Output", tokens.output, .green),
            ("Cache write", tokens.cacheWrite, .orange),
            ("Cache read", tokens.cacheRead, .purple),
        ]
    }

    var body: some View {
        let total = max(tokens.total, 1)
        VStack(alignment: .leading, spacing: 6) {
            GeometryReader { geo in
                HStack(spacing: 1) {
                    ForEach(segments, id: \.0) { seg in
                        seg.2.frame(width: max(0, geo.size.width * CGFloat(seg.1) / CGFloat(total)))
                    }
                }
                .clipShape(RoundedRectangle(cornerRadius: 3))
            }
            .frame(height: 8)
            HStack(spacing: 10) {
                ForEach(segments, id: \.0) { seg in
                    HStack(spacing: 4) {
                        Circle().fill(seg.2).frame(width: 6, height: 6)
                        Text(seg.0).font(.caption2).foregroundStyle(.secondary)
                    }
                }
            }
        }
    }
}
