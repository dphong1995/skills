import Foundation
import Testing
@testable import ClaudeSessionsKit

/// A `feed.list` response captured from a live cmux (haiku session asking `Pick a color`), trimmed to what differs per run.
private let capturedList = """
{"id":"37705c5d","ok":true,"result":{"items":[{"kind":"question","id":"6103AC13","question_multi_select":false,"question_options":[{"id":"opt0","description":"Chọn màu đỏ","label":"Red"}],"cwd":"/tmp/spike-a","question_prompt":"Pick a color","title":"AskUserQuestion","questions":[{"multi_select":false,"id":"q0","header":"Color","options":[{"description":"Chọn màu đỏ","label":"Red","id":"opt0"},{"id":"opt1","description":"Chọn màu xanh dương","label":"Blue"},{"description":"Chọn màu xanh lá","id":"opt2","label":"Green"}],"prompt":"Pick a color"}],"source":"claude","updated_at":"2026-11-06T12:02:56Z","created_at":"2026-11-06T12:02:56Z","workstream_id":"cmux-feed-v1:Y2xhdWRl:ODE0ODFlNDUtMGRkNS00MWM1LTlkODYtMGUyZjYwNmZkNGFk","request_id":"E329BEF6-5CC6-4EF4-BBE2-002B8D2460DD","status":"pending"},{"kind":"permissionRequest","request_id":"P1","status":"pending","workstream_id":"cmux-feed-v1:Y2xhdWRl:eA=="},{"kind":"question","request_id":"OLD","status":"resolved","workstream_id":"cmux-feed-v1:Y2xhdWRl:eA==","questions":[]}]}}
"""

@Test func parseRealItem() {
    let list = Feed.parseList(Data(capturedList.utf8))
    #expect(list.count == 1)
    let q = list[0]
    #expect(q.requestId == "E329BEF6-5CC6-4EF4-BBE2-002B8D2460DD")
    #expect(q.sessionId == "81481e45-0dd5-41c5-9d86-0e2f606fd4ad")
    #expect(q.createdAt == ISO8601DateFormatter().date(from: "2026-11-06T12:02:56Z"))
    #expect(q.questions.count == 1)
    #expect(q.questions[0].prompt == "Pick a color")
    #expect(q.questions[0].header == "Color")
    #expect(!q.questions[0].multiSelect)
    #expect(q.questions[0].options.map(\.label) == ["Red", "Blue", "Green"])
    #expect(q.questions[0].options[1].id == "opt1")
    #expect(q.questions[0].options[1].description == "Chọn màu xanh dương")
}

@Test func parseSkipsOtherSourcesAndMissingDate() {
    var other = feedItem(requestId: "X1")
    other["source"] = "codex"
    var unsourced = feedItem(requestId: "X3")
    unsourced["source"] = nil
    var undated = feedItem(requestId: "X2")
    undated["created_at"] = nil
    let data = try! JSONSerialization.data(withJSONObject: ["ok": true, "result": ["items": [other, unsourced, undated, feedItem(requestId: "OK")]]])
    #expect(Feed.parseList(data).map(\.requestId) == ["OK"])
}

@Test func parseGarbageIsEmpty() {
    #expect(Feed.parseList(Data("not json".utf8)).isEmpty)
    #expect(Feed.parseList(Data("{\"ok\":false,\"error\":{\"code\":\"x\"}}".utf8)).isEmpty)
    #expect(Feed.parseList(Data("{\"ok\":true,\"result\":{}}".utf8)).isEmpty)
}

private func selections(_ p: [String: Any]) -> [String]? { p["selections"] as? [String] }

@Test func replyParamsShapes() {
    // (a) one question, single select
    var p = Feed.replyParams(requestId: "R", answers: [["Blue"]], other: [:])
    #expect(p["request_id"] as? String == "R")
    #expect(selections(p) == ["Blue"])
    // (b) multi-select: the labels joined by ", " (one string per question)
    p = Feed.replyParams(requestId: "R", answers: [["Red", "Blue"]], other: [:])
    #expect(selections(p) == ["Red, Blue"])
    // (c) several questions: one string each, in order
    p = Feed.replyParams(requestId: "R", answers: [["Red"], ["Large"]], other: [:])
    #expect(selections(p) == ["Red", "Large"])
    // (d) Other free text, trimmed
    p = Feed.replyParams(requestId: "R", answers: [[]], other: [0: "  Purple "])
    #expect(selections(p) == ["Purple"])
    // Other after the picks; an unanswered question sends "" (callers only send complete answers)
    p = Feed.replyParams(requestId: "R", answers: [["Red"], []], other: [0: "x"])
    #expect(selections(p) == ["Red, x", ""])
    // With the question known, the slot count is its question count.
    p = Feed.replyParams(requestId: "R", question: feedQuestion(questions: feedQuestion().questions + feedQuestion().questions), answers: [["Red"]], other: [:])
    #expect(selections(p) == ["Red", ""])
    // An unanswered question in the middle keeps the later answers on their own prompts.
    p = Feed.replyParams(requestId: "R", answers: [[], ["Large"]], other: [:])
    #expect(selections(p) == ["", "Large"])
}

