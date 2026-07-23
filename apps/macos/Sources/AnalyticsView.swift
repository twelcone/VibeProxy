// The full analytics window: a daily-spend trend chart, per-model and per-account breakdowns, and a
// sortable model table. Native Swift Charts on the same data the web dashboard renders.

import Charts
import SwiftUI

struct AnalyticsView: View {
    @EnvironmentObject var state: AppState

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                title
                statCards
                trendChart
                HStack(alignment: .top, spacing: 16) {
                    breakdown(title: "By model", rows: modelRows)
                    breakdown(title: "By account", rows: accountRows)
                }
                modelTable
                if !state.analytics.pricedAsOf.isEmpty {
                    Text("API-equivalent value, priced as of \(state.analytics.pricedAsOf). Not your subscription cost.")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
            }
            .padding(24)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .frame(minWidth: 720, minHeight: 560)
        .onAppear { state.refresh() }
    }

    private var title: some View {
        HStack {
            VStack(alignment: .leading) {
                Text("Usage Analytics").font(.largeTitle.bold())
                Text(state.activeProfile?.label ?? "Default (~/.claude)")
                    .foregroundStyle(.secondary)
            }
            Spacer()
            Picker("", selection: Binding(get: { state.range }, set: { state.setRange($0) })) {
                ForEach(RangePreset.allCases) { Text($0.label).tag($0) }
            }
            .pickerStyle(.segmented)
            .fixedSize()
        }
    }

    private var statCards: some View {
        HStack(spacing: 12) {
            StatCard(label: "API value", value: Fmt.usd(state.analytics.totalValue), tint: .green)
            StatCard(label: "Total tokens", value: Fmt.tokens(state.analytics.totals.total), tint: .blue)
            StatCard(label: "Messages", value: Fmt.count(state.analytics.messageCount), tint: .purple)
            StatCard(label: "Models", value: "\(state.analytics.perModel.count)", tint: .orange)
        }
    }

    // MARK: Trend

    private var trendChart: some View {
        GroupBox {
            if state.analytics.perDay.isEmpty {
                Text("No data in this range.")
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, minHeight: 220)
            } else {
                Chart(state.analytics.perDay) { day in
                    let d = parse(day.date)
                    AreaMark(x: .value("Date", d), y: .value("Tokens", day.tokens.total))
                        .foregroundStyle(
                            .linearGradient(colors: [.accentColor.opacity(0.4), .accentColor.opacity(0.02)],
                                            startPoint: .top, endPoint: .bottom))
                    LineMark(x: .value("Date", d), y: .value("Tokens", day.tokens.total))
                        .foregroundStyle(.tint)
                        .interpolationMethod(.monotone)
                }
                .chartYAxis {
                    AxisMarks { value in
                        AxisGridLine()
                        AxisValueLabel {
                            if let v = value.as(Double.self) { Text(Fmt.tokens(UInt64(max(0, v)))) }
                        }
                    }
                }
                .frame(height: 240)
            }
        } label: {
            Label("Daily tokens", systemImage: "chart.xyaxis.line")
        }
    }

    // MARK: Breakdowns

    private struct Row: Identifiable {
        let id: String
        let label: String
        let value: UInt64
        let secondary: String
    }

    private var modelRows: [Row] {
        state.analytics.perModel.map {
            Row(id: $0.model, label: $0.model, value: $0.tokens.total, secondary: Fmt.usd($0.value))
        }
    }

    private var accountRows: [Row] {
        state.analytics.perAccount.map {
            Row(id: $0.account, label: $0.account, value: $0.tokens.total, secondary: Fmt.usd($0.value))
        }
    }

    private func breakdown(title: String, rows: [Row]) -> some View {
        let maxVal = rows.map(\.value).max() ?? 1
        return GroupBox(title) {
            VStack(spacing: 6) {
                if rows.isEmpty {
                    Text("No data.").font(.caption).foregroundStyle(.secondary)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                ForEach(rows) { row in
                    RankBar(label: row.label, value: row.value, max: maxVal,
                            valueText: Fmt.tokens(row.value), secondary: row.secondary)
                }
            }
            .padding(.top, 2)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    // MARK: Table

    private var modelTable: some View {
        GroupBox {
            Table(state.analytics.perModel) {
                TableColumn("Model", value: \.model)
                TableColumn("Input") { Text(Fmt.tokens($0.tokens.input)) }
                TableColumn("Output") { Text(Fmt.tokens($0.tokens.output)) }
                TableColumn("Cache write") { Text(Fmt.tokens($0.tokens.cacheWrite)) }
                TableColumn("Cache read") { Text(Fmt.tokens($0.tokens.cacheRead)) }
                TableColumn("Total") { Text(Fmt.tokens($0.tokens.total)).bold() }
                TableColumn("API value") { Text(Fmt.usd($0.value)) }
            }
            .frame(minHeight: 180)
        } label: {
            Label("Per model", systemImage: "tablecells")
        }
    }

    private func parse(_ ymd: String) -> Date {
        let f = DateFormatter()
        f.dateFormat = "yyyy-MM-dd"
        return f.date(from: ymd) ?? Date(timeIntervalSince1970: 0)
    }
}

struct StatCard: View {
    let label: String
    let value: String
    let tint: Color

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(label.uppercased())
                .font(.caption2.weight(.semibold))
                .foregroundStyle(.secondary)
            Text(value)
                .font(.system(size: 22, weight: .semibold, design: .rounded))
                .foregroundStyle(tint)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(14)
        .background(.quaternary.opacity(0.5), in: RoundedRectangle(cornerRadius: 10))
    }
}

/// A ranked row whose background fill encodes its share of the largest value.
struct RankBar: View {
    let label: String
    let value: UInt64
    let max: UInt64
    let valueText: String
    let secondary: String

    var body: some View {
        let frac = max > 0 ? CGFloat(value) / CGFloat(max) : 0
        ZStack(alignment: .leading) {
            GeometryReader { geo in
                RoundedRectangle(cornerRadius: 5)
                    .fill(.tint.opacity(0.18))
                    .frame(width: value > 0 ? Swift.max(10, geo.size.width * frac) : 0)
            }
            HStack(spacing: 8) {
                Text(label).lineLimit(1).truncationMode(.middle)
                Spacer()
                Text(secondary).font(.caption).foregroundStyle(.secondary)
                Text(valueText).font(.callout.monospacedDigit().weight(.semibold))
            }
            .padding(.horizontal, 10)
        }
        .frame(height: 30)
        .background(.quaternary.opacity(0.4), in: RoundedRectangle(cornerRadius: 5))
    }
}
