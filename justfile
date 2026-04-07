# Get current version from Cargo.toml
version := `grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/'`

# Show current version
current:
    @echo "{{version}}"

# Bump version: just bump 0.3.0
bump new_version:
    # Cargo.toml + Cargo.lock
    sed -i '0,/^version = ".*"/s//version = "{{new_version}}"/' Cargo.toml
    cargo update --workspace
    # DKMS
    sed -i 's/^PACKAGE_VERSION=.*/PACKAGE_VERSION="{{new_version}}"/' kmod/dkms.conf
    # PKGBUILD
    sed -i 's/^pkgver=.*/pkgver={{new_version}}/' pkg/PKGBUILD
    # Install hooks
    sed -i 's|xbelite2-dkms/[0-9.]*|xbelite2-dkms/{{new_version}}|g' pkg/xbelite2.install xbelite2.install
    @echo "Bumped to {{new_version}}"

# Bump, commit, tag, and push
release new_version: (bump new_version)
    git add Cargo.toml Cargo.lock kmod/dkms.conf pkg/PKGBUILD pkg/xbelite2.install xbelite2.install
    git commit -am "v{{new_version}}"
    git tag -a "v{{new_version}}" -m "v{{new_version}}"
    git push && git push --tags
    gh release create "v{{new_version}}" --generate-notes
