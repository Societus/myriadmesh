# Raspberry Pi Quick Start Guide

**Target Time**: < 10 minutes from zero to first message
**Status**: 📋 DRAFT - Ready for Testing
**Document Version**: 1.0
**Last Updated**: 2025-11-16

---

## Quick Links
- [Hardware Setup](#hardware-setup) (2 min)
- [Installation](#installation) (3 min)
- [First Message](#first-message) (2 min)
- [Troubleshooting](#troubleshooting)

---

## Overview

This guide walks you through setting up MyriadMesh on a Raspberry Pi and sending your first message over the mesh network.

**What You'll Need**:
- Raspberry Pi 3B or newer (or Pi Zero W for client-only mode)
- microSD card (16GB recommended)
- USB power supply
- Network adapter (Ethernet or WiFi)
- Optional: LoRa radio module, BLE adapter, etc.

**What You'll Get**:
- A mesh network node
- Ability to send/receive messages
- Connection to other nodes
- Optional: Gateway to other networks

---

## Hardware Setup (2 minutes)

### Step 1: Prepare the microSD Card

**For Beginners**: Use Raspberry Pi Imager
1. Download [Raspberry Pi Imager](https://www.raspberrypi.com/software/)
2. Insert microSD card into your computer
3. Open Raspberry Pi Imager
4. Choose Device: Raspberry Pi 4 (or your model)
5. Choose OS: Raspberry Pi OS Lite (32-bit recommended)
6. Click "NEXT" and wait for download/write

**Alternative**: Download disk image directly and write with `dd`:
```bash
wget https://downloads.raspberrypi.org/raspios_lite_armhf/images/raspios_lite_armhf-*/\
*.zip
unzip *.zip
sudo dd if=*.img of=/dev/sdX bs=4M conv=fsync
```

### Step 2: Boot Your Raspberry Pi

1. Insert microSD card into Pi
2. Connect Ethernet cable (recommended for first setup)
3. Connect USB power supply
4. Wait 30 seconds for boot
5. Pi should appear on your network

### Step 3: Connect to Your Pi

**With Monitor/Keyboard** (easiest for beginners):
- Connect HDMI monitor and USB keyboard
- Wait for login prompt
- Default: `pi` / `raspberry`

**Via SSH** (recommended):
```bash
# From your computer, find your Pi's IP
ping raspberrypi.local

# SSH in (password: raspberry)
ssh pi@raspberrypi.local
```

---

## Installation (3 minutes)

### Step 1: Update System (1 minute)

```bash
# Login to your Pi first (via SSH or terminal)
sudo apt update
sudo apt upgrade -y
sudo apt install -y build-essential libsodium-dev
```

**Time**: ~1 minute
**Status**: Watch the "Setting up" messages

### Step 2: Install MyriadMesh (2 minutes)

**Option A: Install from Package (Recommended)**
```bash
# Add MyriadMesh repository
curl https://repo.myriadmesh.org/install.sh | sudo bash

# Install the package
sudo apt install myriadmesh

# Check installation
myriadnode --version
```

**Option B: Build from Source** (if packages not available)
```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Clone and build
git clone https://github.com/yourorg/myriadmesh.git
cd myriadmesh
cargo build --release

# Install binary
sudo cp target/release/myriadnode /usr/local/bin/
sudo chmod +x /usr/local/bin/myriadnode
```

### Step 3: Initialize Node (< 1 minute)

```bash
# Initialize node (creates keys and default config)
myriadnode --init

# Output should show:
#   ✅ Node ID: [your-node-id]
#   ✅ Generated keys (public/private)
#   ✅ Created config at ~/.myriadmesh/config.toml
#   ✅ Ready to start!
```

---

## First Message (2 minutes)

### Step 1: Start the Node

```bash
# Start myriadnode in foreground (for testing)
myriadnode

# Expected output:
#   2025-11-16 10:42:00 INFO  myriadnode: Starting MyriadMesh node
#   2025-11-16 10:42:00 INFO  node: Node ID: abc123...
#   2025-11-16 10:42:05 INFO  network: Ethernet adapter online
#   2025-11-16 10:42:10 INFO  dht: Joining DHT network...
#   2025-11-16 10:42:15 INFO  api: REST API listening on http://0.0.0.0:8000
#
# Leave this running. Open a new terminal to continue.
```

### Step 2: Open the Web UI

**Option A: Local Network**
- On any device on your network, open:
  ```
  http://raspberrypi.local:8000
  ```
- Or find your Pi's IP and use `http://[Pi-IP]:8000`

**Option B: From the Pi itself**
```bash
# Open another SSH session to your Pi
ssh pi@raspberrypi.local

# Use curl to send a test message
curl -X POST http://localhost:8000/api/messages \
  -H "Content-Type: application/json" \
  -d '{
    "to": "broadcast",
    "content": "Hello from my Raspberry Pi!",
    "urgent": false
  }'

# Response:
#   {"message_id": "msg_abc123", "status": "queued"}
```

### Step 3: Verify Message Sent

**Check Node Logs**:
```
# In the myriadnode terminal, you should see:
2025-11-16 10:45:23 INFO  router: Message msg_abc123 queued for delivery
2025-11-16 10:45:24 INFO  network: Broadcasting to 3 peers
2025-11-16 10:45:25 INFO  router: Delivery confirmed for msg_abc123
```

**Check Web Dashboard**:
- Visit `http://raspberrypi.local:8000` in your browser
- Look for "Network Status" showing:
  - Node ID
  - Number of peers
  - Message count

**🎉 Congratulations!** Your first message has been sent.

---

## Running as Service (Optional)

For permanent operation (survives reboots):

```bash
# Enable as systemd service
sudo systemctl enable myriadmesh
sudo systemctl start myriadmesh

# Check status
sudo systemctl status myriadmesh

# View logs
sudo journalctl -u myriadmesh -f
```

---

## Connecting More Nodes

### On Another Raspberry Pi or Computer

1. Repeat the **Installation** section above
2. When both nodes are running and on the same network:
   - They will automatically discover each other via mDNS
   - Check the Web UI - you should see peers appear
3. Send a message - it will route through all connected nodes

**Expected behavior**:
- Nodes appear in each other's peer lists within 30 seconds
- Messages route automatically
- Network grows with each new node

---

## Troubleshooting

### "No such file or directory: myriadnode"

**Problem**: Binary not found
**Solution**:
```bash
# Check if installed
which myriadnode

# If not found, install from source:
# (See Option B under Step 2 of Installation)

# Or check installation path:
ls -la /usr/local/bin/myriadnode
```

### "Connection refused" or "API not responding"

**Problem**: myriadnode not running
**Solution**:
```bash
# Check if running
ps aux | grep myriadnode

# If not, start it:
myriadnode

# If it crashes immediately, check logs:
myriadnode 2>&1 | head -20
```

### "Could not bind to port 8000"

**Problem**: Another service using port 8000
**Solution**:
```bash
# Find what's using the port
sudo lsof -i :8000

# Change port in config
nano ~/.myriadmesh/config.toml
# Find line: api_port = 8000
# Change to: api_port = 8001

# Restart
myriadnode
```

### Node doesn't find peers

**Problem**: Network isolation
**Solution**:
```bash
# Check network interface
ifconfig

# Verify Ethernet/WiFi is connected:
# Look for IP address on eth0 or wlan0

# If WiFi, check connection:
iwconfig

# Check if other nodes are discoverable:
avahi-browse -at | grep myriadmesh
```

### "Error: Failed to initialize node"

**Problem**: Permission or config issue
**Solution**:
```bash
# Check permissions on config directory
ls -la ~/.myriadmesh/

# If not readable, fix:
chmod 700 ~/.myriadmesh/
chmod 600 ~/.myriadmesh/config.toml

# Check config file syntax:
myriadnode --validate-config

# Reinitialize if needed:
rm -rf ~/.myriadmesh/
myriadnode --init
```

### Can't connect via SSH

**Problem**: Couldn't find Pi on network
**Solution**:
```bash
# From another computer on same network:
ping raspberrypi.local

# If no response, find its IP:
nmap -sn 192.168.1.0/24 | grep -i raspberry

# Or check your router's connected devices

# Once found, SSH with the IP:
ssh pi@192.168.1.100
```

### Messages not being delivered

**Problem**: Peers not connecting
**Solution**:
```bash
# Check node logs for peer connections
# In myriadnode output, look for:
#   "peer_added: ..."  = peer connected
#   "peer_removed: ..." = peer disconnected

# Verify network is connected
ping 8.8.8.8

# Check local network connectivity
ping raspberrypi.local

# If only 1 node, messages may queue:
# - Wait a few seconds for peers to appear
# - Or broadcast to test message delivery

# Check logs for errors:
myriadnode 2>&1 | grep -i "error\|failed"
```

---

## Next Steps

### Learn More
- **Configuration**: See [Configuration Reference](../CONFIGURATION_REFERENCE.md)
- **Admin Guide**: See [Administrator Guide](../ADMIN_DEPLOYMENT_GUIDE.md)
- **Radio Adapters**: Connect LoRa, HF radio, etc.

### Enhance Your Setup
1. **Add LoRa Adapter**: For long-range mesh (>100km)
2. **Add BLE Adapter**: For mobile phones
3. **Add Cellular Backhaul**: For connecting to internet
4. **Monitor Dashboard**: Run `myriadmesh-tui` for terminal UI

### Deploy More Nodes
- Add Pi Zero W nodes as clients (low power)
- Add Pi 4 nodes as gateways (full features)
- Create mesh coverage in your area

---

## Getting Help

### Resources
- **Project Issues**: https://github.com/yourorg/myriadmesh/issues
- **Community Chat**: https://discord.gg/myriadmesh
- **Documentation**: https://myriadmesh.org/docs

### Common Questions

**Q: How far can messages travel?**
A: Depends on adapters. Ethernet: 100m (LAN), LoRa: 10km+, HF: 1000km+

**Q: What happens if I unplug the Pi?**
A: If it was a gateway, network splits. Other peers will find alternate routes.

**Q: Can I run on a Pi Zero W?**
A: Yes, but with fewer features and adapters. RAM is limited to 512MB.

**Q: Is data encrypted?**
A: Yes, all messages are encrypted end-to-end using XSalsa20-Poly1305.

**Q: Can I use WiFi instead of Ethernet?**
A: Yes, WiFi works fine but may have more latency and packet loss than Ethernet.

---

## Safety & Compliance

⚠️ **Radio Regulations**: If using radio adapters (LoRa, HF, CB, GMRS):
- Check local FCC/OFCOM rules
- Adhere to power and frequency restrictions
- Use licensed frequencies where required
- See [License Management Guide](../RADIO_LICENSE_GUIDE.md)

---

## Feedback

Found an issue with this guide? Help improve it:
1. Tried this guide and got stuck? [Open an issue](https://github.com/yourorg/myriadmesh/issues/new)
2. Have improvement suggestions? [Submit a PR](https://github.com/yourorg/myriadmesh/pulls)
3. Need clarification? Ask in [community chat](https://discord.gg/myriadmesh)

---

**Time Estimate**: ⏱️ 7-10 minutes (varies by Pi speed and SD card write speed)
**Difficulty**: Beginner
**Requirements**: None (Pi + power + network)
**Status**: ✅ Ready for testing on actual hardware
