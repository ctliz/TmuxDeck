# Code signing decision (macOS and Windows)

TmuxDeck ships unsigned. This document records why.

## Status

- Releases are built and published by GitHub Actions (`release.yml`).
- Artifacts are not signed or notarized on either platform.
- The workflow contains an optional Azure Trusted Signing hook for Windows, guarded by `if:` conditions. It is dormant and requires no action.

## Why unsigned

- **macOS.** Notarization requires an Apple Developer account ($99/year). That is a business decision, not a technical one; for an open-source project with no revenue it is not justified. Users who hit the "cannot verify developer" warning follow the right-click -> Open flow, which is documented in the README and is common practice for open-source macOS apps.
- **Windows.** Azure Trusted Signing is a paid, per-use service with no free tier. Real cost would be a few cents per month, but it requires binding a credit card to an Azure subscription permanently. That was weighed against the fact that SmartScreen warnings are a known, tolerated hurdle for open-source Windows apps. Decision: do not bind a card.

## If signing is revisited

The `release.yml` Windows job already has the plumbing:

```
- Azure login (OIDC), runs only if AZURE_CLIENT_ID is set
- Sign artifacts (Azure Trusted Signing), runs only if AZURE_TRUSTED_SIGNING_ENDPOINT is set
```

To enable, set the six `AZURE_*` secrets (see the workflow) and create the Trusted Signing account, profile, and OIDC federation in Azure. The steps are not documented here because the decision is currently "no"; the workflow comments and the historical git history contain the details.

## Related

- README's FAQ covers the macOS first-launch warning.
- Opening a signed build would change nothing about the product itself; this is purely about install-time trust UI.
