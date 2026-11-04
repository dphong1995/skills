import Foundation
import Testing
@testable import ClaudeSessionsKit

private let plainUser = #"{"type":"user","message":{"content":"hi"}}"#
private func data(_ lines: [String]) -> Data { Data(lines.joined(separator: "\n").utf8) }

@Test func projectDirName() {
    #expect(Transcript.projectDirName("/Users/phong/.claude/skills-repo") == "-Users-phong--claude-skills-repo")
}

@Test func lastReplyPicksLastText() {
    #expect(Transcript.lastReply(in: data([assistantLine("one"), assistantLine("two")])) == "two")
}

@Test func lastReplySkipsToolOnlyEntries() {
    let toolOnly = #"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash"}]}}"#
    #expect(Transcript.lastReply(in: data([assistantLine("the text"), toolOnly, plainUser])) == "the text")
}

@Test func lastReplyIgnoresCutFirstLine() {
    let cut = String(assistantLine("x").dropFirst(20))
    #expect(Transcript.lastReply(in: data([cut, assistantLine("kept")])) == "kept")
    #expect(Transcript.lastReply(in: data([cut])) == nil)
}

@Test func lastReplySkipsSidechain() {
    #expect(Transcript.lastReply(in: data([assistantLine("main"), assistantLine("side", sidechain: true)])) == "main")
}

@Test func lastReplyCollapsesWithoutCap() {
    #expect(Transcript.lastReply(in: data([assistantLine("a\n\nb   c\n")])) == "a b c")
    #expect(Transcript.lastReply(in: data([assistantLine(String(repeating: "x", count: 400))]))?.count == 400)
}

@Test func lastReplyNone() {
    #expect(Transcript.lastReply(in: data([plainUser, assistantLine(nil)])) == nil)
}

@Test func findFallsBackToScan() throws {
    let projects = makeTempDir()
    let other = projects.appendingPathComponent("some-other-dir")
    try FileManager.default.createDirectory(at: other, withIntermediateDirectories: true)
    let file = other.appendingPathComponent("sid.jsonl")
    try Data().write(to: file)
    #expect(Transcript.find(sessionId: "sid", cwd: "/nope", projects: projects)?.resolvingSymlinksInPath().path == file.resolvingSymlinksInPath().path)
    #expect(Transcript.find(sessionId: "missing", cwd: "/nope", projects: projects) == nil)
}

private func bash(_ id: String, _ cmd: String = "npm test") -> String {
    toolUseLine(id: id, name: "Bash", input: ["command": cmd])
}

@Test func pendingOneBash() {
    #expect(Transcript.pending(in: data([bash("t1")])) == .one(Pending(toolId: "t1", tool: "Bash", text: "npm test")))
}

@Test func pendingClearedByToolResult() {
    #expect(Transcript.pending(in: data([bash("t1"), toolResultLine(id: "t1")])) == nil)
}

@Test func pendingParallelIsUnclear() {
    #expect(Transcript.pending(in: data([bash("t1"), bash("t2")])) == .unclear(count: 2))
}

@Test func pendingAgentOnlyIsUnclear() {
    #expect(Transcript.pending(in: data([toolUseLine(id: "t1", name: "Agent", input: [:])])) == .unclear(count: 1))
}

@Test func pendingUnknownToolIsUnclear() {
    #expect(Transcript.pending(in: data([toolUseLine(id: "t1", name: "mcp__x__y", input: [:])])) == .unclear(count: 1))
    #expect(Transcript.pending(in: data([toolUseLine(id: "t1", name: "ExitPlanMode", input: [:])])) == .unclear(count: 1))
}

@Test func pendingEmptyCommandIsUnclear() {
    #expect(Transcript.pending(in: data([toolUseLine(id: "t1", name: "Bash", input: [:])])) == .unclear(count: 1))
    #expect(Transcript.pending(in: data([toolUseLine(id: "t1", name: "Bash", input: ["command": ""])])) == .unclear(count: 1))
}

@Test func pendingSkipsSidechain() {
    #expect(Transcript.pending(in: data([toolUseLine(id: "t1", name: "Bash", input: ["command": "x"], sidechain: true)])) == nil)
}

@Test func pendingTextPerTool() {
    func text(_ name: String, _ input: [String: Any]) -> String? {
        if case .one(let p) = Transcript.pending(in: data([toolUseLine(id: "t1", name: name, input: input)])) { return p.text }
        return nil
    }
    #expect(text("Edit", ["file_path": "/a/b.swift"]) == "/a/b.swift")
    #expect(text("WebFetch", ["url": "https://x.dev"]) == "https://x.dev")
    #expect(text("WebSearch", ["query": "swift testing"]) == "swift testing")
    #expect(text("Grep", ["pattern": "foo.*"]) == "foo.*")
}

