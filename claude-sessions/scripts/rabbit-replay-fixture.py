#!/usr/bin/env python3
"""Build the sanitized CodeRabbit replay fixture from Bitbucket (read-only GETs).

  python3 -I scripts/rabbit-replay-fixture.py <out.json>
  python3 -I scripts/rabbit-replay-fixture.py --from-cache <cache.json> <out.json>   # dry run: raw dump lacks participants/author

The token is read from Keychain service `bitbucket-api-token` and never printed or stored.
Only whitelisted comment lines survive; names, uuids and code are dropped or aliased.
"""
import base64, json, re, subprocess, sys, time, urllib.error, urllib.request

WS = "t6engineering"
REPOS = ("talo6-dct-backend", "talo6-dct-frontend")
MARKER = "coderabbit.ai/#html-comment"
MARKER_LINE = "[coderabbit-html-comment-v1-0]: https://coderabbit.ai/#html-comment-v1:x"
WHITELIST = re.compile(
    r"Actionable comments posted: \d+|No actionable comments|Addressed in commit|@coderabbit(ai)?\s+approve"
    r"|approval failed|Review thread resolved|Comments resolved", re.I)
COMMENT_FIELDS = ("values.user.uuid,values.created_on,values.updated_on,values.deleted,values.content.raw,"
                  "values.id,values.parent.id,values.inline.path,next")
PR_FIELDS = "id,state,comment_count,source.commit.hash,author.uuid,participants.role,participants.approved,participants.state,participants.user.uuid"


def auth_header():
    out = subprocess.run(["security", "find-generic-password", "-s", "bitbucket-api-token"], capture_output=True, text=True).stdout
    acct = re.search(r'"acct"<blob>="([^"]+)"', out).group(1)
    tok = subprocess.run(["security", "find-generic-password", "-s", "bitbucket-api-token", "-w"], capture_output=True, text=True).stdout.strip()
    return {"Authorization": "Basic " + base64.b64encode(f"{acct}:{tok}".encode()).decode(), "Accept": "application/json"}


def get(path, headers):
    while True:
        try:
            return json.load(urllib.request.urlopen(urllib.request.Request("https://api.bitbucket.org" + path, headers=headers), timeout=60))
        except urllib.error.HTTPError as e:
            if e.code != 429:
                raise
            time.sleep(int(e.headers.get("Retry-After", 60)))


def pages(path, headers):
    vals, nxt = [], path
    while nxt:
        o = get(nxt, headers)
        vals += o["values"]
        nxt = (o.get("next") or "").replace("https://api.bitbucket.org", "") or None
    return vals


def fetch(headers):
    raw = {}
    for repo in REPOS:
        ids = []
        for state, n in (("MERGED", 3), ("OPEN", 1), ("DECLINED", 1)):
            nxt, p = f"/2.0/repositories/{WS}/{repo}/pullrequests?state={state}&pagelen=50&fields=values.id,next", 0
            while nxt and p < n:
                o = get(nxt, headers)
                ids += [v["id"] for v in o["values"]]
                nxt = (o.get("next") or "").replace("https://api.bitbucket.org", "") or None
                p += 1
        for i in ids:
            base = f"/2.0/repositories/{WS}/{repo}/pullrequests/{i}"
            raw[f"{repo}#{i}"] = {
                "pr": get(f"{base}?fields={PR_FIELDS}", headers),
                "comments": pages(f"{base}/comments?pagelen=100&fields={COMMENT_FIELDS}", headers),
                "statuses": get(f"{base}/statuses?pagelen=50", headers)["values"],
            }
    return raw


def sanitize_raw(raw):
    lines = raw.split("\n")
    out, dropped = [], False
    marked = False
    for ln in lines:
        if MARKER in ln:
            if not marked:
                out.append(MARKER_LINE)
                marked = True
            else:
                dropped = True
        elif WHITELIST.search(ln):
            out.append(ln)
        elif ln.strip():
            dropped = True
    if dropped:
        out.append("…")
    return "\n".join(out)


def build(raw):
    aliases = {}

    def alias(uuid, author):
        if uuid is None:
            return None
        if uuid == author:
            return "{author}"
        return aliases.setdefault(uuid, "{u%d}" % (len(aliases) + 1))

    prs = []
    for key, d in raw.items():
        repo, pid = key.rsplit("#", 1)
        pr = d["pr"]
        author = (pr.get("author") or {}).get("uuid")
        comments = []
        for c in d["comments"]:
            o = {"id": c["id"], "deleted": bool(c.get("deleted")), "created_on": c.get("created_on"), "updated_on": c.get("updated_on"),
                 "user": {"uuid": alias((c.get("user") or {}).get("uuid"), author)},
                 "content": {"raw": sanitize_raw((c.get("content") or {}).get("raw") or "")}}
            if c.get("parent"):
                o["parent"] = {"id": c["parent"]["id"]}
            if c.get("inline"):
                o["inline"] = {"path": "f"}
            comments.append(o)
        statuses = []
        for s in d["statuses"]:
            rabbit = s["key"].startswith("coderabbitai/")
            statuses.append({"key": s["key"] if rabbit else "pipeline", "name": s.get("name"), "state": s["state"],
                             "description": (s.get("description") or "") if rabbit else "",
                             "created_on": s.get("created_on"), "updated_on": s.get("updated_on"),
                             "commit": {"hash": (s.get("commit") or {}).get("hash")}})
        participants = [{"role": p["role"], "approved": p["approved"], "state": p.get("state"),
                         "user": {"uuid": alias(p["user"]["uuid"], author)}} for p in pr.get("participants", [])]
        prs.append({"key": key, "repo": f"{WS}/{repo}", "id": int(pid), "commentCount": pr.get("comment_count", len(comments)),
                    "commit": ((pr.get("source") or {}).get("commit") or {}).get("hash", "abc123"),
                    "author": "{author}", "participants": participants, "statuses": statuses, "comments": comments})
    return {"prs": prs}


def main(argv):
    if argv[0] == "--from-cache":
        cache = json.load(open(argv[1]))
        raw = {k: {"pr": {"id": v["pr"]["id"]}, "comments": v["comments"], "statuses": v["statuses"]} for k, v in cache.items()}
        out = argv[2]
    else:
        raw, out = fetch(auth_header()), argv[0]
    fixture = build(raw)
    with open(out, "w") as f:
        json.dump(fixture, f, ensure_ascii=False, separators=(",", ":"))
    print(len(fixture["prs"]))


if __name__ == "__main__":
    main(sys.argv[1:])
