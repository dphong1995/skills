import Foundation
import Testing
@testable import ClaudeSessionsKit

func assistantLine(_ text: String?, sidechain: Bool = false) -> String {
    var content: [[String: Any]] = [["type": "tool_use", "name": "Bash"]]
    if let text { content.insert(["type": "text", "text": text], at: 0) }
    let o: [String: Any] = ["type": "assistant", "isSidechain": sidechain, "message": ["content": content]]
    return String(data: try! JSONSerialization.data(withJSONObject: o), encoding: .utf8)!
}

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
    let projects = makeSessionsDir()
    let other = projects.appendingPathComponent("some-other-dir")
    try FileManager.default.createDirectory(at: other, withIntermediateDirectories: true)
    let file = other.appendingPathComponent("sid.jsonl")
    try Data().write(to: file)
    #expect(Transcript.find(sessionId: "sid", cwd: "/nope", projects: projects)?.resolvingSymlinksInPath().path == file.resolvingSymlinksInPath().path)
    #expect(Transcript.find(sessionId: "missing", cwd: "/nope", projects: projects) == nil)
}
