import Testing
import Foundation
@testable import ClaudeSessionsKit

@Suite struct CredentialsTests {
    @Test func addScriptQuotesValues() {
        #expect(Credentials.addScript(service: "jira-api-token", account: "a@b.com", token: "AbC_12-=")
            == "add-generic-password -U -s \"jira-api-token\" -a \"a@b.com\" -w \"AbC_12-=\"\n")
    }

    @Test func addScriptRejectsUnsafeInput() {
        #expect(Credentials.addScript(service: "s", account: "a\"b", token: "t") == nil)
        #expect(Credentials.addScript(service: "s", account: "a", token: "t\\u") == nil)
        #expect(Credentials.addScript(service: "s", account: "a", token: "t\nu") == nil)
    }

    @Test func isSafeRefusesQuoteBackslashNewline() {
        #expect(Credentials.isSafe("a@b.com"))
        #expect(Credentials.isSafe("AbC_12-= ?"))
        #expect(!Credentials.isSafe("a\"b"))
        #expect(!Credentials.isSafe("a\\b"))
        #expect(!Credentials.isSafe("a\nb"))
        #expect(!Credentials.isSafe("a\r\nb"))
    }

    @Test func connectionMapping() {
        #expect(ConnectionState.after(nil, previous: .rejected) == .ok)
        #expect(ConnectionState.after(BitbucketError.http(401), previous: .ok) == .rejected)
        #expect(ConnectionState.after(BitbucketError.transport, previous: .ok) == .unreachable)
        #expect(ConnectionState.after(BitbucketError.http(429), previous: .ok) == .unreachable)
        #expect(ConnectionState.after(BitbucketError.http(503), previous: .ok) == .unreachable)
        #expect(ConnectionState.after(BitbucketError.http(404), previous: .rejected) == .rejected)
    }

    @Test func accountParsedFromKeychainAttributes() {
        let out = "keychain: \"/x/login.keychain-db\"\nattributes:\n    \"acct\"<blob>=\"phong@example.com\"\n    \"svce\"<blob>=\"bitbucket-api-token\"\n"
        #expect(Credentials.account(fromAttributes: out) == "phong@example.com")
        #expect(Credentials.account(fromAttributes: "    \"acct\"<blob>=<NULL>\n") == nil)
    }
}
