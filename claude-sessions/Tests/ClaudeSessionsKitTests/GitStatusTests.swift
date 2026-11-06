import Foundation
import Testing
@testable import ClaudeSessionsKit

struct GitStatusTests {
    @Test func parseClean() {
        let out = "# branch.oid x\n# branch.head main\n# branch.upstream origin/main\n# branch.ab +0 -0\n"
        #expect(GitStatus.parse(out) == GitCounts(dirty: 0, ahead: 0))
    }

    @Test func parseDirtyAhead() {
        let out = "# branch.oid x\n# branch.head main\n# branch.upstream origin/main\n# branch.ab +3 -1\n1 .M N... 100644 100644 100644 a b a.txt\n1 M. N... 100644 100644 100644 a b b.txt\n? new.txt\n"
        #expect(GitStatus.parse(out) == GitCounts(dirty: 3, ahead: 3))
    }

    @Test func noUpstream() {
        #expect(GitStatus.parse("# branch.oid x\n# branch.head main\n? a\n") == GitCounts(dirty: 1, ahead: nil))
    }

    @Test func detached() {
        #expect(GitStatus.parse("# branch.oid x\n# branch.head (detached)\n") == GitCounts(dirty: 0, ahead: nil))
    }

    private final class Runs { var roots: [String] = []; var out: String? = "? a\n" }

    private func monitor(_ r: Runs) -> GitStatusMonitor {
        GitStatusMonitor(queue: nil, interval: 30) { root in r.roots.append(root); return r.out }
    }

    @Test func monitorSharesRoot() {
        let r = Runs(), m = monitor(r)
        m.update(roots: Set(["/r", "/r"]), headChanged: [], at: Date())
        #expect(r.roots == ["/r"])
        #expect(m.takeSnapshot() == ["/r": GitCounts(dirty: 1, ahead: nil)])
    }

    @Test func monitorInterval() {
        let r = Runs(), m = monitor(r), t = Date()
        m.update(roots: ["/r"], headChanged: [], at: t)
        m.update(roots: ["/r"], headChanged: [], at: t.addingTimeInterval(10))
        #expect(r.roots.count == 1)
        m.update(roots: ["/r"], headChanged: [], at: t.addingTimeInterval(11))
        m.update(roots: ["/r"], headChanged: ["/r"], at: t.addingTimeInterval(12))
        #expect(r.roots.count == 2)
        m.update(roots: ["/r"], headChanged: [], at: t.addingTimeInterval(43))
        #expect(r.roots.count == 3)
    }

    @Test func failureKeepsLast() {
        let r = Runs(), m = monitor(r), t = Date()
        m.update(roots: ["/r"], headChanged: [], at: t)
        r.out = nil
        m.update(roots: ["/r"], headChanged: [], at: t.addingTimeInterval(31))
        #expect(r.roots.count == 2)
        #expect(m.takeSnapshot()["/r"] == GitCounts(dirty: 1, ahead: nil))
    }

    @Test func dropsUnusedRoots() {
        let r = Runs(), m = monitor(r), t = Date()
        m.update(roots: ["/a", "/b"], headChanged: [], at: t)
        m.update(roots: ["/a"], headChanged: [], at: t.addingTimeInterval(1))
        #expect(Set(m.takeSnapshot().keys) == ["/a"])
    }

    @Test func repoRootWorktree() throws {
        let dir = makeTempDir()
        let wt = dir.appendingPathComponent("wt"), sub = wt.appendingPathComponent("a/b", isDirectory: true)
        try FileManager.default.createDirectory(at: sub, withIntermediateDirectories: true)
        try "gitdir: /elsewhere/.git/worktrees/wt\n".write(to: wt.appendingPathComponent(".git"), atomically: true, encoding: .utf8)
        #expect(GitBranch.repoRoot(in: sub.path) == wt.path)
        #expect(GitBranch.repoRoot(in: "relative") == nil)
    }
}
