#!/usr/bin/env python3
"""End-to-end probe: a real approval request, rendered and decided in the real client.

Drives a running Robrix through makepad's `--remote` HTTP control surface (a
localhost bridge baked into every makepad app) against a real Matrix homeserver.
Input is injected via `Cx::dispatch_studio_msg`, the same path studio uses, so
hit-testing and capture behave exactly as they do for a human click.

    # 1. start the app with the bridge on a fixed port
    MAKEPAD_REMOTE=8099 ./target/debug/robrix <user> <password> <homeserver>
    # 2. with a pending request already in the room:
    python3 probe_approval.py --bridge 8099 --homeserver http://127.0.0.1:8128 \
        --room '!room:server' --request-id approval_<32hex> --token <reader-token> \
        --user soak_owner

Why HTTP and not the headless stdin protocol: `MAKEPAD=headless` does not build
on macOS at makepad rev 493d23a — `os/cx_shared.rs` calls
`crate::os::apple::metal::note_input_event()` under `#[cfg(target_vendor =
"apple")]` while `os/mod.rs` gates `pub mod apple;` behind `not(headless)`, so
the call survives and the module does not (E0433). The remote bridge needs no
headless build at all.

Note `MAKEPAD_REMOTE=1` means *port 1*, not "enabled" — pass a real port.

Exit status is 0 only if every check passes.
"""

import argparse
import json
import sys
import time
import urllib.parse
import urllib.request

results = []


def check(name, ok, detail=''):
    results.append((name, bool(ok)))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}" + (f"  -- {detail}" if detail else ''), flush=True)
    return bool(ok)


class Bridge:
    """makepad's `--remote` control surface. Every answer is one line of JSON."""

    def __init__(self, port):
        self.base = f'http://127.0.0.1:{port}'

    def get(self, path):
        return json.load(urllib.request.urlopen(self.base + path, timeout=20))

    def snap(self, query=None, every=False):
        """Widget rects ready to click: {"s":[{"i":id,"ty":type,"r":[x,y,w,h],"t":text}]}."""
        params = {}
        if query:
            params['q'] = query
        if every:
            params['all'] = '1'
        return self.get('/snap?' + urllib.parse.urlencode(params)).get('s', [])

    def click(self, x, y, settle=1.0):
        # wait=1 answers only after the next frame is drawn.
        self.get(f'/click?x={x:.0f}&y={y:.0f}&wait=1')
        time.sleep(settle)

    def grab(self):
        return self.get('/g').get('png')

    def grab_and_quit(self):
        """The bridge's contract: a probe that launched the app must end here."""
        return self.get('/gq')


def texts(widgets):
    return [(w.get('t') or '').strip() for w in widgets]


def find(widgets, needle, exact=False):
    out = []
    for w in widgets:
        t = (w.get('t') or '').strip()
        if (t == needle) if exact else (needle.lower() in t.lower()):
            out.append(w)
    return out


def center(w):
    x, y, width, height = w['r']
    return x + width / 2.0, y + height / 2.0


def verdicts_on_server(hs, token, room, request_id):
    """Verdict events the homeserver actually holds for this request."""
    url = f"{hs}/_matrix/client/v3/rooms/{urllib.parse.quote(room)}/messages?dir=b&limit=40"
    req = urllib.request.Request(url, headers={'authorization': f'Bearer {token}'})
    chunk = json.load(urllib.request.urlopen(req, timeout=15))['chunk']
    return [
        e for e in chunk
        if e.get('content', {}).get('msgtype') == 'com.agentchat.approval.verdict.v1'
        and e['content'].get('com.agentchat.approval', {}).get('request_id') == request_id
    ]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--bridge', default='8099')
    ap.add_argument('--homeserver', required=True)
    ap.add_argument('--room', required=True)
    ap.add_argument('--request-id', required=True)
    ap.add_argument('--token', required=True, help='access token able to read the room')
    ap.add_argument('--user', required=True, help='localpart of the logged-in owner')
    ap.add_argument('--room-name', default='Approval')
    a = ap.parse_args()

    b = Bridge(a.bridge)
    server = lambda: verdicts_on_server(a.homeserver, a.token, a.room, a.request_id)

    check('no verdict for this request before the run', not server())

    rows = b.snap(a.room_name)
    if not check('client synced and listed the approval room', bool(rows)):
        return finish(b)

    leaked = [t for t in texts(b.snap(every=True)) if 'CustomMessageContent' in t]
    check('rooms list shows no raw CustomMessageContent debug text', not leaked,
          leaked[0][:70] if leaked else '')
    check('rooms-list preview shows the human-readable body',
          any('asking for approval' in t for t in texts(rows)))

    x, y = center(max(rows, key=lambda w: w['r'][2] * w['r'][3]))
    b.click(x, y, settle=2.5)

    widgets = b.snap(every=True)
    labels = texts(widgets)
    buttons = {}
    for name in ('Approve once', 'Allow for this task', 'Always allow this operation', 'Deny'):
        hit = find(widgets, name, exact=True)
        if hit:
            buttons[name] = hit[0]
    check('approval card shows all four bridge-supplied buttons', len(buttons) == 4,
          f'found {sorted(buttons)}')
    check('card shows the tool and runtime', any('Bash' in t for t in labels))
    check('card shows Pending', any(t == 'Pending' for t in labels))
    check('card shows the command preview', any('cargo test --lib' in t for t in labels))
    check('card shows the "text replies are not approval" hint',
          any('not approval' in t.lower() for t in labels))
    print('window grab:', b.grab(), flush=True)

    if 'Approve once' not in buttons:
        return finish(b)
    bx, by = center(buttons['Approve once'])
    b.click(bx, by, settle=2.0)

    after = texts(b.snap(every=True))
    check('card records the decision',
          any('✓' in t for t in after) or any(t in ('Decided', 'Sending…') for t in after),
          str([t for t in after if '✓' in t or t in ('Decided', 'Sending…', 'Pending')])[:100])

    found = []
    for _ in range(20):
        found = server()
        if found:
            break
        time.sleep(2)
    if check('a real verdict event reached the homeserver', bool(found)):
        v = found[0]
        payload = v['content']['com.agentchat.approval']
        check('verdict sender is the logged-in owner',
              v['sender'].startswith(f'@{a.user}:'), v['sender'])
        check('verdict echoes the request_id', payload.get('request_id') == a.request_id)
        check('verdict action is approve_once', payload.get('action') == 'approve_once')
        check('verdict replies to the request event',
              'm.in_reply_to' in v['content'].get('m.relates_to', {}))
    print('post-click grab:', b.grab(), flush=True)
    return finish(b)


def finish(b):
    passed = sum(1 for _, ok in results if ok)
    print(f'\nSUMMARY: {passed}/{len(results)} checks passed', flush=True)
    try:
        b.grab_and_quit()  # never leave a test window on the user's screen
    except Exception:
        pass
    return 0 if passed == len(results) else 1


if __name__ == '__main__':
    sys.exit(main())
