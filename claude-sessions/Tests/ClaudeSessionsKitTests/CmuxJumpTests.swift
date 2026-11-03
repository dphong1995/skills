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
