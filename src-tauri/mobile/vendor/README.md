# Vendored third-party code (mobile SPA)

This directory is a **provenance record only**. It deliberately contains no
JavaScript: the library code lives in exactly one place, inlined into
`mobile/index.html`. Keeping a second copy here would mean two artifacts that
can silently drift apart, so only the paperwork is kept.

**These are not npm dependencies of this project.** The mobile SPA is a single
HTML file served by `src-tauri/src/connection.rs` via `include_str!`, and the
embedded HTTP server answers only `/v1/` — every other path returns 404. No
route can serve `vendor/*.js`, and none should be added for this, so the code is
inlined instead of fetched.

## Where the code actually lives

In `mobile/index.html`, each library sits between explicit markers carrying its
name, version and SPDX identifier:

```
<!-- BEGIN VENDOR: marked 18.0.9 -- SPDX-License-Identifier: MIT -->
<!-- END VENDOR: marked 18.0.9 -->
<!-- BEGIN VENDOR: DOMPurify 3.4.13 -- SPDX-License-Identifier: Apache-2.0 -->
<!-- END VENDOR: DOMPurify 3.4.13 -->
```

The inlined bytes are the upstream files verbatim, including their own license
headers. Full license texts are kept here rather than inlined into the HTML.

## Contents

| Inlined file | Package | Version | License | Source |
| --- | --- | --- | --- | --- |
| `package/lib/marked.umd.js` | [marked](https://github.com/markedjs/marked) | 18.0.9 | MIT (`marked.LICENSE`) | `https://registry.npmjs.org/marked/-/marked-18.0.9.tgz` |
| `package/dist/purify.min.js` | [DOMPurify](https://github.com/cure53/DOMPurify) | 3.4.13 | Apache-2.0 (`dompurify.LICENSE-APACHE`) | `https://registry.npmjs.org/dompurify/-/dompurify-3.4.13.tgz` |

DOMPurify is dual-licensed "Apache-2.0 OR MPL-2.0". This project takes the
**Apache-2.0** option, and only that license text is vendored.

## Integrity

npm tarball integrity, as published by the registry and verified locally with
`openssl dgst -sha512 -binary <tarball> | openssl base64 -A`:

- `marked-18.0.9.tgz`
  `sha512-/Sa4qiiHZxf0/FQdBBowr9q4r10krCwMvpK48FUBdXdUXScDxiQGR9zCPrFgRVR5LU3iySOiIjy09ZQvADir1w==`
- `dompurify-3.4.13.tgz`
  `sha512-2vmYIoqjze2d+kakP8S/nS5shfsl587kzwEjcGlTdiksUVgFHnFCsLYDVj/JNqJVOQZGSYBTmuycv0PodwmnMQ==`

SHA-256 of the extracted files, as inlined into `mobile/index.html`:

- `marked.umd.js` `ba65f1c8948e6b01321399800843e9048b31e1c197652d4b0fafae840b30e32b`
- `purify.min.js` `9ab3d44d73c3e3947f9ab72e0f0bc15c7f1931d60b365ba261fc85fe59013c56`

To re-verify what ships, extract the bytes between the markers in
`mobile/index.html` and compare against these hashes.

## Re-vendoring

```sh
npm pack marked@<version> dompurify@<version>
tar -xzf marked-<version>.tgz && tar -xzf dompurify-<version>.tgz
# Replace the code between the BEGIN/END VENDOR markers in mobile/index.html
# with package/lib/marked.umd.js and package/dist/purify.min.js, update the
# versions in the markers, and refresh the hashes above. Do not copy the .js
# files into this directory.
```

Neither file contains the byte sequence `</script`, which is what makes inlining
them safe; re-check that after any upgrade.
