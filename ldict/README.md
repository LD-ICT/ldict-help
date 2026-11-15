# LD ICT Help

LD ICT Help (Dutch: hulp op afstand) is the remote support program of [LD ICT](https://www.ldict.nl): a fork of
[RustDesk](https://github.com/rustdesk/rustdesk) (AGPL-3.0) that only talks to LD ICT's own RustDesk server. The full
plan is `PLAN-HULP-OP-AFSTAND.md` in LD ICT's website repository.

## What is different from RustDesk

Kept as small as possible, so a new RustDesk release is an easy rebase.

- `src/ldict.rs`: the technical name `LDICT-Help` (paths, the `ldict-help://` links, the Windows service) and the name
  people see: "LD ICT Hulp" on a Dutch computer, "LD ICT Help" otherwise, the fixed server `hulp.ldict.nl` with its public key, and the settings a
  customer can't change: no fixed password (only "Accept" at the computer), no direct IP access, no RustDesk account
  or address book, no update check against rustdesk.com, and the server, proxy and websocket settings hidden.
- Small hooks: `src/common.rs` (`load_custom_client` and `read_custom_client` call `ldict::apply()`), `src/lang.rs`
  (translations say the display name instead of "RustDesk"), `src/flutter_ffi.rs` (`main_get_app_name_sync`, the
  window titles) and the module line in `src/lib.rs`.
- `src/ldict/token.rs` and `src/ldict/report.rs`: connection tokens and reports. A connection needs a short-lived
  token signed by LD ICT's admin (Ed25519, public key in `ldict/token.pub`); the technician's program gets it from an
  `ldict-help://connection/new/<id>?password=ldict1....` link and sends it in a new login request field
  (`ldict_token`, `libs/base/protos/message.proto`). The customer's program refuses every login without a valid token
  for its own id, also with a password. It starts without "Accept" only when both the token and the device's setting
  at www.ldict.nl allow unattended access. It reports each session's start, a heartbeat every minute and the end to
  `/api/hulp/sessies/melding`, and its version and name to `/api/hulp/apparaten/status` every hour.
- More hooks for that: `src/client.rs` (`handle_hash` keeps the token out of the password, `create_login_msg` sends
  it), `src/server/connection.rs` (admit, unattended, started, every second, closed) and `src/server.rs` (the hourly
  status report).
- `ldict/icons/` and `ldict/brand.sh`: the LD ICT icons, copied over RustDesk's.
- `ldict/key.pub`: the public key of the server, and `ldict/token.pub`: LD ICT's public key for connection tokens
  (copy both from `/admin/servers/hulp` in the admin).
- `.github/workflows/ldict-release.yml`: a release build on a tag like `ldict-1.5.3-2`, or started by hand with that
  tag. The number after the last dash must match `BUILD` in `src/ldict.rs`.

Known: the Linux package is still called `rustdesk` and installs to RustDesk's paths, so it replaces a normal RustDesk
on that computer. Renaming it means changing RustDesk's packaging throughout; not worth it while few customers use
Linux.

## Updating to a new RustDesk release

```sh
git fetch upstream --tags
git rebase <new RustDesk tag> ldict    # conflicts, if any, are in the hooks above
./ldict/brand.sh                      # icons again, in case RustDesk changed theirs
git push --force-with-lease origin ldict
git tag ldict-<version>-1 && git push origin ldict-<version>-1
```

## License

AGPL-3.0, like RustDesk. The source of every version LD ICT distributes is in this repository.
