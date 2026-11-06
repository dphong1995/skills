import Foundation

public struct Quota: Equatable {
    public var fiveHour: Int
    public var fiveHourReset: Date
    public var weekly: Int
    public var weeklyReset: Date
    public var updated: Date
    public var forecast: Date?

    /// The cache is stale once its last success is over 15 minutes old.
    public func isStale(now: Date) -> Bool { now.timeIntervalSince(updated) > 900 }

    /// The percent shown on the collapsed pill: the larger of the two, only from 80.
    public var pillPercent: Int? {
        let m = max(fiveHour, weekly)
        return m >= 80 ? m : nil
    }
}

public enum QuotaLevel {
    case normal, warn, high

    public static func level(_ percent: Int) -> QuotaLevel {
        percent >= 95 ? .high : percent >= 80 ? .warn : .normal
    }
}

/// Reads the OMC HUD usage cache; re-reads only when the file changed and at most every 10 s.
public final class QuotaReader {
    private let url: URL
    private var stamp: FileStamp?
    private var lastRead: Date?
    private var cached: Quota?
    private var samples: [(Date, Int)] = []
    private var window: Date?

    public init(url: URL) { self.url = url }

    public func read(at now: Date) -> Quota? {
        if let lastRead, now.timeIntervalSince(lastRead) < 10 { return cached }
        let st = FileStamp(url.path)
        if st != nil, st == stamp { lastRead = now; return cached }
        lastRead = now
        stamp = st
        cached = parse(now: now)
        return cached
    }

    private func parse(now: Date) -> Quota? {
        guard let data = try? Data(contentsOf: url),
              let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              o["error"] as? Bool != true,
              let d = o["data"] as? [String: Any],
              let five = (d["fiveHourPercent"] as? NSNumber).map({ Int($0.doubleValue.rounded()) }),
              let weekly = (d["weeklyPercent"] as? NSNumber).map({ Int($0.doubleValue.rounded()) }),
              let fiveReset = (d["fiveHourResetsAt"] as? String).flatMap(ISODate.date),
              let weeklyReset = (d["weeklyResetsAt"] as? String).flatMap(ISODate.date),
              let last = o["lastSuccessAt"] as? Double
        else { return nil }
        let updated = Date(timeIntervalSince1970: last / 1000)
        if window != fiveReset { window = fiveReset; samples = [] }
        if samples.last?.0 != updated {
            samples.append((updated, five))
            if samples.count > 60 { samples.removeFirst(samples.count - 60) }
        }
        return Quota(fiveHour: five, fiveHourReset: fiveReset, weekly: weekly, weeklyReset: weeklyReset, updated: updated,
                     forecast: Self.forecast(samples: samples, now: now, reset: fiveReset))
    }

    /// When the 5h percent reaches 100 at the pace of the last 30 minutes; nil without two samples 5+ minutes apart, a rising pace, or a hit after `reset`.
    public static func forecast(samples: [(Date, Int)], now: Date, reset: Date) -> Date? {
        let recent = samples.filter { now.timeIntervalSince($0.0) <= 1800 }
        guard let first = recent.first, let last = recent.last, last.0.timeIntervalSince(first.0) >= 300 else { return nil }
        let rate = Double(last.1 - first.1) / last.0.timeIntervalSince(first.0)
        guard rate > 0 else { return nil }
        let hit = now.addingTimeInterval(Double(100 - last.1) / rate)
        return hit < reset ? hit : nil
    }
}
