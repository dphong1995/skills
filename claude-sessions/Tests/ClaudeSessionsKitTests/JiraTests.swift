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
        #expect(body?["fields"] as? [String] == ["summary", "status", "issuetype"])
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

    private static let iconURL = "https://talosix.atlassian.net/rest/api/2/universal_avatar/view/type/issuetype/avatar/10315?size=medium"

    private func bulk(_ category: String?) -> [String: JiraIssue]? {
        let cat = category.map { #","statusCategory":{"key":"\#($0)","colorName":"yellow"}"# } ?? ""
        let json = #"{"issues":[{"key":"T6IR-13681","fields":{"summary":"S","issuetype":{"name":"Story/New Feature Request","iconUrl":"\#(Self.iconURL)"},"status":{"name":"IMPLEMENTED"\#(cat)}}}]}"#
        return Jira.issues(from: Data(json.utf8), requested: ["T6IR-13681"])
    }

    @Test func parsesTypeIconAndTone() {
        #expect(bulk("indeterminate")?["T6IR-13681"] == JiraIssue(title: "S", status: "IMPLEMENTED", type: "Story/New Feature Request",
                                                                  iconURL: URL(string: Self.iconURL), tone: .doing))
        #expect(bulk("new")?["T6IR-13681"]?.tone == .todo)
        #expect(bulk("done")?["T6IR-13681"]?.tone == .done)
    }

    @Test func parsesUnknownCategoryAsNilTone() {
        #expect(bulk("weird")?["T6IR-13681"]?.tone == nil)
        #expect(bulk(nil)?["T6IR-13681"]?.tone == nil)
        let json = #"{"issues":[{"key":"A-1","fields":{"summary":"S","status":{"name":"X"}}}]}"#
        let i = Jira.issues(from: Data(json.utf8), requested: ["A-1"])?["A-1"]
        #expect(i?.type == nil)
        #expect(i?.iconURL == nil)
    }

    @Test func iconRequestShape() throws {
        let req = try #require(JiraClient.iconRequest(url: URL(string: Self.iconURL)!, auth: "Basic x"))
        #expect(req.httpMethod == "GET")
        let items = URLComponents(url: try #require(req.url), resolvingAgainstBaseURL: false)?.queryItems ?? []
        #expect(items.contains(URLQueryItem(name: "size", value: "medium")))
        #expect(items.contains(URLQueryItem(name: "format", value: "png")))
        #expect(req.value(forHTTPHeaderField: "Authorization") == "Basic x")
        #expect(JiraClient.iconRequest(url: URL(string: "https://evil.example/x.png")!, auth: "x") == nil)
        #expect(JiraClient.iconRequest(url: URL(string: "https://atlassian.net.evil.example/x")!, auth: "x") == nil)
    }
}
