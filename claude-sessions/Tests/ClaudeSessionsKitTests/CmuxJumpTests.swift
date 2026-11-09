import Foundation
import Testing
@testable import ClaudeSessionsKit

private let line = "claude 7d0fcbef-0908-41eb-9779-ea86d5bf838a  workspace=4E6B8144-0E40-4E20-A810-62245241C5A0  surface=20FAA4D5-E0C5-4F59-9D8E-277C0038558C  cwd=/Users/phong/.claude/skills-repo  active_ws=no  active_surface=yes  updated=2026-11-03T13:47:06.593Z  scratch=no  session_dir=-  fork_command=yes  fork=yes  pid_exists=yes"

@Test func surfaceFound() {
    #expect(CmuxJump.surface(fromSessionsOutput: line) == "20FAA4D5-E0C5-4F59-9D8E-277C0038558C")
}

@Test func surfaceMissing() {
    #expect(CmuxJump.surface(fromSessionsOutput: "") == nil)
}

@Test func surfaceFirstLine() {
    let second = line.replacingOccurrences(of: "20FAA4D5-E0C5-4F59-9D8E-277C0038558C", with: "AAAAAAAA-E0C5-4F59-9D8E-277C0038558C")
    #expect(CmuxJump.surface(fromSessionsOutput: line + "\n" + second) == "20FAA4D5-E0C5-4F59-9D8E-277C0038558C")
}

@Test func approveKeys() {
    #expect(CmuxJump.keys(for: .approve("s", toolId: "t")) == [.raw("1")])
}

@Test func denyKeysWithoutReason() {
    #expect(CmuxJump.keys(for: .deny("s", toolId: "t", reason: "")) == [.raw("\\x1b")])
    #expect(CmuxJump.keys(for: .deny("s", toolId: "t", reason: " \n ")) == [.raw("\\x1b")])
}

@Test func denyKeysWithReason() {
    #expect(CmuxJump.keys(for: .deny("s", toolId: "t", reason: "chỉ chạy make clean"))
        == [.raw("\\x1b"), .pause(0.3), CmuxJump.clearInput, .paste("chỉ chạy make clean"), .raw("\\r")])
}

// A half-typed draft in Claude's input must not be sent with the deny reason: Ctrl-E, then Ctrl-U + Backspace per line.
@Test func clearInputEmptiesMultiLineDraft() {
    #expect(CmuxJump.clearInput == .raw("\\x05" + String(repeating: "\\x15\\x7f", count: 20)))
}

@Test func scriptRaw() {
    let s = CmuxJump.script(surface: "S", key: .raw("\\x1b"))
    #expect(s.contains(#"perform action "text:\\x1b" on (first terminal whose id is "S")"#))
}

@Test func scriptEscapesPaste() {
    let s = CmuxJump.script(surface: "S", key: .paste("a \"b\" \\ c\nd"))
    #expect(s.contains("input text \"a \\\"b\\\" \\\\ c\nd\" to (first terminal whose id is \"S\")"))
}

@Test func scriptPauseIsEmpty() {
    #expect(CmuxJump.script(surface: "S", key: .pause(0.3)).isEmpty)
}

@Test func openLinkSendsNoKeys() {
    #expect(CmuxJump.keys(for: .openLink("s", URL(string: "https://x.y")!)).isEmpty)
}

@Test func askClaudeKeys() {
    #expect(CmuxJump.keys(for: .askClaude("s", prompt: "Fix PR", pr: PRRef(workspace: "w", repo: "r", id: 1))) == [CmuxJump.clearInput, .paste("Fix PR"), .raw("\\r")])
}

@Test func replyKeys() {
    #expect(CmuxJump.keys(for: .reply("s", text: "say hi")) == [CmuxJump.clearInput, .paste("say hi"), .raw("\\r")])
    #expect(CmuxJump.keys(for: .reply("s", text: "a\nb")) == [CmuxJump.clearInput, .paste("a\nb"), .raw("\\r")])
}

@Test func stopKeys() {
    #expect(CmuxJump.keys(for: .stop("s")) == [.raw("\\x1b")])
}

private let identify = #"{"focused": {"surface_id": "6E39B188-F067-48FA-852D-3CE78A94EF92", "surface_type": "terminal", "pane_id": "P"}, "caller": null}"#

@Test func focusedSurfaceFromIdentify() {
    #expect(CmuxJump.focusedSurface(fromIdentify: Data(identify.utf8)) == "6E39B188-F067-48FA-852D-3CE78A94EF92")
}

@Test func focusedSurfaceIgnoresBrowser() {
    let browser = identify.replacingOccurrences(of: "terminal", with: "browser")
    #expect(CmuxJump.focusedSurface(fromIdentify: Data(browser.utf8)) == nil)
}

@Test func focusedSurfaceBadJSON() {
    #expect(CmuxJump.focusedSurface(fromIdentify: Data("nope".utf8)) == nil)
    #expect(CmuxJump.focusedSurface(fromIdentify: Data("{}".utf8)) == nil)
}

@Test func answerSendsNoKeystrokes() {
    #expect(CmuxJump.keys(for: .answer("a", requestId: "R", answers: [["x"]], other: [:])).isEmpty)
}
