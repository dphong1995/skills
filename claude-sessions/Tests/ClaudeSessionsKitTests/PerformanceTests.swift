import Foundation
import Testing
@testable import ClaudeSessionsKit

/// Builds sessions on temp dirs: transcripts, task files and real `.git/HEAD` files, read by a `Store` that uses the real branch reader.
private final class Fixture {
    let dir = makeTempDir()
    let projects = makeTempDir()
    let tasks = makeTempDir()
    let scanner = TranscriptScanner(queue: nil)
    let store: Store
    private(set) var ids: [String] = []
    private var created = 0

    init() {
        store = Store(dir: dir, projects: projects, tasks: tasks, startHidden: false, prs: nil, jira: nil, scanner: scanner, branch: nil)
    }

    /// One repeatable block of realistic transcript lines: a prompt with a Jira link, a reply, a tool call and its result, a PR link.
    static let block: String = [
        userLine("Please fix the failing test in the parser module. See https://acme.atlassian.net/browse/ABC-1 for the details."),
        assistantLine("I will look at the parser first and then run the tests. " + String(repeating: "Reading the code. ", count: 12)),
        toolUseLine(id: "toolu_01", name: "Bash", input: ["command": "swift test --filter ParserTests"]),
        toolResultLine(id: "toolu_01"),
        userLine("The PR is https://bitbucket.org/t6engineering/talo6-dct-backend/pull-requests/12 now."),
    ].joined(separator: "\n") + "\n"

    static func real(bytes: Int) -> String {
        String(repeating: block, count: max(1, bytes / block.utf8.count))
    }

    /// Lines without a `"type":"user"` or `"type":"assistant"` marker: the scanner skips them without a JSON parse.
    static let filler = (#"{"type":"progress","data":""# + String(repeating: "x", count: 900) + "\"}\n")
    static let fillerMB = String(repeating: filler, count: (1 << 20) / filler.utf8.count)

    /// Adds a session whose cwd is a temp dir with a git repo (`worktree`: a `.git` file pointing at a git dir elsewhere).
    @discardableResult
    func add(status: String, waitingFor: String? = nil, transcriptBytes: Int, fillerMB: Int = 0, worktree: Bool = false, pendingTool: Bool = false, taskFiles: Int = 5) -> String {
        let id = "perf-\(created)"
        created += 1
        let cwd = makeTempDir()
        let fm = FileManager.default
        if worktree {
            let gitdir = makeTempDir().appendingPathComponent("wt", isDirectory: true)
            try! fm.createDirectory(at: gitdir, withIntermediateDirectories: true)
            try! "ref: refs/heads/feature/wt\n".write(to: gitdir.appendingPathComponent("HEAD"), atomically: true, encoding: .utf8)
            try! "gitdir: \(gitdir.path)\n".write(to: cwd.appendingPathComponent(".git"), atomically: true, encoding: .utf8)
        } else {
            try! fm.createDirectory(at: cwd.appendingPathComponent(".git"), withIntermediateDirectories: true)
            try! "ref: refs/heads/main\n".write(to: cwd.appendingPathComponent(".git/HEAD"), atomically: true, encoding: .utf8)
        }
        let file = writeTranscript(projects, cwd: cwd.path, id: id, lines: [])
        append(file, Fixture.real(bytes: transcriptBytes))
        for _ in 0..<fillerMB { append(file, Fixture.fillerMB) }
        if pendingTool { append(file, toolUseLine(id: "toolu_perm", name: "Bash", input: ["command": "rm -rf build"]) + "\n") }
        for t in 1...max(1, taskFiles) where taskFiles > 0 {
            writeTask(tasks, session: id, id: t, status: t == 1 ? "in_progress" : "pending", subject: "Task \(t)", activeForm: "Doing task \(t)")
        }
        writeSession(dir, id, pid: me, status: status, waitingFor: waitingFor, ageSec: 60, name: id, cwd: cwd.path)
        ids.append(id)
        return id
    }

    func remove(_ id: String) {
        try? FileManager.default.removeItem(at: dir.appendingPathComponent("\(id).json"))
        ids.removeAll { $0 == id }
    }

    func transcript(_ id: String) -> URL {
        let cwd = (try! JSONSerialization.jsonObject(with: Data(contentsOf: dir.appendingPathComponent("\(id).json"))) as! [String: Any])["cwd"] as! String
        return projects.appendingPathComponent(Transcript.projectDirName(cwd)).appendingPathComponent("\(id).jsonl")
    }

    /// 10 sessions: 3 busy, 4 idle, 3 waiting (one on a permission prompt); ~2 MB transcripts, the first one 64 MB.
    static func standard() -> Fixture {
        let f = Fixture()
        for i in 0..<3 { f.add(status: "busy", transcriptBytes: 2 << 20, fillerMB: i == 0 ? 62 : 0) }
        for i in 0..<4 { f.add(status: "idle", transcriptBytes: 2 << 20, worktree: i == 0) }
        for i in 0..<3 {
            f.add(status: "waiting", waitingFor: i == 2 ? "permission prompt" : nil, transcriptBytes: 2 << 20, pendingTool: i == 2)
        }
        f.warmUp()
        return f
    }

    /// Refreshes until the scanner has parsed everything: `parsedLines` flat for 3 refreshes, after at least as many
    /// refreshes as the 64 MB transcript has 1 MB chunks (the filler part parses nothing, so it looks flat too).
    func warmUp() {
        var flat = 0
        var last = -1
        for n in 1...200 {
            store.refresh()
            let parsed = scanner.parsedLines
            flat = parsed == last ? flat + 1 : 0
            last = parsed
            if flat >= 3 && n >= 70 { return }
        }
    }

    struct Counters {
        var bytes = 0, parses = 0, heads = 0, listings = 0, parsed = 0
        static func - (a: Counters, b: Counters) -> Counters {
            Counters(bytes: a.bytes - b.bytes, parses: a.parses - b.parses, heads: a.heads - b.heads,
                     listings: a.listings - b.listings, parsed: a.parsed - b.parsed)
        }
    }

    var counters: Counters {
        Counters(bytes: store.transcriptBytesRead, parses: store.sessionParses, heads: store.headReads,
                 listings: store.taskListings, parsed: scanner.parsedLines)
    }
}

@Suite(.serialized) struct PerformanceTests {
    private func steadyDeltas() -> Fixture.Counters {
        let f = Fixture.standard()
        let before = f.counters
        for _ in 0..<20 { f.store.refresh() }
        let d = f.counters - before
        print("steady x20 deltas: bytes=\(d.bytes) sessionParses=\(d.parses) headReads=\(d.heads) taskListings=\(d.listings) parsedLines=\(d.parsed)")
        return d
    }

