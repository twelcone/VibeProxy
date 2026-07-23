// Thin typed layer over the uniffi bindings: JSON strings in, decoded models out. Everything the app
// touches from the Rust core funnels through here, so the FFI surface stays in one place.

import Foundation

enum Core {
    private static let decoder = JSONDecoder()

    static var version: String { coreVersion() }

    static func profiles() throws -> [Profile] {
        try decode([Profile].self, from: listProfilesJson())
    }

    static func activeProfile() -> String? { activeProfileId() }

    /// Adopt the default ~/.claude login as "Main" on first run, so there's always an account.
    static func bootstrap() throws { try bootstrapDefaultProfile() }

    /// Live 5-hour / weekly quota for every configured account.
    static func usageAll() throws -> [ProfileUsage] {
        try decode([ProfileUsage].self, from: usageAllJson())
    }

    // MARK: Add / remove accounts

    /// Create an isolated config dir and open a Terminal running `claude auth login` for it.
    /// Returns the pending config dir to poll.
    static func beginAdd() throws -> String { try beginAddProfile() }

    /// Poll whether the login into `configDir` completed.
    static func checkLogin(_ configDir: String) throws -> AuthStatus {
        try decode(AuthStatus.self, from: checkLoginJson(configDir: configDir))
    }

    /// Register a logged-in config dir as a new account.
    static func adopt(label: String, dir: String) throws {
        try adoptProfile(label: label, configDir: dir)
    }

    /// Abandon an in-progress add (removes the not-yet-registered dir).
    static func cancelAdd(_ configDir: String) throws { try cancelAddProfile(configDir: configDir) }

    /// Remove an account from VibeProxy (its Claude login is left untouched).
    static func remove(_ id: String) throws { try removeProfile(id: id) }

    /// Open a Terminal running `claude` on the active account, so a switch takes effect now.
    static func openClaude() throws { try relaunchClaude() }

    /// Clean up dirs left by an add that was abandoned mid-login.
    static func sweepOrphans() { gcOrphans() }

    /// Whether switches reach the user's own new terminals (shell integration installed).
    static func isShellInstalled() -> Bool { shellInstalled() }

    /// Install the shell integration; returns the file written.
    static func setUpShell() throws -> String { try installShell() }

    /// `range` is "FROM..TO" (either side may be empty), or nil for all time.
    static func usage(range: String?) throws -> Analytics {
        try decode(Analytics.self, from: usageJson(range: range))
    }

    /// Make a profile active by id or label. Throws FfiError.Message on an unknown target.
    static func activate(_ target: String) throws {
        try switchProfile(target: target)
    }

    static var shell: String { shellSnippet() }

    private static func decode<T: Decodable>(_ type: T.Type, from json: String) throws -> T {
        try decoder.decode(type, from: Data(json.utf8))
    }
}

// MARK: - Formatting (mirrors the web UI's src/lib/format.ts)

enum Fmt {
    /// Compact token count: 1.8M, 3.2B. Whole numbers below 1000 print exactly.
    static func tokens(_ n: UInt64) -> String {
        let v = Double(n)
        switch v {
        case 1e12...: return trim(v / 1e12) + "T"
        case 1e9...: return trim(v / 1e9) + "B"
        case 1e6...: return trim(v / 1e6) + "M"
        case 1e3...: return trim(v / 1e3) + "K"
        default: return String(n)
        }
    }

    static func fullTokens(_ n: UInt64) -> String {
        let f = NumberFormatter()
        f.numberStyle = .decimal
        f.locale = Locale(identifier: "en_US")
        return f.string(from: NSNumber(value: n)) ?? String(n)
    }

    /// USD, nil (unpriced) rendered as an em dash. Forced to en_US so grouping is an unambiguous
    /// comma ("$6,831") regardless of the host locale.
    static func usd(_ v: Double?) -> String {
        guard let v else { return "—" }
        let f = NumberFormatter()
        f.numberStyle = .currency
        f.currencyCode = "USD"
        f.locale = Locale(identifier: "en_US")
        f.maximumFractionDigits = v >= 100 ? 0 : 2
        return f.string(from: NSNumber(value: v)) ?? "$\(v)"
    }

    static func count(_ n: UInt64) -> String { fullTokens(n) }

    /// Quota percent as a whole number: "33%", or "—" when unavailable.
    static func pct(_ v: Double?) -> String {
        guard let v else { return "—" }
        return "\(Int(v.rounded()))%"
    }

    /// A short "resets in 2h 14m" from an ISO-8601 timestamp, or "" if it can't be parsed / is past.
    static func resets(_ iso: String?, now: Date = Date()) -> String {
        guard let iso, let date = isoDate(iso) else { return "" }
        let secs = Int(date.timeIntervalSince(now))
        guard secs > 0 else { return "resets now" }
        let h = secs / 3600
        let m = (secs % 3600) / 60
        if h >= 24 { return "resets in \(h / 24)d \(h % 24)h" }
        if h > 0 { return "resets in \(h)h \(m)m" }
        return "resets in \(m)m"
    }

    private static func isoDate(_ s: String) -> Date? {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return f.date(from: s) ?? {
            let g = ISO8601DateFormatter()
            g.formatOptions = [.withInternetDateTime]
            return g.date(from: s)
        }()
    }

    private static func trim(_ v: Double) -> String {
        let s = String(format: "%.1f", v)
        return s.hasSuffix(".0") ? String(s.dropLast(2)) : s
    }
}
