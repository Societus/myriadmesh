# Contributing to MyriadMesh

Thank you for your interest in contributing to MyriadMesh! This document provides guidelines and instructions for contributing to the project.

## Code of Conduct

Be respectful, inclusive, and professional in all interactions. We're building a communication protocol for everyone.

## Getting Started

### Prerequisites

- Rust (stable toolchain)
- libsodium development libraries
- Git

### Setting Up Your Development Environment

1. **Clone the repository:**
   ```bash
   git clone https://github.com/Societus/myriadmesh.git
   cd myriadmesh
   ```

2. **Install dependencies:**

   **Ubuntu/Debian:**
   ```bash
   sudo apt-get install libsodium-dev pkg-config
   ```

   **macOS:**
   ```bash
   brew install libsodium
   ```

   **Fedora:**
   ```bash
   sudo dnf install libsodium-devel
   ```

3. **Build the project:**
   ```bash
   cargo build
   ```

4. **Run tests:**
   ```bash
   cargo test
   ```

## Development Workflow

### Branch Strategy

- `main` - Stable release branch
- `develop` - Integration branch for features
- `feature/*` - New features
- `fix/*` - Bug fixes
- `docs/*` - Documentation updates

### Making Changes

1. **Create a new branch:**
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make your changes:**
   - Write clean, documented code
   - Add tests for new functionality
   - Update documentation as needed

3. **Format your code:**
   ```bash
   cargo fmt --all
   ```

4. **Run linter:**
   ```bash
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   ```

5. **Run tests:**
   ```bash
   cargo test --workspace --all-features
   ```

6. **Pre-submission checklist** (run before committing):

   **IMPORTANT:** Run these checks to ensure CI will pass:

   ```bash
   # 1. Format check (no changes should be needed)
   cargo fmt --all -- --check

   # 2. Clippy on entire workspace
   cargo clippy --workspace --all-targets --all-features -- -D warnings

   # 3. Run all tests
   cargo test --workspace --all-features

   # 4. Build all targets
   cargo build --workspace --all-targets --all-features
   ```

   **Common issues to watch for:**
   - Unused imports (especially after refactoring)
   - Unnecessary `.clone()` calls on Copy types (NodeId, PublicKey)
   - Needless borrows in function arguments
   - `mut` variables that aren't actually mutated
   - Dead code in incomplete features (use `#[allow(dead_code)]` with comment)
   - Formatting inconsistencies (run `cargo fmt --all`)

   **Known exceptions:**
   - `myriadmesh-android`: Skeleton implementation awaiting hardware integration
     - Contains intentional dead code for future MyriadNode integration
     - JNI functions require `mut env` even when clippy suggests otherwise

7. **Commit your changes:**
   ```bash
   git add .
   git commit -m "Add feature: description of your changes"
   ```

   **Commit message format:**
   - Use imperative mood: "Add feature" not "Added feature"
   - First line: short summary (50 chars or less)
   - Blank line, then detailed explanation if needed
   - Reference relevant issues: "Fixes #123"

7. **Push to your fork:**
   ```bash
   git push origin feature/your-feature-name
   ```

8. **Create a Pull Request:**
   - Go to the GitHub repository
   - Click "New Pull Request"
   - Select your branch
   - Fill out the PR template
   - Wait for review

## Code Style

### Rust Guidelines

- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `rustfmt` for formatting (configuration in `rustfmt.toml`)
- Address all `clippy` warnings
- Maximum line length: 100 characters
- Use descriptive variable and function names

### Documentation

- All public APIs must have documentation comments
- Use `///` for documentation comments
- Include examples in doc comments where appropriate
- Document error conditions and panics

Example:
```rust
/// Sign a message using the node's identity
///
/// # Arguments
///
/// * `identity` - The node identity containing the signing key
/// * `message` - The message bytes to sign
///
/// # Returns
///
/// A `Signature` struct containing the Ed25519 signature
///
/// # Example
///
/// ```
/// use myriadmesh_crypto::*;
///
/// let identity = identity::NodeIdentity::generate().unwrap();
/// let signature = signing::sign_message(&identity, b"Hello").unwrap();
/// ```
pub fn sign_message(identity: &NodeIdentity, message: &[u8]) -> Result<Signature> {
    // ...
}
```

### Testing

- Write unit tests for all new functionality
- Aim for >80% code coverage
- Include integration tests where appropriate
- Test edge cases and error conditions

Test naming convention:
```rust
#[test]
fn test_function_name_expected_behavior() {
    // Test implementation
}
```

## Project Structure

```
myriadmesh/
├── .github/
│   └── workflows/      # CI/CD configuration
├── crates/
│   ├── myriadmesh-core/       # Main library integration
│   ├── myriadmesh-crypto/     # Cryptographic primitives
│   └── myriadmesh-protocol/   # Protocol data structures
├── docs/               # Documentation
├── Cargo.toml         # Workspace configuration
└── README.md
```

## Areas for Contribution

### Phase 1 (Current)

- [x] Core cryptography implementation
- [x] Protocol data structures
- [x] Frame serialization
- [ ] Additional test coverage
- [ ] Mock network adapters

### Future Phases

Check [docs/roadmap/phases.md](docs/roadmap/phases.md) for upcoming work:
- DHT implementation
- Network adapters (Bluetooth, LoRa, etc.)
- Web UI
- Mobile applications

## Reporting Bugs

When reporting bugs, please include:

1. **Description:** Clear description of the bug
2. **Steps to reproduce:** Minimal code example
3. **Expected behavior:** What should happen
4. **Actual behavior:** What actually happens
5. **Environment:**
   - OS and version
   - Rust version (`rustc --version`)
   - MyriadMesh version

## Feature Requests

Feature requests are welcome! Please:

1. Check existing issues first
2. Describe the use case
3. Explain why it fits with MyriadMesh's goals
4. Propose an implementation approach if possible

## Security

If you discover a security vulnerability, please email [security contact] instead of creating a public issue.

## Review Process

All contributions go through code review:

1. **Automated checks:** CI must pass (tests, formatting, linting)
2. **Code review:** At least one maintainer review required
3. **Testing:** Verify functionality works as intended
4. **Documentation:** Check that docs are updated

Review criteria:
- [ ] Code follows style guidelines
- [ ] Tests are included and passing
- [ ] Documentation is updated
- [ ] No breaking changes (or justified if necessary)
- [ ] Commit history is clean

## License

By contributing to MyriadMesh, you agree that your contributions will be licensed under the GNU General Public License v3.0 (GPL-3.0).

## Questions?

- Open a discussion on GitHub
- Check existing documentation in `docs/`
- Review the roadmap: `docs/roadmap/phases.md`

## Acknowledgments

Thank you for contributing to MyriadMesh and helping build resilient communication infrastructure!
