# KeliCAD Agent

Local simulation agent for KeliCAD - runs LTspice simulations on your computer.

## Why Use This Agent?

LTspice cannot be used commercially on servers due to its EULA restrictions. This agent allows you to run LTspice simulations locally on your own computer while using the KeliCAD web application, ensuring full compliance with LTspice's license terms.

## Features

- Runs as a background application (system tray)
- Auto-detects LTspice installation
- WebSocket server for communication with KeliCAD web app
- Secure: only accepts connections from localhost and verified origins

## Installation

### Pre-built Binaries

Download the latest release for your platform:

- **Windows**: `kelicad-agent_x.x.x_x64-setup.exe`
- **macOS (Intel)**: `kelicad-agent_x.x.x_x64.dmg`
- **macOS (Apple Silicon)**: `kelicad-agent_x.x.x_aarch64.dmg`

### Building from Source

Requirements:
- Rust 1.70+
- Node.js 18+
- LTspice installed on your system

```bash
# Install dependencies
npm install

# Development mode
npm run dev

# Build for production
npm run build
```

## Usage

1. Install and launch the KeliCAD Agent
2. The agent will run in your system tray
3. Open the KeliCAD web application
4. Click "Connect Agent" in the circuit editor
5. Select LTspice as your simulator
6. Run your simulations!

## How It Works

1. The agent starts a WebSocket server on `localhost:9347`
2. When you click "Connect Agent" in KeliCAD, your browser connects to this local server
3. When you run a simulation with LTspice selected, the netlist is sent to the agent
4. The agent runs LTspice, parses the results, and sends them back
5. Results are displayed in the KeliCAD waveform viewer

## Security

- **Localhost Only**: The WebSocket server only binds to `127.0.0.1`, preventing external access
- **Origin Validation**: Only accepts connections from `kelicad.com` and `localhost:3000`
- **No Data Storage**: Netlists and results are processed in memory and not stored

## Supported Platforms

| Platform | Architecture | Status |
|----------|--------------|--------|
| Windows | x64 | ✅ Supported |
| Windows | ARM64 | ✅ Supported |
| macOS | x64 (Intel) | ✅ Supported |
| macOS | ARM64 (Apple Silicon) | ✅ Supported |
| Linux | - | ❌ LTspice not available |

## Troubleshooting

### LTspice Not Detected

The agent looks for LTspice in these locations:

**Windows:**
- `C:\Program Files\LTC\LTspiceXVII\XVIIx64.exe`
- `C:\Program Files\LTC\LTspice\LTspice.exe`
- `C:\Program Files (x86)\LTC\LTspiceXVII\XVIIx86.exe`

**macOS:**
- `/Applications/LTspice.app/Contents/MacOS/LTspice`

If LTspice is installed in a different location, you'll need to add it to your system PATH.

### Connection Failed

1. Ensure the agent is running (check system tray)
2. Make sure no other application is using port 9347
3. Try restarting the agent

## License

Copyright (c) 2024-2025 Wanyeki Technologies LLC. All rights reserved.

This software is proprietary. The source code is made publicly available for
transparency and security review purposes only. You may view the code, report
bugs, and suggest improvements, but you may not use, modify, or distribute
this software for commercial purposes.

See the [LICENSE](LICENSE) file for full terms.

**Note**: This agent uses LTspice, which is subject to its own [EULA](https://www.analog.com/en/design-center/design-tools-and-calculators/ltspice-simulator.html).
Using this agent ensures compliance with LTspice's licensing terms by running simulations locally on your own computer.