@Test func supportsAndComplete() {
    #expect(Feed.supports(feedQuestion()))
    #expect(!Feed.supports(feedQuestion(questions: [])))
    #expect(!Feed.supports(feedQuestion(questions: [FeedQuestion.Item(prompt: "P", header: nil, multiSelect: false, options: [])])))
    let two = feedQuestion(questions: [FeedQuestion.Item(prompt: "A", header: nil, multiSelect: false, options: [.init(id: "1", label: "x", description: nil)]),
                                       FeedQuestion.Item(prompt: "B", header: nil, multiSelect: false, options: [.init(id: "1", label: "y", description: nil)])])
    #expect(!Feed.complete(two, answers: [["x"], []], other: [:]))
    #expect(!Feed.complete(two, answers: [["x"], []], other: [1: "   "]))
    #expect(Feed.complete(two, answers: [["x"], []], other: [1: "z"]))
    #expect(Feed.complete(two, answers: [["x"], ["y"]], other: [:]))
    #expect(!Feed.complete(two, answers: [], other: [:]))
}

@Test func windowAndCountdown() {
    let t0 = Date()
    let q = feedQuestion(created: t0)
    #expect(Feed.window == 120)
    #expect(Feed.countdown(q, now: t0) == "còn 2:00")
    #expect(Feed.countdown(q, now: t0.addingTimeInterval(61)) == "còn 0:59")
    #expect(Feed.countdown(q, now: t0.addingTimeInterval(200)) == "còn 0:00")
    #expect(Feed.blockState(q, live: true, sent: false, failed: false, now: t0.addingTimeInterval(119)) == .form)
    #expect(Feed.blockState(q, live: true, sent: false, failed: false, now: t0.addingTimeInterval(120)) == .fallback)
    #expect(Feed.blockState(q, live: false, sent: false, failed: false, now: t0) == .fallback)
    #expect(Feed.blockState(q, live: true, sent: false, failed: true, now: t0) == .fallback)
    #expect(Feed.blockState(q, live: true, sent: true, failed: false, now: t0) == .sent)
    #expect(Feed.blockState(q, live: false, sent: true, failed: false, now: t0.addingTimeInterval(500)) == .sent)
    let odd = feedQuestion(created: t0, questions: [])
    #expect(Feed.blockState(odd, live: true, sent: false, failed: false, now: t0) == .fallback)
}

@Test func monitorPollsOnlyWhenActive() {
    let t = FakeFeedTransport()
    t.items = [feedItem()]
    let m = FeedMonitor(transport: t, queue: nil)
    let now = Date()
    m.update(active: false, at: now)
    #expect(t.calls.isEmpty)
    #expect(m.takeSnapshot().isEmpty)
    m.update(active: true, at: now)
    #expect(t.calls.map(\.method) == ["feed.list"])
    #expect(t.calls[0].params["pending_only"] as? Bool == true)
    #expect(m.takeSnapshot().map(\.requestId) == ["R1"])
    // Going inactive empties the snapshot and stops polling.
    m.update(active: false, at: now.addingTimeInterval(1))
    #expect(t.calls.count == 1)
    #expect(m.takeSnapshot().isEmpty)
}

@Test func transportErrorDisables() {
    let t = FakeFeedTransport()
    t.failAll = true
    let m = FeedMonitor(transport: t, queue: nil)
    let now = Date()
    m.update(active: true, at: now)
    #expect(m.takeSnapshot().isEmpty)
    #expect(t.calls.count == 1)
    // Backs off instead of hammering a broken socket, then retries.
    m.update(active: true, at: now.addingTimeInterval(1))
    #expect(t.calls.count == 1)
    t.failAll = false
    t.items = [feedItem()]
    m.update(active: true, at: now.addingTimeInterval(Feed.retryAfter + 1))
    #expect(m.takeSnapshot().count == 1)
}

