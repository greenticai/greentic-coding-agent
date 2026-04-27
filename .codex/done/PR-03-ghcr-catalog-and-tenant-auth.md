# PR-03 — Add GHCR catalog sync with tenant/private authentication

## Goal

Support public and tenant-private index catalogs hosted in GHCR.

The coding agent must be able to download:

- the public Greentic catalog
- optional tenant catalogs
- public repo indexes
- private tenant repo indexes

## Depends on

- PR-01 repo identity migration.
- Existing local OCI simulation must be kept as a deterministic fixture backend or explicitly retired.

## CLI requirements

Add flags to relevant commands:

```bash
--catalog <oci-ref>
--tenant <name>
--tenant-catalog <oci-ref>
--token <token>
--token-env <ENV_NAME>
--strict
--public-only
--private-only
--include-private
```

Examples:

```bash
greentic-coding-agent sync

greentic-coding-agent sync   --tenant meeza   --token $GHCR_TOKEN

greentic-coding-agent sync   --catalog ghcr.io/greenticai/indexes/catalog:latest   --tenant meeza   --tenant-catalog ghcr.io/greenticai/indexes/tenants/meeza/catalog:latest   --token-env MEEZA_GHCR_TOKEN
```

## Environment variables

Support:

```text
GREENTIC_AGENT_CATALOG
GREENTIC_AGENT_TENANT
GREENTIC_AGENT_TENANT_CATALOG
GREENTIC_AGENT_TOKEN
GHCR_TOKEN
```

Precedence:

1. CLI flags
2. Greentic-specific env vars
3. `GHCR_TOKEN`
4. default public catalog

## Default catalogs

Public:

```text
ghcr.io/greenticai/indexes/catalog:latest
```

Tenant:

```text
ghcr.io/greenticai/indexes/tenants/<tenant>/catalog:latest
```

## OCI client

MVP production transport: use `oras` as a subprocess.

Do not bake ORAS directly into all sync/publish code paths. Add a backend abstraction so tests and local development can keep using the current local-store simulation.

```rust
pub enum RemoteBackendKind {
    LocalFixture,
    GhcrOras,
}

pub trait RemoteIndexBackend {
    fn pull(&self, reference: &str, out_dir: &Path, auth: Option<&RegistryAuth>) -> Result<()>;
    fn push(&self, reference: &str, dir: &Path, auth: Option<&RegistryAuth>) -> Result<()>;
}
```

Use:

- `LocalFixtureBackend` for tests and offline development.
- `GhcrOrasBackend` for real GHCR transport.

Add module:

```text
crates/gca-oci/src/oras.rs
```

Functions:

```rust
pub fn oras_pull(ref: &str, out_dir: &Path, auth: Option<&RegistryAuth>) -> Result<()>;
pub fn oras_push(ref: &str, dir: &Path, auth: Option<&RegistryAuth>) -> Result<()>;
pub fn oras_login(registry: &str, username: &str, token: &str) -> Result<()>;
```

Do not print tokens.

If `oras` is missing, return a helpful error:

```text
oras is required for GHCR sync. Install it with: brew install oras
```

## Auth model

Add:

```rust
pub struct RegistryAuth {
    pub registry: String,
    pub username: Option<String>,
    pub token: String,
}
```

For GitHub Actions, username can be `${{ github.actor }}`.

For local use, username can default to `greentic-agent`.

## Auth/config resolution

Resolve all catalog, tenant, token, and backend settings in one place. Sync, serve, catalog publish, watcher, and workflow generation should not each implement their own precedence logic.

Add:

```rust
pub struct RemoteConfig {
    pub backend: RemoteBackendKind,
    pub public_catalog_ref: String,
    pub tenant: Option<String>,
    pub tenant_catalog_ref: Option<String>,
    pub auth: Option<RegistryAuth>,
    pub strict: bool,
}
```

Test precedence once in the config resolver:

1. CLI flags
2. Greentic-specific env vars
3. `GHCR_TOKEN`
4. default public catalog

## Catalog merge

When tenant is provided:

```text
merged_catalog = public_catalog + tenant_catalog
```

Rules:

- `repo_id` is the merge key.
- Tenant catalog overrides public entry with same `repo_id`.
- Disabled entries are retained in catalog but skipped by sync.
- Missing private indexes are warnings unless `--strict`.

## Tests

- Resolve default public catalog.
- Resolve tenant catalog from `--tenant`.
- Token from `--token-env`.
- Token redaction in all `Debug`, status, error, and log output.
- Local fixture backend pull/push still works without network.
- GHCR backend returns a helpful error when `oras` is missing.
- Tenant override of public repo.
- Disabled repo is skipped.
- Private repo without token errors only with `--strict`.

## Acceptance criteria

- `sync` can fetch public catalog without tenant.
- `sync --tenant <name> --token <token>` includes tenant catalog.
- No token is logged.
- Catalog entries keep `repo_id = org/repo`.
