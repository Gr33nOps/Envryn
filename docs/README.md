# Envryn documentation

This is the best starting point if you want to understand how Envryn works, how it is tested, or how to prepare a release.

## For users

- [Update policy](UPDATE_POLICY.md): safe upgrades, backups, and rollback guidance
- [AI data access](AI_DATA_ACCESS.md): exactly what the optional local model can receive
- [Security and privacy testing](SECURITY_TESTING.md): checks you can run before trusting a build

## For contributors

- [Architecture](ARCHITECTURE.md): component boundaries and data flow
- [Quality testing](QUALITY_TESTING.md): unit, integration, browser, mobile, and accessibility coverage
- [Dependency policy](DEPENDENCY_POLICY.md): how dependencies are selected and reviewed
- [Release process](RELEASE_PROCESS.md): versioning, validation, packaging, and publishing

## Security design

- [Threat model](THREAT_MODEL.md)
- [Cryptography](CRYPTOGRAPHY.md)
- [Security invariants](SECURITY_INVARIANTS.md)
- [AI security](AI_SECURITY.md)
- [AI data access](AI_DATA_ACCESS.md)

The historical [audit report](../AUDIT_REPORT.md) and [remediation report](../SECURITY_REMEDIATION_REPORT.md) record how the current controls were developed. When those reports conflict with current code or CI, the current guides and repository configuration are authoritative.

## Project operations

- [Contributing](../CONTRIBUTING.md)
- [Support](../SUPPORT.md)
- [Security policy](../.github/SECURITY.md)
- [Release signing](../RELEASE_SIGNING.md)
- [Beta release checklist](../BETA_RELEASE_CHECKLIST.md)

If a document is wrong or unclear, please open a documentation issue. Good documentation is part of the product, not an afterthought.