@Test func replyOnlyForListedRequestId() {
    let t = FakeFeedTransport()
    t.items = [feedItem(requestId: "R1")]
    let m = FeedMonitor(transport: t, queue: nil)
    m.update(active: true, at: Date())
    var ok: Bool?
    m.reply(feedQuestion(requestId: "R-unknown"), answers: [["Red"]], other: [:]) { ok = $0 }
    #expect(ok == false)
    #expect(t.calls.map(\.method) == ["feed.list"])
    m.reply(feedQuestion(requestId: "R1"), answers: [["Red"]], other: [:]) { ok = $0 }
    #expect(ok == true)
    let last = t.calls.last!
    #expect(last.method == "feed.question.reply")
    #expect(last.params["request_id"] as? String == "R1")
    #expect(last.params["selections"] as? [String] == ["Red"])
    // A delivered answer leaves the snapshot at once, so the next card build cannot show the form again.
    #expect(m.takeSnapshot().isEmpty)
    t.replyResult = "{\"ok\":false,\"error\":{\"code\":\"not_found\",\"message\":\"x\"}}"
    m.update(active: true, at: Date().addingTimeInterval(1))
    m.reply(feedQuestion(requestId: "R1"), answers: [["Red"]], other: [:]) { ok = $0 }
    #expect(ok == false)
    m.update(active: true, at: Date().addingTimeInterval(2))
    t.failAll = true
    m.reply(feedQuestion(requestId: "R1"), answers: [["Red"]], other: [:]) { ok = $0 }
    #expect(ok == false)
}

@Test func vanishedItemFallsBack() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    let t = FakeFeedTransport()
    t.items = [feedItem(requestId: "R1", sessionId: "a")]
    store.feedMonitor = FeedMonitor(transport: t, queue: nil)
    writeSession(dir, "a", pid: me, status: "waiting", waitingFor: "input needed", ageSec: 0, name: "a", cwd: frontend)
    let t0 = Date()
    store.refresh(at: t0)
    var s = store.sessions[0]
    #expect(s.question?.requestId == "R1" && s.questionLive)
    #expect(Feed.blockState(s.question!, live: s.questionLive, sent: false, failed: false, now: t0) == .form)
    // The item leaves feed.list (answered in the terminal, or the 120 s hook gave up) while the session still waits.
    t.items = []
    store.refresh(at: t0.addingTimeInterval(1))
    s = store.sessions[0]
    #expect(s.question?.requestId == "R1" && !s.questionLive)
    #expect(Feed.blockState(s.question!, live: s.questionLive, sent: false, failed: false, now: t0.addingTimeInterval(1)) == .fallback)
    // The session runs again: the block is gone.
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: t0.addingTimeInterval(2))
    #expect(store.sessions[0].question == nil)
}

@Test func socketRoundTrip() throws {
    let dir = URL(fileURLWithPath: "/tmp/fs-\(UUID().uuidString.prefix(8))", isDirectory: true)
    try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: dir) }
    let sock = dir.appendingPathComponent("s.sock").path
    let pathFile = dir.appendingPathComponent("last-socket-path")
    try (sock + "\nignored\n").write(to: pathFile, atomically: true, encoding: .utf8)
    let server = try TestUnixServer(path: sock)
    defer { server.close() }
    guard let transport = SocketFeedTransport(pathFile: pathFile) else { Issue.record("no transport"); return }
    server.answer = "{\"ok\":true,\"result\":{\"items\":[]}}\n"
    let data = try transport.request("feed.list", ["pending_only": true], timeout: 2)
    #expect(String(decoding: data, as: UTF8.self).contains("\"items\""))
    let received = String(decoding: server.lastRequest(), as: UTF8.self)
    #expect(received.hasSuffix("\n"))
    let o = try JSONSerialization.jsonObject(with: Data(received.utf8)) as? [String: Any]
    #expect(o?["method"] as? String == "feed.list")
    #expect((o?["params"] as? [String: Any])?["pending_only"] as? Bool == true)
    #expect(o?["id"] is String)
    // Silent server: times out.
    server.answer = nil
    let t0 = Date()
    #expect(throws: (any Error).self) { try transport.request("feed.list", [:], timeout: 0.4) }
    #expect(Date().timeIntervalSince(t0) < 2)
}

@Test func socketTransportNeedsPathFile() {
    #expect(SocketFeedTransport(pathFile: URL(fileURLWithPath: "/nonexistent/last-socket-path")) == nil)
}
