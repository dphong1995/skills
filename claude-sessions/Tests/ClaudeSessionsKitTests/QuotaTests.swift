import Foundation
import Testing
@testable import ClaudeSessionsKit

private func writeCache(_ url: URL, five: Double = 30, fiveReset: String = "2026-11-06T11:00:00.171Z", weekly: Double = 5, error: Bool = false, last: Double = 1793961364363) {
    let o: [String: Any] = ["timestamp": last, "data": ["fiveHourPercent": five, "fiveHourResetsAt": fiveReset, "weeklyPercent": weekly,
                            "weeklyResetsAt": "2026-11-13T06:00:00.171Z", "scopedWeeklyBuckets": []],
                            "error": error, "source": "anthropic", "lastSuccessAt": last]
    try! JSONSerialization.data(withJSONObject: o).write(to: url)
}

private func cacheURL() -> URL { makeTempDir().appendingPathComponent("usage.json") }

@Test func parsesRealFile() {
    let url = cacheURL()
    writeCache(url)
    let q = QuotaReader(url: url).read(at: Date(timeIntervalSince1970: 1793961364))
    #expect(q?.fiveHour == 30)
    #expect(q?.weekly == 5)
    #expect(q?.fiveHourReset == Date(timeIntervalSince1970: 1793962800.171))
    #expect(q?.weeklyReset == Date(timeIntervalSince1970: 1794549600.171))
}

@Test func fractionalPercentIsRounded() {
    let url = cacheURL()
    writeCache(url, five: 37.5, weekly: 4.2)
    let q = QuotaReader(url: url).read(at: Date(timeIntervalSince1970: 1793961364))
    #expect(q?.fiveHour == 38)
    #expect(q?.weekly == 4)
}

@Test func missingAndStale() {
    let now = Date(timeIntervalSince1970: 1793961364)
    #expect(QuotaReader(url: cacheURL()).read(at: now) == nil)
    let bad = cacheURL()
    writeCache(bad, error: true)
    #expect(QuotaReader(url: bad).read(at: now) == nil)
    let url = cacheURL()
    writeCache(url)
    let later = now.addingTimeInterval(16 * 60)
    let q = QuotaReader(url: url).read(at: later)
    #expect(q != nil)
}

@Test func forecastLinear() {
    let t0 = Date(timeIntervalSince1970: 1_000_000)
    let t1 = t0.addingTimeInterval(20 * 60)
    let f = QuotaReader.forecast(samples: [(t0, 20), (t1, 40)], now: t1, reset: t0.addingTimeInterval(3 * 3600))
    #expect(f == t0.addingTimeInterval(80 * 60))
}

@Test func forecastNeedsTwoSamples() {
    let t0 = Date(timeIntervalSince1970: 1_000_000)
    let reset = t0.addingTimeInterval(3 * 3600)
    #expect(QuotaReader.forecast(samples: [(t0, 20)], now: t0, reset: reset) == nil)
    let t1 = t0.addingTimeInterval(4 * 60)
    #expect(QuotaReader.forecast(samples: [(t0, 20), (t1, 40)], now: t1, reset: reset) == nil)
}

@Test func forecastNotBeforeReset() {
    let t0 = Date(timeIntervalSince1970: 1_000_000)
    let t1 = t0.addingTimeInterval(20 * 60)
    #expect(QuotaReader.forecast(samples: [(t0, 20), (t1, 40)], now: t1, reset: t0.addingTimeInterval(60 * 60)) == nil)
}

@Test func windowChangeClearsSamples() {
    let url = cacheURL()
    let t0 = 1793961364363.0
    let reader = QuotaReader(url: url)
    writeCache(url, five: 20, fiveReset: "2026-11-06T14:00:00.171Z", last: t0)
    _ = reader.read(at: Date(timeIntervalSince1970: t0 / 1000))
    writeCache(url, five: 40, fiveReset: "2026-11-06T14:00:00.171Z", last: t0 + 20 * 60 * 1000)
    let now = Date(timeIntervalSince1970: t0 / 1000 + 20 * 60 + 11)
    #expect(reader.read(at: now)?.forecast != nil)
    writeCache(url, five: 50, fiveReset: "2026-11-06T19:00:00.171Z", last: t0 + 21 * 60 * 1000)
    #expect(reader.read(at: now.addingTimeInterval(11))?.forecast == nil)
}

@Test func levels() {
    #expect(QuotaLevel.level(79) == .normal)
    #expect(QuotaLevel.level(80) == .warn)
    #expect(QuotaLevel.level(94) == .warn)
    #expect(QuotaLevel.level(95) == .high)
}
