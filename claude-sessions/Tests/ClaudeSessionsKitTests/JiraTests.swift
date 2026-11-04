import Testing
import Foundation
@testable import ClaudeSessionsKit

@Suite struct JiraTests {
    @Test func parsesBulkFetch() {
        let json = #"{"issues":[{"key":"T6IR-104","fields":{"summary":"Fix login","status":{"name":"Code Review"}}}],"issueErrors":[]}"#
        let r = Jira.issues(from: Data(json.utf8), requested: ["T6IR-104", "T6IR-999"])
        #expect(r?["T6IR-104"] == JiraIssue(title: "Fix login", status: "Code Review"))
        #expect(r?["T6IR-999"] == JiraIssue(title: nil, status: nil))
    }

    @Test func parseRejectsNonJSON() {
        #expect(Jira.issues(from: Data("nope".utf8), requested: ["A-1"]) == nil)
    }

    @Test func bulkFetchRequestShape() throws {
        let req = try #require(JiraClient.request(host: "talosix.atlassian.net", keys: ["T6IR-104"], auth: "Basic " + Data("a@b.c:tok".utf8).base64EncodedString()))
        #expect(req.httpMethod == "POST")
        #expect(req.url?.absoluteString == "https://talosix.atlassian.net/rest/api/3/issue/bulkfetch")
        #expect(req.value(forHTTPHeaderField: "Authorization") == "Basic " + Data("a@b.c:tok".utf8).base64EncodedString())
        #expect(req.value(forHTTPHeaderField: "Content-Type") == "application/json")
        let body = try JSONSerialization.jsonObject(with: try #require(req.httpBody)) as? [String: Any]
        #expect(body?["issueIdsOrKeys"] as? [String] == ["T6IR-104"])
        #expect(body?["fields"] as? [String] == ["summary", "status"])
    }

    @Test func requestRefusesForeignHosts() {
        #expect(JiraClient.request(host: "evil.example", keys: ["T6IR-1"], auth: "x") == nil)
        #expect(JiraClient.request(host: "atlassian.net.evil.example", keys: ["T6IR-1"], auth: "x") == nil)
        #expect(JiraClient.request(host: "Talosix.Atlassian.NET", keys: ["T6IR-1"], auth: "x") != nil)
    }

    @Test func connectionMapsJiraErrors() {
        #expect(ConnectionState.after(JiraError.http(401), previous: .ok) == .rejected)
        #expect(ConnectionState.after(JiraError.transport, previous: .ok) == .unreachable)
        #expect(ConnectionState.after(JiraError.http(503), previous: .ok) == .unreachable)
        #expect(ConnectionState.after(JiraError.http(429), previous: .ok) == .unreachable)
        #expect(ConnectionState.after(JiraError.http(404), previous: .rejected) == .rejected)
    }
}