    @Test func steadyRefreshReadsNoTranscriptBytes() {
        let d = steadyDeltas()
        #expect(d.bytes == 0)
        #expect(d.parsed == 0)
    }

    @Test(.disabled("enabled in phase 2")) func steadyRefreshReadsNothing() {
        let d = steadyDeltas()
        #expect(d.parses == 0)
        #expect(d.heads == 0)
        #expect(d.listings == 0)
    }

    /// 10 rounds of K bytes of `tool_use` lines appended to the 3 busy transcripts, each followed by a refresh.
    private func appendRounds() -> [(appended: Int, delta: Fixture.Counters)] {
        let f = Fixture.standard()
        let busy = f.ids.prefix(3).map(f.transcript)
        var n = 0
        func chunk() -> String {
            var s = ""
            while s.utf8.count < 4096 {
                n += 1
                s += toolUseLine(id: "toolu_a\(n)", name: "Bash", input: ["command": "echo \(n)"]) + "\n"
            }
            return s
        }
        var rounds: [(Int, Fixture.Counters)] = []
        for round in 1...10 {
            let before = f.counters
            var appended = 0
            for url in busy {
                let c = chunk()
                appended += c.utf8.count
                append(url, c)
            }
            f.store.refresh()
            let d = f.counters - before
            print("append round \(round): appended=\(appended) bytesRead=\(d.bytes) sessionParses=\(d.parses)")
            rounds.append((appended, d))
        }
        return rounds
    }

    @Test func appendReadsOnlyNewBytes() {
        let longest = toolUseLine(id: "toolu_a99999", name: "Bash", input: ["command": "echo 99999"]).utf8.count + 1
        for r in appendRounds() { #expect(r.delta.bytes <= r.appended + 3 * longest) }
    }

    @Test(.disabled("enabled in phase 2")) func appendDoesNotReparseSessions() {
        for r in appendRounds() { #expect(r.delta.parses == 0) }
    }

    @Test func refreshMedianUnder10ms() {
        let f = Fixture.standard()
        var times: [Duration] = []
        let clock = ContinuousClock()
        for _ in 0..<50 { times.append(clock.measure { f.store.refresh() }) }
        times.sort()
        let median = times[times.count / 2]
        let p95 = times[Int(Double(times.count) * 0.95)]
        print("refresh median=\(median) p95=\(p95)")
        #expect(median < .milliseconds(10))
    }

    @Test func cachesFollowLiveSessions() {
        let f = Fixture()
        for i in 0..<10 { f.add(status: ["busy", "idle", "waiting"][i % 3], transcriptBytes: 16 << 10, taskFiles: 2) }
        f.store.refresh()
        var worst = 0
        for _ in 0..<25 {
            for _ in 0..<2 {
                let id = f.add(status: "idle", transcriptBytes: 16 << 10, taskFiles: 2)
                f.store.surfaces[id] = "surface-\(id)"
            }
            for id in f.ids.prefix(2) { f.remove(id) }
            f.store.refresh()
            let live = f.ids.count
            for (name, count) in f.store.cacheCounts {
                worst = max(worst, count - live)
                #expect(count <= live, "\(name) holds \(count) entries for \(live) live sessions")
            }
        }
        print("caches: worst excess over live sessions = \(worst)")
        for id in f.ids { f.remove(id) }
        f.store.refresh()
        for (name, count) in f.store.cacheCounts { #expect(count == 0, "\(name) holds \(count) entries with no session left") }
    }
}
