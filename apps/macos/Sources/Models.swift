// Codable models mirroring the JSON the FFI emits (the same shapes as `vibeproxy … --json`).
// JSON keys are camelCase (serde rename_all), so property names match keys directly — no CodingKeys.

import Foundation

struct Tokens: Codable, Equatable {
    var input: UInt64 = 0
    var output: UInt64 = 0
    var cacheWrite: UInt64 = 0
    var cacheRead: UInt64 = 0

    var total: UInt64 { input + output + cacheWrite + cacheRead }
}

struct AccountRow: Codable, Identifiable {
    let account: String
    let tokens: Tokens
    let messages: UInt64
    let value: Double?
    var id: String { account }
}

struct ModelRow: Codable, Identifiable {
    let model: String
    let tokens: Tokens
    let messages: UInt64
    let value: Double?
    var id: String { model }
}

struct DayRow: Codable, Identifiable {
    let date: String
    let tokens: Tokens
    let value: Double?
    var id: String { date }
}

struct ProjectRow: Codable, Identifiable {
    let project: String
    let tokens: Tokens
    let value: Double?
    var id: String { project }
}

struct ModelDayRow: Codable {
    let date: String
    let model: String
    let tokens: Tokens
    let value: Double?
}

struct AccountDayRow: Codable {
    let date: String
    let account: String
    let tokens: Tokens
    let value: Double?
}

struct DateRange: Codable, Equatable {
    var from: String?
    var to: String?
}

struct Analytics: Codable {
    let totals: Tokens
    let messageCount: UInt64
    let perAccount: [AccountRow]
    let perModel: [ModelRow]
    let perDay: [DayRow]
    let perProject: [ProjectRow]
    let perModelPerDay: [ModelDayRow]
    let perAccountPerDay: [AccountDayRow]
    let range: DateRange?
    let totalValue: Double
    let pricedAsOf: String
    let unpricedModels: [String]

    /// A zeroed analytics, so views can render before the first scan completes.
    static let empty = Analytics(
        totals: Tokens(), messageCount: 0, perAccount: [], perModel: [], perDay: [],
        perProject: [], perModelPerDay: [], perAccountPerDay: [], range: nil,
        totalValue: 0, pricedAsOf: "", unpricedModels: []
    )
}

/// Freshness/health of a profile's live usage reading.
enum UsageStatus: String, Codable {
    case ok
    case needsReauth
    case error
}

/// A profile's live quota reading — the 5-hour and weekly limits Claude Code Pro/Max enforce.
/// This is the product's headline number, not historical token cost.
struct ProfileUsage: Codable, Identifiable {
    let profileId: String
    let fiveHourPct: Double?
    let fiveHourResetsAt: String?
    let weeklyPct: Double?
    let weeklyResetsAt: String?
    let status: UsageStatus
    let error: String?
    var id: String { profileId }
}

/// Result of polling whether a pending login has completed (mirrors the core's AuthStatus).
struct AuthStatus: Codable {
    let loggedIn: Bool
    let email: String?
    let orgId: String?
    let subscriptionType: String?
}

struct Profile: Codable, Identifiable {
    let id: String
    let label: String
    let configDir: String
    let email: String?
    let orgId: String?
    let subscriptionType: String?
    let priority: Int
    let createdAt: String
}
