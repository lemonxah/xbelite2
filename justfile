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

# Build and reload kernel module (dev cycle)
kmod:
    make -C kmod
    -sudo rmmod xbelite2 2>/dev/null
    sudo insmod kmod/xbelite2.ko
    @echo "Module loaded"

# Build daemon
build:
    cargo build --workspace --release

# Build and install everything locally (no package)
install: build
    sudo systemctl stop xbelite2d
    sudo cp target/release/xbelite2d /usr/bin/xbelite2d
    sudo cp target/release/xbelite2-gui /usr/bin/xbelite2-gui
    sudo systemctl start xbelite2d
    @echo "Daemon restarted"

# Disable all Xbox controller modules for USB passthrough to VM
passthrough:
    sudo systemctl stop xbelite2d
    -sudo rmmod xbelite2 2>/dev/null
    -sudo rmmod xpad 2>/dev/null
    -sudo rmmod hid_microsoft 2>/dev/null
    printf 'blacklist xbelite2\nblacklist xpad\nblacklist hid_microsoft\ninstall xbelite2 /bin/false\ninstall xpad /bin/false\n' | sudo tee /etc/modprobe.d/xbelite2-temp-blacklist.conf
    sudo depmod -a
    sudo udevadm control --reload-rules
    sudo udevadm trigger
    @echo "All Xbox modules blacklisted. Unplug controller, wait 5 sec, replug, then pass to VM."
    @echo "Run 'just passthrough-done' when finished."

# Re-enable modules after VM passthrough
passthrough-done:
    sudo rm -f /etc/modprobe.d/xbelite2-temp-blacklist.conf
    sudo udevadm control --reload-rules
    sudo modprobe xbelite2
    sudo systemctl start xbelite2d
    @echo "Modules re-enabled"

aur_dir := "../aur/xbelite2-dkms"

# Bump, commit, tag, and push
release new_version: (bump new_version)
    git add Cargo.toml Cargo.lock kmod/dkms.conf pkg/PKGBUILD pkg/xbelite2.install xbelite2.install
    git commit -am "v{{new_version}}"
    git tag -a "v{{new_version}}" -m "v{{new_version}}"
    git push && git push --tags
    gh release create "v{{new_version}}" --generate-notes
    @just aur-publish {{new_version}}

# Update AUR repo with new version
aur-publish new_version:
    cp pkg/xbelite2.install {{aur_dir}}/xbelite2.install
    sed -i 's/^pkgver=.*/pkgver={{new_version}}/' {{aur_dir}}/PKGBUILD
    cd {{aur_dir}} && makepkg --printsrcinfo > .SRCINFO
    cd {{aur_dir}} && git commit -am "updated the version to {{new_version}}" && git push
