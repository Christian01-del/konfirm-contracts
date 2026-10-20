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

## Third-party GitHub App access

This section records the deliberate, reviewed decisions about third-party GitHub Apps installed on the `Konfirm-xyz` organization, so that org-wide grants are a recorded choice rather than silent default scope.

### Drips Wave (`drips-wave`)

- **App:** Drips Wave (`app_slug: drips-wave`, `app_id: 2617547`), a legitimate OSS contribution/bounty platform (drips.network). It is **not** suspected-malicious.
- **Installation:** Installed on the `Konfirm-xyz` organization with `repository_selection: "all"`.
- **Permissions:** `members:read`, `issues:write`, `pull_requests:write`, `metadata:read`.
- **Decision:** The org-wide installation is intentional. Wave is used across the organization's repositories to manage contribution and bounty workflows, so it is granted access to all repositories rather than a per-repo selection. This includes this funds-custody contracts repository.
- **Review:** This grant is reviewed periodically (at least every 6 months, and whenever the app's permissions or repository selection change) to confirm it is still needed and still scoped as intended. If Wave stops being used org-wide, its repository access should be narrowed to only the repositories that actively use it, excluding this and any other contract/funds repositories.
