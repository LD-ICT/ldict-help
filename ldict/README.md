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
- `ldict/icons/` and `ldict/brand.sh`: the LD ICT icons, copied over RustDesk's.
- `ldict/key.pub`: the public key of the server (copy it from `/admin/servers/hulp` in the admin).
- `.github/workflows/ldict-release.yml`: a release build on a tag like `ldict-1.5.3-1`.

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
