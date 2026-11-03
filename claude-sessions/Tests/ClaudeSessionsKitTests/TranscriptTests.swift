import Foundation
import Testing
@testable import ClaudeSessionsKit

private let userLine = #"{"type":"user","message":{"content":"hi"}}"#
private func data(_ lines: [String]) -> Data { Data(lines.joined(separator: "\n").utf8) }

@Test func projectDirName() {
    #expect(Transcript.projectDirName("/Users/phong/.claude/skills-repo") == "-Users-phong--claude-skills-repo")
}

@Test func lastReplyPicksLastText() {
    #expect(Transcript.lastReply(in: data([assistantLine("one"), assistantLine("two")])) == "two")
}

@Test func lastReplySkipsToolOnlyEntries() {
    let toolOnly = #"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash"}]}}"#
    #expect(Transcript.lastReply(in: data([assistantLine("the text"), toolOnly, userLine])) == "the text")
}

@Test func lastReplyIgnoresCutFirstLine() {
    let cut = String(assistantLine("x").dropFirst(20))
    #expect(Transcript.lastReply(in: data([cut, assistantLine("kept")])) == "kept")
    #expect(Transcript.lastReply(in: data([cut])) == nil)
}

@Test func lastReplySkipsSidechain() {
    #expect(Transcript.lastReply(in: data([assistantLine("main"), assistantLine("side", sidechain: true)])) == "main")
}

@Test func lastReplyCollapsesAndCaps() {
    #expect(Transcript.lastReply(in: data([assistantLine("a\n\nb   c\n")])) == "a b c")
    #expect(Transcript.lastReply(in: data([assistantLine(String(repeating: "x", count: 400))]))?.count == 300)
}

@Test func lastReplyNone() {
    #expect(Transcript.lastReply(in: data([userLine, assistantLine(nil)])) == nil)
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
