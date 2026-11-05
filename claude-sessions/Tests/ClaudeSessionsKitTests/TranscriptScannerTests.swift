import Foundation
import Testing
@testable import ClaudeSessionsKit

private func scan(_ scanner: TranscriptScanner, _ url: URL, id: String = "a") -> ScanResult {
    let size = (try! FileManager.default.attributesOfItem(atPath: url.path)[.size] as! NSNumber).uint64Value
    scanner.update([ScanTarget(sessionId: id, transcript: url, size: size)], at: Date())
    return scanner.takeSnapshot()[id] ?? ScanResult()
}

private func transcript(_ lines: [String]) -> URL {
    writeTranscript(makeTempDir(), cwd: frontend, id: "a", lines: lines + [""])
}

private func ref(_ repo: String, _ id: Int) -> PRRef { PRRef(workspace: "t", repo: repo, id: id) }
private func link(_ repo: String, _ id: Int) -> String { "https://bitbucket.org/t/\(repo)/pull-requests/\(id)" }

@Suite struct TranscriptScannerTests {
    @Test func prefilterSkipsNonTextLines() {
        let url = transcript([
            toolUseLine(id: "1", name: "Bash", input: [:]), toolResultLine(id: "1"),
            jsonLine(["type": "assistant", "message": ["content": [["type": "thinking", "thinking": "x"]]]]),
            assistantLine("side", sidechain: true), userLine("meta", isMeta: true), aiTitleLine("t"),
            userLine("one"), assistantLine("two")])
        let s = TranscriptScanner(queue: nil)
        _ = scan(s, url)
        #expect(s.parsedLines == 2)
    }

    @Test func incrementalReadsOnlyAppended() {
        let url = transcript([userLine("T6IR-1")])
        let s = TranscriptScanner(queue: nil)
        #expect(scan(s, url).tickets.keys.sorted() == ["T6IR-1"])
        append(url, userLine("T6IR-2") + "\n")
        let r = scan(s, url)
        #expect(r.tickets.keys.sorted() == ["T6IR-1", "T6IR-2"])
        #expect(s.parsedLines == 2)
    }

    @Test func partialLineWaitsForNewline() {
        let url = transcript([userLine("T6IR-1")])
        append(url, userLine("T6IR-2"))
        let s = TranscriptScanner(queue: nil)
        #expect(scan(s, url).tickets.keys.sorted() == ["T6IR-1"])
        #expect(s.parsedLines == 1)
        append(url, "\n")
        #expect(scan(s, url).tickets.keys.sorted() == ["T6IR-1", "T6IR-2"])
    }

    @Test func oversizedLineSkipped() {
        let big = userLine(String(repeating: "x", count: 5 << 20))
        let url = transcript([userLine("T6IR-1"), big, userLine("T6IR-2")])
        let s = TranscriptScanner(queue: nil)
        #expect(scan(s, url).tickets.keys.sorted() == ["T6IR-1", "T6IR-2"])
    }

    @Test func mentionsBySource() {
        let summary = jsonLine(["type": "user", "isCompactSummary": true, "message": ["content": "summary T6IR-3"]])
        let url = transcript([userLine("T6IR-1"), assistantLine("T6IR-2 T6IR-1"), summary])
        let t = scan(TranscriptScanner(queue: nil), url).tickets
        #expect(t["T6IR-1"]?.byUser == true)
        #expect(t["T6IR-2"]?.byUser == false && t["T6IR-3"]?.byUser == false)
        #expect(t["T6IR-1"]!.first < t["T6IR-2"]!.first)
        #expect(t["T6IR-2"]!.first < t["T6IR-3"]!.first)
        #expect(t["T6IR-1"]!.last > t["T6IR-2"]!.first && t["T6IR-1"]!.last < t["T6IR-3"]!.first)
    }

    @Test func learnsJiraHosts() {
        let url = transcript([userLine("https://talosix.atlassian.net/browse/T6IR-1"),
                              assistantLine("https://talosix.atlassian.net/jira/boards/1?selectedIssue=OPS-2")])
        #expect(scan(TranscriptScanner(queue: nil), url).jiraHosts == ["T6IR": "talosix.atlassian.net", "OPS": "talosix.atlassian.net"])
    }

    @Test func jiraHostLowercasedAndAtlassianWins() {
        let url = transcript([userLine("https://Evil.Example/browse/T6IR-1"),
                              userLine("https://TaloSix.Atlassian.net/browse/T6IR-2"),
                              userLine("https://Other.Example/browse/OPS-1")])
        #expect(scan(TranscriptScanner(queue: nil), url).jiraHosts == ["T6IR": "talosix.atlassian.net", "OPS": "other.example"])
    }

    @Test func claudeTicketsOnlyInTurnsYouStarted() {
        let url = transcript([
            userLine("start"), assistantLine("T6IR-1"),
            notificationLine("agent report T6IR-2"), assistantLine("master has commits of T6IR-3"),
            userLine("ok", origin: "human"), assistantLine("T6IR-4"),
            notificationLine("T6IR-5", origin: false), assistantLine("T6IR-6")])
        #expect(scan(TranscriptScanner(queue: nil), url).tickets.keys.sorted() == ["T6IR-1", "T6IR-4"])
    }