@Test func cleanPromptRules() {
    #expect(Transcript.cleanPrompt("<command-name>/cook</command-name><command-message>cook</command-message><command-args>plans/x/plan.md</command-args>") == "/cook plans/x/plan.md")
    #expect(Transcript.cleanPrompt("<command-name>/clear</command-name>\n<command-message>clear</command-message>\n<command-args></command-args>") == nil)
    #expect(Transcript.cleanPrompt("<local-command-stdout>ok</local-command-stdout>") == nil)
    #expect(Transcript.cleanPrompt("Caveat: The messages below were generated") == nil)
    #expect(Transcript.cleanPrompt("sửa bug\n<system-reminder>x</system-reminder>  đăng nhập") == "sửa bug đăng nhập")
    #expect(Transcript.cleanPrompt("xem log <command-name>/x</command-name>") == "xem log <command-name>/x</command-name>")
    let long = String(repeating: "a", count: 70)
    #expect(Transcript.cleanPrompt(long) == long)
}

@Test func firstPromptSkipsClearAndLocalCommand() {
    let meta = #"{"type":"user","isMeta":true,"message":{"content":"meta text"}}"#
    let local = userLine("<local-command-stdout>ok</local-command-stdout>")
    let lines = [commandLine("/clear", args: ""), local, meta, toolResultLine(id: "t"), userLine("sửa widget"), userLine("câu sau")]
    #expect(Transcript.firstPrompt(in: data(lines)) == "sửa widget")
}

@Test func firstPromptArrayContent() {
    let l = #"{"type":"user","message":{"content":[{"type":"text","text":"xin chào"}]}}"#
    #expect(Transcript.firstPrompt(in: data([l])) == "xin chào")
}

@Test func firstPromptNone() {
    #expect(Transcript.firstPrompt(in: data([commandLine("/clear", args: ""), assistantLine("x")])) == nil)
}

@Test func lastAITitleAndBranch() {
    let empty = #"{"type":"assistant","gitBranch":"","message":{"content":[]}}"#
    let d = data([aiTitleLine("old"), userLine("a", branch: "main"), aiTitleLine("new"), assistantLine("b", branch: "feat-x"), empty])
    #expect(Transcript.lastAITitle(in: d) == "new")
    #expect(Transcript.gitBranch(in: d, last: true) == "feat-x")
    #expect(Transcript.gitBranch(in: d, last: false) == "main")
}

@Test func headDropsPartialLine() {
    let dir = makeTempDir()
    let f = dir.appendingPathComponent("h.jsonl")
    let l1 = userLine("one"), l2 = userLine("two is a longer line"), l3 = userLine("three")
    try! Data([l1, l2, l3].joined(separator: "\n").utf8).write(to: f)
    let bytes = l1.utf8.count + 1 + 10
    let h = Transcript.head(f, bytes: bytes)
    #expect(Transcript.firstPrompt(in: h) == "one")
    #expect(h.last != nil && String(decoding: h, as: UTF8.self) == l1 + "\n")
}

@Test func labelRules() {
    let cases = [
        ("https://bitbucket.org/t6engineering/talo6-dct-backend/pull-requests/7562", "PR #7562"),
        ("https://bitbucket.org/t6engineering/talo6-dct-frontend/pull-requests/6867/_/diff", "PR #6867"),
        ("https://github.com/formio/formio.js/pull/3759", "PR #3759"),
        ("https://gitlab.com/a/b/-/merge_requests/12", "PR #12"),
        ("https://talosix.atlassian.net/browse/T6IR-14728", "T6IR-14728"),
        ("https://talosix.atlassian.net/browse/T6IR-13103?focusedCommentId=88378", "T6IR-13103"),
        ("https://github.com/a/b/issues/42", "#42"),
        ("https://www.anthropic.com/legal/consumer-terms", "anthropic.com"),
        ("https://bitbucket.org/t6engineering/talo6-dct-frontend/pull-requests/new?source=x", "bitbucket.org"),
        ("http://localhost:8080", "localhost"),
    ]
    for (u, l) in cases { #expect(Transcript.label(for: URL(string: u)!) == l, "\(u)") }
}

@Test func displayTextReplacesUrlsAndCuts() {
    #expect(Transcript.displayText("/cook https://talosix.atlassian.net/browse/T6IR-14728", limit: 140) == "/cook T6IR-14728")
    #expect(Transcript.displayText("xem **PR** [đây](https://github.com/a/b/pull/9).", limit: 140) == "xem **PR** đây.")
    #expect(Transcript.displayText("done: https://bitbucket.org/x/y/pull-requests/7562**", limit: 140) == "done: PR #7562**")
    #expect(Transcript.displayText(String(repeating: "a", count: 700), limit: 600) == String(repeating: "a", count: 600) + "…")
    let exact = String(repeating: "a", count: 600)
    #expect(Transcript.displayText(exact, limit: 600) == exact)
}

@Test func markdownStylesWithoutLinks() {
    let a = markdown("a **b** `c` [d](https://x.y)")
    #expect(String(a.characters) == "a b c d")
    #expect(a.runs.allSatisfy { $0.link == nil })
    #expect(String(markdown("**open").characters) == "**open")
}
