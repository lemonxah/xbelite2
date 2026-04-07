#!/bin/bash
set -euo pipefail

echo "Building xbelite2d..."
cargo build --release

echo "Installing binary..."
sudo install -m 755 target/release/xbelite2d /usr/local/bin/xbelite2d

echo "Installing udev rules..."
sudo install -m 644 99-xbelite2.rules /etc/udev/rules.d/99-xbelite2.rules
sudo udevadm control --reload-rules
sudo udevadm trigger

echo "Installing systemd service..."
sudo install -m 644 xbelite2d.service /etc/systemd/system/xbelite2d.service
sudo systemctl daemon-reload

echo ""
echo "Installation complete!"
echo ""
echo "To start the daemon:"
echo "  sudo systemctl start xbelite2d"
echo ""
echo "To enable on boot:"
echo "  sudo systemctl enable xbelite2d"
echo ""
echo "To check status:"
echo "  sudo systemctl status xbelite2d"
echo ""
echo "NOTE: You may need to blacklist xpadneo for Elite 2 PIDs if it's installed."
echo "Add to /etc/modprobe.d/xbelite2.conf:"
echo '  # Prevent xpadneo from claiming Elite 2 controllers'
echo '  # (only needed if xpadneo is installed)'