    @Test func notificationTurnKeepsPRLinks() {
        let url = transcript([userLine("start"), notificationLine("done"), assistantLine("PR \(link("a", 9)) for T6IR-7")])
        let r = scan(TranscriptScanner(queue: nil), url)
        #expect(r.tickets.isEmpty)
        #expect(r.prRefs == [ref("a", 9)])
    }

    @Test func shellOutputAndRemindersIgnored() {
        let url = transcript([userLine("<bash-stdout>T6IR-5</bash-stdout>"),
                              assistantLine("ok <system-reminder>T6IR-6</system-reminder>")])
        #expect(scan(TranscriptScanner(queue: nil), url).tickets.isEmpty)
    }

    @Test func prLinksAndRefs() {
        let lines = [
            userLine("plain \(link("a", 1))"),
            toolResultTextLine("listing \(link("b", 2))"),
            userLine("meta \(link("c", 3))", isMeta: true),
            assistantLine("side \(link("d", 4))", sidechain: true),
            assistantLine("main \(link("e", 5)) https://github.com/x/y/pull/1"),
            userLine("<bash-stdout>\(link("f", 6))</bash-stdout>"),
            userLine("  <local-command-stdout>\(link("g", 7))</local-command-stdout>"),
            userLine("real \(link("h", 8))"), ""]
        let r = scan(TranscriptScanner(queue: nil), transcript(lines))
        #expect(r.prRefs == [ref("a", 1), ref("e", 5), ref("h", 8)])
        #expect(r.prLinks.map(\.absoluteString) == [link("a", 1), link("e", 5), "https://github.com/x/y/pull/1", link("h", 8)])
    }

    @Test func prLinksDedupedByIdentity() {
        let lines = [
            userLine("\(link("a", 12)) then \(link("a", 12))/diff and \(link("a", 12))/"),
            userLine("https://github.com/x/y/pull/1 https://github.com/x/y/pull/1/files \(link("a", 1))")]
        let r = scan(TranscriptScanner(queue: nil), transcript(lines))
        #expect(r.prLinks.map(\.absoluteString) == [link("a", 12), "https://github.com/x/y/pull/1", link("a", 1)])
    }

    @Test func sessionPublishedBeforeLargerOneFinishes() throws {
        let dir = makeTempDir()
        let small = writeTranscript(dir, cwd: frontend, id: "small", lines: [userLine("T6IR-1"), ""])
        let filler = userLine(String(repeating: "word ", count: 200))
        let big = writeTranscript(dir, cwd: frontend, id: "big", lines: Array(repeating: filler, count: 40_000) + [""])
        func size(_ u: URL) -> UInt64 { (try! FileManager.default.attributesOfItem(atPath: u.path)[.size] as! NSNumber).uint64Value }
        let q = DispatchQueue(label: "test-scanner")
        let s = TranscriptScanner(queue: q)
        let published = DispatchSemaphore(value: 0)
        let release = DispatchSemaphore(value: 0)
        s.onPublish = { id in
            guard id == "small" else { return }
            published.signal()
            release.wait()
        }
        s.update([ScanTarget(sessionId: "small", transcript: small, size: size(small)),
                  ScanTarget(sessionId: "big", transcript: big, size: size(big))], at: Date())
        let gotPublished = published.wait(timeout: .now() + 10)
        let early = s.takeSnapshot()
        release.signal()
        q.sync {}
        try #require(gotPublished == .success)
        #expect(early["small"]?.tickets.keys.sorted() == ["T6IR-1"])
        #expect(early["big"] == nil)
    }

    @Test func rewrittenFileRescanned() {
        let url = transcript([userLine("T6IR-1 " + String(repeating: "x", count: 200))])
        let s = TranscriptScanner(queue: nil)
        #expect(scan(s, url).tickets.keys.sorted() == ["T6IR-1"])
        try! Data((userLine("T6IR-2") + "\n").utf8).write(to: url)
        #expect(scan(s, url).tickets.keys.sorted() == ["T6IR-2"])
    }

    @Test func largeFileScanTime() {
        let url = makeTempDir().appendingPathComponent("big.jsonl")
        let result = toolResultTextLine(String(repeating: "r", count: 2000)) + "\n"
        var data = Data()
        while data.count < 32 << 20 { data.append(Data(result.utf8)) }
        for i in 0..<100 { data.append(Data((userLine("T6IR-\(i + 1)") + "\n").utf8)) }
        try! data.write(to: url)
        let s = TranscriptScanner(queue: nil)
        let start = Date()
        let r = scan(s, url)
        #expect(Date().timeIntervalSince(start) < 5)
        #expect(s.parsedLines == 100)
        #expect(r.tickets.count == 100)
    }
}
