#!/bin/bash
set -euo pipefail

echo "Building userspace tools..."
cargo build --release --workspace

echo "Installing binaries..."
sudo install -m 755 target/release/xbe2-rw  /usr/local/bin/xbe2-rw
sudo install -m 755 target/release/xbe2-bt  /usr/local/bin/xbe2-bt
if [[ -x target/release/xbelite2-gui ]]; then
    sudo install -m 755 target/release/xbelite2-gui /usr/local/bin/xbelite2-gui
fi

echo "Installing udev rules..."
sudo install -m 644 99-xbelite2.rules /etc/udev/rules.d/99-xbelite2.rules
sudo udevadm control --reload-rules
sudo udevadm trigger --subsystem-match=misc --action=change || true

echo "Building kernel module..."
make -C kmod

echo "Installing kernel module..."
KVER="$(uname -r)"
sudo install -m 644 kmod/xbelite2.ko "/lib/modules/${KVER}/extra/xbelite2.ko"
sudo depmod -a

echo ""
echo "Installation complete."
echo ""
echo "Load the module with:  sudo modprobe xbelite2"
echo "Unload with:           sudo rmmod xbelite2"
echo ""
echo "Ensure your user is in the 'input' group so xbe2-rw can access /dev/xbelite2:"
echo "  sudo usermod -aG input \$USER   # logout/login for it to take effect"
