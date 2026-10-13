# 0002. Hush broker: agents use secrets without seeing them

Status: accepted (2026-10-13). Built as four stacked PRs.

## Problem

Agents run as the same OS user as the human. Today `BWS_ACCESS_TOKEN` and about 63 real values sit in `~/.env`, every shell, and every service. So any agent can run `env`, `cat ~/.env`, `bws secret list`, `hush get`, or `hush exec -- sh -c 'echo $X'` and see values. A friendlier CLI cannot fix that, because the agent can go around it.

Goal: any agent (any harness, or a plain script) can use a secret but never sees its value. The model is 1Password: the CLI holds no key, and `run` puts values into a child process and masks them in the output.

Accepted limit: a command that deliberately prints its own secret in a transformed form (for example base64) can leak that one value. Masking is best effort.

## Decision

Sol's broker design is the base. Wonder Full's child-run design replaces Sol's spawn step. Signed command grants from Wonder Full are not part of this decision.

### Process split

- `hush daemon` (a hidden subcommand of the same binary) is the broker. It runs as root. It is the only process that holds the BWS token or any secret value.
- `hush` is the client. It holds no token and never receives secret bytes for `run`.
- They talk over one local Unix socket. No TCP. The daemon reads the caller's uid, gid, and pid from the kernel (`SO_PEERCRED` on Linux, `getpeereid` and `LOCAL_PEERPID` on macOS). Policy uses those values, never a value the client claims.
- Socket paths: `/run/hush/hush.sock` (Linux), `/var/run/hush/hush.sock` (macOS). `HUSH_SOCKET` overrides the path for tests and same-user development only.

### Token custody

- Linux: root systemd service. Token is a systemd credential (`LoadCredentialEncrypted=` where systemd is 250 or newer, otherwise a root-only `LoadCredential=` file).
- macOS: root LaunchDaemon. Token is a root-only file `/var/db/hush/token`, mode 0600, directory 0700. (Sol proposed a System Keychain item. A root-only file gives the same protection against a non-root agent and does not need a signed binary. Keychain can come later.)
- The daemon refuses to start if the token file is not owned by root with mode 0600.
- The daemon runs `bws` by absolute path with a cleared environment plus `BWS_ACCESS_TOKEN`. The `bws` child runs as root, so non-root processes cannot read its argv or env through `/proc` or `ps -E`.

### Client surface

The client is one small type. No public type contains a secret value.

```rust
pub struct Client { /* private transport */ }

impl Client {
    pub fn connect() -> Result<Self>;
    pub fn catalog(&mut self, project: Option<&str>) -> Result<Vec<SecretMeta>>;
    pub fn run(&mut self, spec: RunSpec, sink: &mut dyn OutputSink) -> Result<RunOutcome>;
    pub fn reveal_to_tty(&mut self, secret: SecretRef, tty: BorrowedFd<'_>) -> Result<()>;
}
```

CLI usage:

```bash
hush list [--json]                                   # metadata only
hush run --secret CLOUDFLARE_API_TOKEN -- wrangler deploy
hush run --secret "Cloudflare Admin=CF_ADMIN" --fd GH_TOKEN -- ./deploy.sh
hush reveal --key CLOUDFLARE_API_TOKEN               # human only, real TTY, approval
```

`hush get` is removed. `hush exec` stays one release as an alias of `hush run` with the same code path.

### Child run (from Wonder Full)

The daemon starts the child. It never uses process replacement, and it never runs the child as root.

1. Resolve each `--secret` / `--fd` reference to one secret id through the daemon's catalog. Audit before any fetch.
2. Fetch each value inside the daemon.
3. Pick the run identity:
   - `RunAs::Requester` (default): the caller's own uid and gid, from the socket. The child can use the caller's files.
   - `RunAs::HushRun`: a dedicated `hushrun` system user. The daemon policy file (`/etc/hush/hushd.toml`) can require this for named high-value keys. Then a sibling process of the agent cannot read the child's environment.
   - Root is never a run identity. A request that resolves to uid 0 is refused.
4. Build the child environment from scratch: the bindings, plus a short allowlist copied from the client request (`PATH`, `HOME`, `USER`, `LOGNAME`, `LANG`, `LC_*`, `TERM`, `TZ`, `TMPDIR`). Strip anything named `BWS_*`, `LD_*`, or `DYLD_*`.
5. Delivery per binding:
   - `--secret NAME[=ENV]`: the value goes in the child's environment.
   - `--fd NAME[=ENV]`: the value goes through a pipe. The child gets `ENV_FD=<n>` and reads the value from that fd. For hush-aware tools, this keeps the value out of `/proc/<pid>/environ`.
