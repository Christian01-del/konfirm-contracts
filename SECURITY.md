# Security Policy

This repository holds the Soroban contracts that sit on Konfirm's payment and settlement path. If you find a security issue, please report it privately rather than opening a public issue or pull request.

## Reporting a vulnerability

Use GitHub's private vulnerability reporting: open the **Security** tab of this repository and choose **Report a vulnerability**. Include the affected contract, the steps to reproduce, and the impact you expect. Please do not disclose the issue publicly until we have had a chance to respond.

## Scope

In scope:

- Authorization and access control in the `compliance`, `payment`, `treasury`, and `channel` contracts (admin checks, multisig signer and threshold logic, pause handling, `initialize` guards).
- Fund safety: escrow handling and refunds in `channel`, settlement execution in `treasury`, and double-spend or double-refund paths in `payment`.
- Signature verification and replay protection, including the `KONFIRM_CHAN_V1` domain separation and nonce checks in `channel`.
- Key handling for the deployer and treasury signer keys. The operational key procedures are documented in `konfirm-backend/docs/KEYS.md`.

Out of scope:

- Issues that require control of the admin, deployer, or treasury signer keys without a flaw in the contract logic.
- Findings against Stellar or Soroban itself; report those to the upstream projects.
- Deployment parameter choices, such as the challenge period value, which are documented as operational decisions rather than contract guarantees.

## Response

We aim to acknowledge reports within 5 business days. This is a small, actively developed project, so response and fix times may vary. We will keep you informed of progress and credit reporters who want to be credited.

## Repository Actions permissions

The repository-level GitHub Actions gate must not allow arbitrary third-party actions to run. Maintainers should configure the repository's Actions permissions (Settings → Actions → General, or via the API) as follows:

- Set `allowed_actions` to `selected` rather than `all`, with an explicit allowlist of the actions actually in use:
  - `actions/checkout`
  - `dtolnay/rust-toolchain`
  - `rustsec/audit-check`
- Enable `sha_pinning_required` once the workflow's action references are pinned to full commit SHAs (tracked separately).

This is a platform-level setting and cannot be enforced from files in the repository; it must be applied through the repository settings or the API. Verify with:

```
gh api repos/Konfirm-xyz/konfirm-contracts/actions/permissions
```

`allowed_actions` should report `selected` and, once pinning is complete, `sha_pinning_required` should report `true`.
