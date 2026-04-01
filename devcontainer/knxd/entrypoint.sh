#!/bin/sh
set -e

echo "Starting KNX daemon setup..."

# Create necessary directories
mkdir -p /tmp /var/run/knxd /home/knxd/.config

# KNX daemon address and client range
KNX_ADDRESS=${ADDRESS:-0.0.1}
KNX_CLIENT_START=${CLIENT_ADDRESS:-0.0.2:8}
DEBUG_LEVEL=${DEBUG_LEVEL:-info}

echo "KNX Configuration:"
echo "  Daemon Address: $KNX_ADDRESS"
echo "  Client Range: $KNX_CLIENT_START"
echo "  Debug Level: $DEBUG_LEVEL"
echo "  Interface: dummy (simulation mode)"

# Create a minimal working knxd configuration file
cat > /home/knxd/.config/knxd.ini << EOF
[main]
addr = $KNX_ADDRESS
client-addrs = $KNX_CLIENT_START
connections = server,dummy

[server]
server = ets_router
port = 3671
discover = true
tunnel = true
router = true

[dummy]
driver = dummy
EOF

echo "Configuration file created:"
cat /home/knxd/.config/knxd.ini

echo "Starting knxd daemon with configuration file..."
echo "KNX/IP server will be available on port 3671"
echo "KNX daemon control will be available on port 6720"

# Start knxd with configuration file (no -t option for config file mode)
exec knxd /home/knxd/.config/knxd.ini
