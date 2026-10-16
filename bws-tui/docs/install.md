# Install the hush broker

For administrators. `hush install` sets up the root broker daemon on one Linux machine with systemd or one macOS machine. `hush status` reports what is installed. Neither command needs a running daemon.

## Before you run it

1. Stage a trusted hush binary in a root-owned path. The service runs that exact file. `hush install` refuses a binary under a home directory, a Cargo or Homebrew build directory, or any path a non-root user can change. It never copies, downloads, or updates a binary.

   ```bash
   # Linux
   sudo install -o root -g root -m 0755 ~/.cargo/bin/hush /usr/local/bin/hush
   # macOS
   sudo install -o root -g wheel -m 0755 ~/.cargo/bin/hush /usr/local/bin/hush
   ```

   Every directory above the binary must be root-owned and not group or world writable. The file must be a regular file with one hard link, mode bits with no setuid, setgid, sticky, or group or world write, and no ACL grants. Audit the release before you stage it. Root ownership does not prove where the bytes came from.
2. Put `bws` at `/usr/local/bin/bws` or `/usr/bin/bws` under the same rules. `hush install` never provisions `bws`. The daemon checks it again each time it starts.
3. Check the native tools.
   - Linux: systemd 247 or newer, `useradd`, `getent`, `systemd-creds` on systemd 250 or newer, a `nobody` account with a non-root group, `/usr/sbin/nologin`, and a Debian-style (`common-auth` and `common-account`) or `system-auth` PAM layout.
   - macOS: `dscl`, `dsimport`, and `launchctl`.
4. Run the staged binary as root.

   ```bash
   sudo /usr/local/bin/hush install
   ```

## The token

The token is the Bitwarden machine account token.

- **Input.** `hush install` reads it from a hidden terminal prompt, a pipe such as `bws ... | sudo hush install`, or a named pipe or file redirected to stdin. A named pipe or file must belong to the installing user, with no group or other permission bits and no ACL grant. Input whose privacy cannot be proven is refused before it is read. It never takes the token from arguments or the environment. The limit is 64 KiB. Empty input, a NUL byte, and invalid UTF-8 are refused.
- **When.** Input is read only after every check passes and before the first write. A rerun that finds a checked credential reads nothing.
- **Linux, systemd 250 or newer.** The token is encrypted with the systemd host key into `/etc/hush/token.cred`, root-owned 0600, and the unit loads it with `LoadCredentialEncrypted=`. If `/var/lib/systemd/credential.secret` is absent, `hush install` creates it with `systemd-creds setup` as its own step, and only on persistent storage. It never replaces an existing key. If encryption fails, install stops. It never falls back to plaintext. Host-key encryption does not protect a full disk copy that includes the key file.
- **Linux, systemd 247 to 249.** The token is stored in `/etc/hush/token`, root-owned 0600, and the unit loads it with `LoadCredential=`. Older systemd is refused.
- **Linux runtime.** systemd gives the daemon a read-only copy at mode 0400 under `$CREDENTIALS_DIRECTORY`. The daemon accepts 0400 or 0600 there. Stored sources stay 0600.
- **macOS.** The token is stored in `/var/db/hush/token`, root-owned 0600, in a 0700 directory.
- **Old copies.** `hush install` does not remove copies of the token from shell profiles, environment files, or other services. Remove those yourself.

## What install creates

| | Linux | macOS |
| --- | --- | --- |
| Run account | `hushrun`, a system uid chosen by `useradd`, the `nobody` group, home `/nonexistent`, shell `/usr/sbin/nologin`, locked password | `hushrun` from one complete `dsimport` record, the highest free uid from 200 to 499 checked live, the `nobody` group, home `/var/empty`, shell `/usr/bin/false`, hidden, password `*` imported as a crypt value that matches nothing |
| Directories | `/etc/hush` and `/var/log/hush`, root-owned 0700 | `/var/db/hush`, root-owned 0700 |
| Token | see above | see above |
| Password approval | `/etc/pam.d/hush`, 0644, including the distro stack. A protected existing service in `/etc/pam.d` or `/usr/lib/pam.d` is kept as the operator's | OpenDirectory, no file |
| Service | `/etc/systemd/system/hush.service`, 0644, then reload, enable, and start | `/Library/LaunchDaemons/hush.plist`, 0600, label `hush`, then enable if disabled and bootstrap, or start a loaded job that is not running |

The daemon creates its socket directory and its audit log. `hush install` creates neither, and it writes no policy file. An existing `/etc/hush/hushd.toml` must be root-owned, not group or world writable, and valid. Every directory above it must be root-owned and not group or world writable, and neither the file nor those directories may have an ACL grant.

The macOS broker is a root LaunchDaemon. [ADR 0002](../../docs/adr/0002-hush-broker.md) records why it is an exception to the usual managed path for Mac background jobs.

## Refusals and resume

- **Before any change.** `hush install` lists every problem it can see before it reads input or writes anything. Facts it cannot read, or native queries that fail, also refuse. Nothing changes.
- **Never repairs.** It never changes the mode or owner of, overwrites, adopts, or deletes an artifact it did not write exactly. A foreign or edited service definition, a loaded systemd command or run identity that differs from the rendered unit, drop-ins, a mask, other unit names, a listener that is not the managed daemon, a partial or unexpected `hushrun` record, and an unknown PAM layout are refused.
- **During install.** Each step checks its own fact again right before it acts. Before each service manager command, every file install writes, the binaries, and the policy are checked again. Install stops at the first failure or changed fact. Completed steps stay. Nothing is rolled back. Fix the cause and run `hush install` again. It does only the missing steps.
- **Already installed.** A complete, running, reachable install is a no-op. It reads no input and runs no service manager command.
- **After a crash.** A private staging file named `.hush-stage.*` is never reused. If a staging name is the second link of a complete final file, the next run removes that staging name.

## hush status

`hush status` prints six independent lines. It needs no root and no daemon, and it never reads the token.

- **installed.** Every artifact and the account match what install writes. `no` names the first missing or refused item.
- **enabled.** The service starts at boot.
- **active.** The service manager reports a running process.
- **reachable.** A connection to the socket completed, and its peer is root with the service manager's main pid. A live socket alone is not enough. A full queue is `unknown`.
- **credential.** File type, owner, mode, links, and ancestry were checked. The contents were not read.
- **authentication.** Always `not checked`. status never contacts Bitwarden.

A failed query is `unknown`, never a false `no`. A non-root user sees `unknown` for facts only root can read.

## Limits

- No real install has run. Account creation, `systemd-creds`, unit start and the runtime credential path, `dsimport`, and `launchctl` were tested through a fake command boundary on real files, plus `systemd-analyze verify` and `plutil -lint` parse checks. Treat the first real install as an acceptance test on a disposable machine.
- PAM checks cover file metadata only. They do not prove that a custom stack accepts a password.
- `useradd` and `dsimport` are not proven power-loss safe. A crash inside one can leave a partial record. `hush install` then refuses that record and does not repair it.
