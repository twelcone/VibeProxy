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

    private static func trim(_ v: Double) -> String {
        let s = String(format: "%.1f", v)
        return s.hasSuffix(".0") ? String(s.dropLast(2)) : s
    }
}
