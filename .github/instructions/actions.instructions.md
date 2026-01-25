---
applyTo: ".github/workflows/*.yml"
description: "GitHub Actions CI/CD best practices"
---

# GitHub Actions Best Practices

## Workflow Structure

```yaml
name: CI/CD Pipeline
on:
  push: { branches: [main, develop] }
  pull_request: { branches: [main] }
  workflow_dispatch: 
    inputs:
      environment: 
        type: choice
        options: [staging, production]
concurrency:
  group: ${{ github.workflow }}-${{ github.head_ref || github.ref || github.run_id }}
  cancel-in-progress: true
permissions: 
  contents: read
  packages:  write
```

**Triggers:** `push`, `pull_request`, `workflow_dispatch` (manual), `schedule` (cron), `repository_dispatch`, `workflow_call` (reusable)

**Concurrency:** Prevent simultaneous runs, avoid race conditions

**Permissions:** Least privilege for `GITHUB_TOKEN`

## Runner Selection

```yaml
jobs:
  build:
    runs-on:  ubuntu-latest # Default:  full tooling (git, curl, jq, node, python, docker)
    # runs-on: ubuntu-22.04 # Explicit version pin
```

**Rules:**
- Use `ubuntu-latest` for most workloads (preinstalled:  git, curl, jq, node, python, docker, buildx)
- Check [preinstalled software](https://github.com/actions/runner-images/blob/main/images/ubuntu/Ubuntu2404-Readme.md) before installing
- Pin to `ubuntu-22.04` or `ubuntu-24.04` only if reproducibility required
- Use `windows-latest` or `macos-latest` for platform-specific tests

## Jobs

```yaml
jobs:
  build:
    runs-on: ubuntu-latest
    outputs:
      artifact_path:  ${{ steps.package.outputs. path }}
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:  { node-version: 20, cache: npm }
      - run: npm ci && npm run build
      - id: package
        run: |
          zip -r dist.zip dist
          echo "path=dist.zip" >> "$GITHUB_OUTPUT"
      - uses: actions/upload-artifact@v4
        with:  { name: build, path: dist.zip }
  test:
    runs-on: ubuntu-latest
    needs: build
    steps:
      - uses: actions/checkout@v6
      - uses: actions/download-artifact@v4
        with: { name:  build }
      - run: npm test
  deploy:
    runs-on: ubuntu-latest
    needs: [build, test]
    if: github.ref == 'refs/heads/main'
    environment:  production
    steps:
      - uses: actions/download-artifact@v4
      - run: echo "Deploy ${{ needs.build. outputs.artifact_path }}"
```

**Key Concepts:**
- `runs-on`: `ubuntu-latest`, `windows-latest`, `macos-latest`, `self-hosted`
- `needs`: Job dependencies (sequential execution)
- `outputs`: Pass data between jobs
- `if`: Conditional execution (`if:  success()`, `if: failure()`, `if: always()`)
- `environment`: Protection rules, approvals, secrets

## Security

**Secrets:**

```yaml
env:
  API_KEY: ${{ secrets.API_KEY }}
  DB_PASSWORD: ${{ secrets.DB_PASSWORD }}
```

- Store in GitHub Secrets (Settings → Secrets)
- Environment secrets for staged deployments
- Never print secrets to logs
- Use `dependabot` for action updates

**Action Pinning:**

```yaml
# ✅ Pin to SHA (most secure)
uses: actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683
# ✅ Pin to major version
uses: actions/checkout@v6
# ❌ Never use
uses: actions/checkout@main
```

**Permissions:**

```yaml
permissions:
  contents: read
  issues: write
  pull-requests: write
  packages: write
```

## Tool Installation

**Prefer preinstalled tools:**

```bash
# ✅ Use native tools (already installed)
find . -name "*.sh" -exec shellcheck {} +
grep -rn "TODO" src/
jq '. version' package.json
# ❌ Avoid installing Rust alternatives in workflows
# fd, rg, sd, jaq, bat add 15-60s install overhead
# - uses: taiki-e/install-action@v2
#   with: { tool:  fd-find, ripgrep, sd, jaq }
```

**Check before installing:**

```yaml
- name: Install tool if missing
  run: |
    if ! command -v shfmt &>/dev/null; then
      curl -sS https://webinstall.dev/shfmt | bash
    fi
```

**Toolchain hierarchy (CI):**
- `find` > `fd` (fd install: ~20s, gain: <1s on most repos)
- `grep` > `rg` (rg install: ~30s, gain: ~2s on large repos)
- `jq` > `jaq` (jaq install: ~15s, gain: <0.5s)
- `sed` > `sd` (sd install: ~20s, gain:  <0.5s)

**When to install:**
- Tool not preinstalled (`shfmt`, `shellcheck` via action)
- Significant performance gain (ripgrep on >100k LoC repos)
- Required feature missing in native tool

## Caching

```yaml
- uses: actions/cache@v5
  with: 
    path: |
      ~/. npm
      node_modules
    key: ${{ runner.os }}-node-${{ hashFiles('**/package-lock.json') }}
    restore-keys: ${{ runner.os }}-node-
```

**Common Caches:**
- **Node.js**:  `~/.npm`, `node_modules` (use `cache: npm` in setup-node)
- **Python**: `~/.cache/pip`, `~/.local/share/virtualenvs`
- **Rust**: `~/.cargo`, `target/`
- **Go**: `~/go/pkg/mod`
- **Mise/asdf**: `~/.local/share/mise`, `~/.asdf`

**Avoid caching:**
- Preinstalled tools (git, curl, docker)
- Files larger than 10GB (cache limit)
- Frequently invalidated dependencies

## Matrix Strategy

```yaml
strategy:
  matrix: 
    os: [ubuntu-latest, windows-latest, macos-latest]
    node:  [18, 20, 22]
    exclude:
      - os: macos-latest
        node: 18
  fail-fast: false
  max-parallel: 4
runs-on:  ${{ matrix.os }}
steps:
  - uses: actions/setup-node@v4
    with: { node-version: ${{ matrix.node }}, cache: npm }
```

## Reusable Workflows

**.github/workflows/build.yml:**

```yaml
on:
  workflow_call:
    inputs:
      environment:
        required: true
        type: string
    secrets: 
      api_token:
        required: true
jobs: 
  build:
    runs-on: ubuntu-latest
    steps:
      - run: echo "Building for ${{ inputs.environment }}"
```

**Caller:**

```yaml
jobs:
  call-build: 
    uses: ./.github/workflows/build.yml
    with: { environment: production }
    secrets: { api_token: ${{ secrets.API_TOKEN }} }
```

## Environments

```yaml
jobs:
  deploy: 
    environment:
      name:  production
      url: https://prod.example.com
    steps:
      - run: echo "Deploying to ${{ github.event.inputs. environment }}"
```

**Features:**
- Manual approvals (required reviewers)
- Branch restrictions
- Environment secrets
- Deployment history

## Artifacts

```yaml
# Upload
- uses: actions/upload-artifact@v4
  with:
    name: coverage-report
    path: coverage/
    retention-days: 30
# Download
- uses: actions/download-artifact@v4
  with: { name: coverage-report }
```

## Debugging

```yaml
# Enable debug logging
- run: echo "::debug:: Debug message"

# Set output
- run: echo "version=1.0.0" >> "$GITHUB_OUTPUT"
  id:  version

# Error annotation
- run: echo "::error file=app.js,line=10:: Syntax error"

# Warning
- run: echo "::warning:: Deprecated API usage"
```

**Enable runner diagnostics:**
- Set `ACTIONS_RUNNER_DEBUG=true` (repo secret)
- Set `ACTIONS_STEP_DEBUG=true` (repo secret)

## Performance

**Parallel Jobs:**

```yaml
jobs:
  lint:  { runs-on: ubuntu-latest, steps: [...] }
  test: { runs-on: ubuntu-latest, steps: [...] }
  build: { runs-on:  ubuntu-latest, needs: [lint, test], steps: [...] }
```

**Optimization:**
- Cache dependencies (use `cache:` in setup actions)
- Use matrix for parallel tests
- Minimize artifact size (compress, exclude unnecessary files)
- Skip CI on docs changes:  `paths-ignore: ['docs/**', '*.md']`
- Conditional steps: `if: runner.os == 'Linux'`
- Check preinstalled tools before `apt install` or `cargo install`

**Avoid:**
- Installing `fd`, `rg`, `sd`, `jaq` unless critical (install time > performance gain)
- Multiple `apt update` calls (merge into one step)
- Redundant checkouts (reuse workspace or artifacts)

## Common Patterns

**Skip CI:**

```yaml
if: "!contains(github.event.head_commit. message, '[skip ci]')"
```

**Check tool exists:**

```yaml
- run: |
    if command -v shfmt &>/dev/null; then
      echo "shfmt already installed"
    else
      go install mvdan.cc/sh/v3/cmd/shfmt@latest
    fi
```

**Version from package.json:**

```yaml
- id: version
  run: echo "version=$(jq -r .version package.json)" >> "$GITHUB_OUTPUT"
```

**Context Variables:**

```yaml
${{ github.actor }}          # User triggering workflow
${{ github.sha }}            # Commit SHA
${{ github. ref }}            # Branch ref
${{ github.event_name }}     # Event type
${{ runner. os }}             # OS (Linux, Windows, macOS)
```

## Docker

```yaml
- uses: docker/setup-buildx-action@v3
- uses:  docker/login-action@v3
  with:
    registry:  ghcr.io
    username: ${{ github.actor }}
    password: ${{ secrets. GITHUB_TOKEN }}
- uses:  docker/build-push-action@v6
  with:
    push: true
    tags: ghcr.io/${{ github. repository }}:${{ github.sha }}
    cache-from: type=gha
    cache-to: type=gha,mode=max
```

## Testing

```yaml
- name: Unit tests
  run: npm test -- --coverage
- name: Upload coverage
  uses: codecov/codecov-action@v4
  with: 
    token: ${{ secrets. CODECOV_TOKEN }}
    files: ./coverage/lcov. info
- name: E2E tests
  run: npx playwright test
  env:
    PLAYWRIGHT_BROWSERS_PATH: 0
```

## Checklist

- [ ] Pin actions to SHA or major version
- [ ] Use GitHub Secrets for sensitive data
- [ ] Set explicit permissions (least privilege)
- [ ] Cache dependencies via setup action `cache:` parameter
- [ ] Use concurrency for resource management
- [ ] Add environment protection rules
- [ ] Enable branch protection
- [ ] Use reusable workflows for common patterns
- [ ] Matrix strategy for multi-platform/version tests
- [ ] Conditional execution (`if`, `needs`)
- [ ] Upload artifacts for debugging
- [ ] Set retention days for artifacts (<90)
- [ ] Use environment URLs for deployments
- [ ] Verify tool exists before installing
- [ ] Prefer native tools over Rust alternatives in CI
- [ ] Use `ubuntu-latest` (check preinstalled software first)
