#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
command -v xcodegen >/dev/null
if [[ ! -d .build/HushBridge.xcframework ]]; then
  scripts/build-bridge.sh
fi
xcodegen generate
mkdir -p .build/results
for family in iPhone iPad; do
  destination=$(xcrun simctl list devices available -j | python3 -c '
import json,re,sys
family=sys.argv[1]
runtimes=json.load(sys.stdin)["devices"]
candidates=[]
for runtime, devices in runtimes.items():
    match=re.search(r"iOS-(\d+)-(\d+)(?:-(\d+))?$", runtime)
    if not match: continue
    version=tuple(int(part or 0) for part in match.groups())
    if version[0] < 17: continue
    candidates.extend((version, d["udid"]) for d in devices if d["name"].startswith(family))
if not candidates: raise SystemExit("No available "+family+" simulator")
print(max(candidates)[1])
' "$family")
  result=".build/results/${family}-$(date -u +%Y%m%dT%H%M%SZ).xcresult"
  xcodebuild test -project Hush.xcodeproj -scheme Hush     -destination "platform=iOS Simulator,id=$destination"     -derivedDataPath "${HUSH_DERIVED_DATA:-.build/DerivedData}" -resultBundlePath "$result"     CODE_SIGN_IDENTITY='-' CODE_SIGN_STYLE=Automatic
 done