6. Drop to the run identity with `setgroups`, `setgid`, then `setuid` before exec. Close every inherited fd except stdin, stdout, stderr, and the delivery pipes. Set the working directory from the client request and refuse if it does not exist.
7. Capture stdout and stderr through pipes. Forward stdin and signals (INT, TERM, HUP, QUIT) from the client. Mask both streams. Send masked frames and the exit status back.
8. Zero every secret buffer when the child exits. If the client disconnects, kill the child's process group.

### Masking (Sol)

A streaming byte masker per stream. It holds back `longest_secret_len - 1` bytes between chunks, so a value split across two reads is still caught. It replaces exact secret byte sequences with `<hidden by hush>`. Values shorter than 8 bytes are not masked, and `hush run` warns once on stderr naming the key. No encoded variants in this decision.

### Types

```rust
struct SecretMaterial(secrecy::SecretBox<[u8]>); // daemon only. No Clone, Debug, Serialize, or public constructor.
trait VaultBackend { fn catalog(&self) -> Result<Vec<SecretMeta>>; fn fetch(&self, r: &SecretRef) -> Result<SecretMaterial>; }
enum RunAs { Requester, HushRun }
enum Delivery { Env, Fd }
struct Binding { secret: SecretRef, env: EnvName, delivery: Delivery }
struct RunSpec { bindings: Vec<Binding>, program: OsString, args: Vec<OsString>, cwd: PathBuf, env_passthrough: Vec<(EnvName, OsString)> }
```

`SecretMaterial` lives only in daemon modules. The wire protocol module is private, and no frame type can hold `SecretMaterial` (it does not implement `Serialize`). A test proves this.

### Human reveal

`hush reveal` requires a real terminal. The client passes its TTY fd to the daemon over the socket (`SCM_RIGHTS`). The daemon asks the human to approve on that TTY, then writes the value straight to it. The value never crosses the socket as a frame and never reaches the client's stdout. A pipe such as `hush reveal ... | cat` is refused.

Approval checks the account password of the socket's peer uid: PAM on Linux, OpenDirectory on macOS. Agents do not know the password. Touch ID can replace this later. Edit, delete, and TUI copy go through the same approval.

### Ownership

- The daemon owns the token, the catalog, the audit log, and every secret value.
- The client owns the terminal, stdin and stdout forwarding, and TUI state.
- BWS owns stored values. The kernel owns peer identity.

## Build plan: four stacked PRs

1. `hush-broker/1-core`: this ADR, the private wire protocol, `hush daemon` socket server with peer credentials, `VaultBackend` with a fake backend and the real `bws` backend, the daemon-owned metadata catalog, `Client::catalog`, and `hush list` through the broker when a daemon is reachable. Existing commands keep working.
2. `hush-broker/2-run`: daemon-owned `run` with the child-run steps above, the streaming masker, `hush run`, and `exec` as an alias.
3. `hush-broker/3-human`: `hush reveal` with TTY fd passing and password approval, approval for edit, delete, and copy, a metadata-first TUI, and removal of `get`, the old `exec` code, value-bearing public structs, and the user-owned cache.
4. `hush-broker/4-install`: `hush install` and `hush status` for Linux (systemd unit, credential, `hushrun` user, socket dir) and macOS (LaunchDaemon, token file, `hushrun` user), plus docs. Installing on a machine stays a human step with `sudo`.

Outside this stack: EnvSync stops writing secret values, shells stop sourcing them, services move to `hush run`, and each machine gets the daemon. Those changes live in other repos and on the machines.

## Alternatives considered

- Opus: a separate-user daemon that hands values to the client, which then starts the command. It lost because the agent's own process received raw values.
- Wonder Full in full: every command needs a human-signed grant. It lost because unattended agents would need a human for each new command.
- Harness sandbox features: not agent-agnostic.

## Open questions

- A root LaunchDaemon on macOS conflicts with the rule that Mac background jobs ship through `obi bgjob deploy`. PR 4 documents it as an exception unless `obi bgjob` grows LaunchDaemon support.
- PAM headers (`libpam0g-dev`) must exist on the Linux build machine and CI runner for PR 3.
