# Licence text provenance

Retrieved on 2026-10-23; these texts are redistributed unmodified. The versioned dependency inventories and `THIRD_PARTY_NOTICES.md` identify the libraries to which they apply.

- `Apache-2.0.txt`: https://www.apache.org/licenses/LICENSE-2.0.txt
- `SSHJ.txt`: https://raw.githubusercontent.com/hierynomus/sshj/v0.40.0/LICENSE
- `Bouncy-Castle.txt`: https://raw.githubusercontent.com/bcgit/bc-java/r1rv86/LICENSE.html (HTML text)
- `SLF4J.txt`: https://raw.githubusercontent.com/qos-ch/slf4j/v_2.0.17/LICENSE.txt
- `JediTerm-LGPLv3.txt`: https://raw.githubusercontent.com/JetBrains/jediterm/master/LICENSE-LGPLv3.txt
- `GPL-3.0-or-later.txt`: https://raw.githubusercontent.com/spdx/license-list-data/main/text/GPL-3.0-or-later.txt
- `CC0-1.0.txt`: https://raw.githubusercontent.com/spdx/license-list-data/main/text/CC0-1.0.txt

The JediTerm source JARs for **3.59** must be supplied to the reference rebuild recipe and checked against the source artifact hashes in `gradle/verification-metadata.xml`. The licence text from upstream's current tree does not assert that the current upstream dual-licence declaration applies retroactively to 3.59; the notices conservatively retain the 3.59 POM's LGPL declaration.

The xterm.js notice is copied directly from the npm integrity-verified package into Android assets by `web/bundle.mjs`. New project code is not assigned a licence by these third-party notices.

## Selected runtime inventory

`dependencies.json` and `native-components.json` link exact versions, artifact hashes,
POM/parent-POM license declarations, embedded NOTICE/LICENSE files and pinned native
source revisions. Evidence bytes are stored unmodified under `evidence/`, addressed
by SHA-256. Apache-2.0 is selected for dual-licensed JNA. License text hashes are
checked independently from dependency artifacts. No unknown declaration is accepted.

Run `python3 scripts/verify_licenses.py` after `./gradlew writeDependencyInventory`.
`refresh_licenses.py` is an explicit maintenance command, never a CI auto-approval
step. Review its generated changes and upstream evidence before updating the registry.
Source/archive notices do not assign a license to new application code.

`jediterm-rebuild/` supplies the verified replacement build recipe, compiler metadata
and dependency locks. Its generated caches and binaries must never be redistributed
as license materials. The current Rust apps do not package a JVM or JediTerm; removed JVM packaging procedures are summarized in `docs/HISTORY.md`.
