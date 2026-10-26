# Hush for iPhone and iPad

A native SwiftUI companion to the [Hush CLI](../bws-tui/). Search Bitwarden Secrets Manager, read and copy a secret, or add one from a phone or tablet. Distribution is through a private, internal TestFlight group.

## Connect

1. Enable a device passcode.
2. Create a Bitwarden Secrets Manager machine account and access token with access to the projects you need. Creating secrets also needs write access.
3. Open Hush, choose the matching US or EU region, and enter the access token. This is a Secrets Manager access token, not a password-manager master password or API key.
4. Complete device authentication. Hush saves the token in this device's Keychain after sign-in succeeds.

The token does not sync through iCloud or device backups. Locking clears the live session. Forgetting the connection removes its saved credential and does not delete anything in Bitwarden.

## Use

Search matches secret keys and project names. Project filters and selections use IDs, so equal names do not merge records. Open a row to fetch its value. Values start hidden. Copy is local to the device and expires after 30 seconds.

Add a secret with a key, value, and project. Notes are optional. A definite rejection keeps the draft. If the server's response is uncertain, Hush does not retry the write. Refresh, inspect matching new records, and explicitly acknowledge the result before another create. A matching name is evidence to inspect, not proof of which request created it.

## Privacy

The catalog holds metadata only. Values and notes are fetched on demand and are not cached to disk. The app covers its content when inactive and locks in the background. Details, drafts, and an app-owned clipboard entry are cleared when locked. The background lock is the exception for the clipboard: a copied value stays so you can paste it in another app, and iOS removes it 30 seconds after the copy. The app does not clear text another app has since copied.

Credentials use `WhenPasscodeSetThisDeviceOnly` and user presence. The Rust session and in-flight work use a serial queue. Lock invalidates the session immediately, so a late response cannot unlock it or restore its contents. This reduces retention but does not promise that Swift strings can be securely erased from process memory.

The privacy manifest declares app-local preferences and file metadata used when reading the bundled notices file. The bridge passes `state_file: None` and does not initialize the SDK database. Hush does not enable Rust backtraces or read an SDK state file. Dormant Rust file and diagnostic symbols do not describe an additional app feature. The file metadata declaration applies to files in the app container, not arbitrary library paths.

## Build and test

Use an Apple silicon Mac with Xcode, an iOS simulator runtime, Rust, Python 3, and XcodeGen. The app supports iOS and iPadOS 17 or later. The bridge currently builds arm64 device and simulator slices. It links Apple's system SQLite to satisfy the SDK's rusqlite dependency. Release verification rejects SQLite imports outside the checked iOS 17 set.

```sh
brew install xcodegen
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
cd hush-ios
scripts/build-bridge.sh
xcodegen generate
scripts/test.sh
```

`project.yml` is the project source. Generated Xcode projects, build output, profiles, certificates, and archives stay out of Git. `test.sh` runs the app's tests on one available iPhone and one available iPad simulator. From the repository root, `bun run check:hush-ios-bridge` runs Rust formatting, tests, and Clippy.

## TestFlight

Use [App Store Connect CLI](https://github.com/rorkai/App-Store-Connect-CLI) for Apple account operations and upload. API authentication and Apple web-session authentication are separate. New app records use `asc web apps create`. Build uploads use an App Store Connect API key. Keep credentials outside the checkout.

Before archiving, install an Apple Distribution identity and an App Store profile for `studio.aoj.hush`. Set these environment variables from the verified signing configuration.

| Variable | Value |
| --- | --- |
| `HUSH_TEAM_ID` | Apple developer team ID |
| `HUSH_PROFILE_NAME` | Installed App Store profile name |
| `HUSH_SIGNING_KEYCHAIN` | Keychain holding the distribution private key |
| `HUSH_BUILD_NUMBER` | Unused positive build number |

```sh
scripts/archive.sh
asc builds upload --app "$ASC_APP_ID" \
  --ipa ".build/export-${HUSH_BUILD_NUMBER}/Hush.ipa" --wait
```

The archive script exports for internal TestFlight only. After Apple processes the upload, attach it to the intended internal group and verify that the build is available to its tester. An uploaded file alone is not a usable TestFlight build. Do not submit an App Store release.

The app contains standard encryption through the Bitwarden SDK as well as transport encryption. This internal TestFlight build is not distributed on the French App Store. Apple requires no documentation for that combination and directs apps without required documentation to declare the exemption in Info.plist. The project sets `ITSAppUsesNonExemptEncryption` to `false` on that basis, not on a claim that it uses only operating-system encryption. Reassess this setting if distribution or cryptography changes. See [Apple's documentation table](https://developer.apple.com/help/app-store-connect/reference/app-information/export-compliance-documentation-for-encryption) and [Info.plist guidance](https://developer.apple.com/help/app-store-connect/manage-app-information/determine-and-upload-app-encryption-documentation).

## Structure and licenses

- `Sources/App` owns SwiftUI screens and navigation.
- `Sources/Core` owns session state, credential storage, clipboard lifetime, and the C boundary adapter. Its [API contract](Sources/Core/API.md) describes the view interface.
- `Bridge` pins the official Bitwarden Rust SDK and exposes four C functions. See the [wire contract](Bridge/contract.md).
- `Tests` covers core behavior and user journeys.

Hush's source license does not replace the SDK or dependency licenses. The Bitwarden SDK has its own [SDK license](https://github.com/bitwarden/sdk-sm/blob/main/LICENSE). This build is for personal use with Bitwarden and internal TestFlight distribution. Bundled dependency notices are in `Resources/ThirdPartyNotices.txt`. Run `python3 scripts/notices.py` after changing `Bridge/Cargo.lock` to refresh them from the locked dependency sources.
