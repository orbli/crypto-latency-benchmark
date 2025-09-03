# Crypto Market Data Latency Benchmark

[![CI](https://github.com/orbli/crypto-latency-benchmark/workflows/CI/badge.svg)](https://github.com/orbli/crypto-latency-benchmark/actions)
[![Rust](https://img.shields.io/badge/rust-2024%20edition-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A comprehensive Rust benchmark suite comparing parsing performance of different cryptocurrency market data protocols for ultra-low latency trading.

## 🚀 Overview

This benchmark measures real-world latency for three market data protocols from Binance:
- **WebSocket + JSON** - Standard human-readable format
- **WebSocket + SBE** - Binary Simple Binary Encoding for ultra-low latency
- **FIX Market Data** - Financial Information Exchange protocol

## 📊 Performance Results

### Protocol Comparison

| Protocol | Parsing Latency (P50) | Message Size | Throughput | vs JSON |
|----------|----------------------|--------------|------------|---------|
| **WebSocket+JSON** | 272-524 ns | 172-297 bytes | 1.9-3.7M msg/s | baseline |
| **WebSocket+SBE** | 15-20 ns | 66-682 bytes | 2.9-59M msg/s | **18-35x faster** |
| **FIX Market Data** | 8,395 ns | 131-279 bytes | 0.1M msg/s | 0.4x slower |

### Network Latency (Real Production Data)

- **Trade Stream**: 2ms (P50) / 71ms (P99)  
- **Depth Stream**: 54ms (P50) / 104ms (P99)
- **Network dominates**: 99.999% of total latency
- **Geographic impact**: Location adds 50-100ms baseline

## 🔑 Key Findings

### SBE Performance Excellence
- **15-20 nanoseconds** parsing time
- **Zero-copy deserialization** directly from binary
- **62% smaller** message size (66 vs 172 bytes)
- **18-35x faster** than JSON parsing

### Network Reality Check
- **Network**: 2-104 milliseconds
- **Parsing**: 0.015-0.524 microseconds  
- **Ratio**: 1,000,000:1

> **Bottom Line**: Geography beats technology. A trader in Tokyo using JSON will beat a trader in New York using SBE when trading on Binance.

## 💡 Strategic Insights

### For HFT/Ultra-Low Latency Trading
- **Must have**: Colocation (reduces 100ms → 1-5ms)
- **Nice to have**: SBE (saves 0.5 microseconds)  
- **ROI**: Colocation provides 20-100x improvement, SBE provides 0.0005x

### For Regular Trading
- JSON is completely sufficient
- Network optimization matters more than protocol choice
- Protocol optimization is premature optimization

### For Institutional Trading
- FIX provides compliance and audit trails
- Performance penalty negligible vs network latency
- Standardization matters more than microseconds

## 🎯 Optimization Priority

1. **🥇 Colocation**: 95-99ms reduction ($5K-50K/month)
2. **🥈 Network tuning**: 10-20ms reduction  
3. **🥉 Protocol (SBE)**: 0.0005ms reduction (weeks of work)

## 🏗️ Architecture

### Implementation Details
- **Language**: Rust with async/await and tokio runtime
- **Protocols**: All three running in parallel for fair comparison
- **Authentication**: ED25519 signatures for SBE and FIX
- **Data Collection**: 60 seconds per stream for statistical significance
- **Message Parsing**: Zero-copy where possible, comprehensive timing

### Protocol Features

#### WebSocket + JSON
```rust
// Combined stream subscription
let streams = vec!["bookTicker", "trade", "depth@100ms"];
let url = format!("wss://stream.binance.com:9443/stream?streams={}", 
                 stream_params.join("/"));
```

#### WebSocket + SBE  
```rust
// Binary format with official Binance schema
let request = http::Request::builder()
    .header("X-MBX-APIKEY", api_key)
    .uri("wss://stream-sbe.binance.com:9443/ws/btcusdt@bestBidAsk")
```

#### FIX Market Data
```rust
// TCP+TLS with ED25519 authentication
let signature = generate_ed25519_signature(&payload, &private_key);
// Full FIX 4.4 message construction
```

## 🚀 Quick Start

### Prerequisites
- Rust 1.70+ with edition 2024
- Binance API key with ED25519 authentication (for SBE/FIX)
- `.env` file with credentials

### Setup
```bash
git clone https://github.com/orbli/crypto-latency-benchmark.git
cd crypto-latency-benchmark

# Create .env file
cp .env.example .env
# Add your BINANCE_API_KEY and BINANCE_API_SECRET

# Build in release mode
cargo build --release
```

### Run Benchmarks

```bash
# Run all protocols in parallel (default)
cargo run --release

# Run individual protocols
cargo run --release json    # JSON WebSocket only
cargo run --release sbe     # SBE binary only (requires API key)  
cargo run --release fix     # FIX market data only (requires API key)

# Quick synthetic benchmark
cargo run --release quick
```

## 📋 Requirements

### For JSON Streams
- No authentication required
- Public WebSocket endpoints

### For SBE Streams  
- ED25519 API key required
- Binance account with API access
- Binary message parsing capability

### For FIX Streams
- ED25519 API key required  
- FIX_API permission enabled
- TCP+TLS connection handling

## 🔧 Configuration

### Environment Variables
```bash
# Required for SBE and FIX
BINANCE_API_KEY=your_api_key_here
BINANCE_API_SECRET=your_ed25519_private_key_base64

# Optional
USE_TESTNET=false
CONFIG_PATH=./config.yaml
```

### API Key Setup
1. Create Binance account
2. Generate ED25519 API key (not RSA/HMAC)
3. Enable FIX_API permissions for FIX protocol
4. Add to `.env` file

## 📈 Sample Output

```
================================================================================
PERFORMANCE COMPARISON TABLE
================================================================================
Protocol             Data Type          Time (ns) Size (bytes)   Throughput      vs JSON
--------------------------------------------------------------------------------
WebSocket+JSON       @bookTicker              272          172   3.7M msg/s     baseline
                     @depth                   524          297   1.9M msg/s     baseline
--------------------------------------------------------------------------------  
WebSocket+SBE        @bookTicker               15           66  66.7M msg/s        18.1x
                     @depth                    15           66  66.7M msg/s        34.9x
--------------------------------------------------------------------------------
FIX Market Data      @bookTicker              659          131   1.5M msg/s         0.4x
                     @depth                  1227          279   0.8M msg/s         0.4x
--------------------------------------------------------------------------------
```

## 🧠 Technical Excellence

### Why Rust?
- **Zero-cost abstractions** for maximum performance
- **Memory safety** without garbage collection overhead  
- **Async/await** for efficient I/O handling
- **Cross-platform** binary protocol support

### Measurement Methodology
- **High-precision timing** using `quanta` crate
- **Real production data** from live Binance streams
- **Statistical analysis** with percentiles (P50, P90, P95, P99, P99.9)
- **Parallel execution** for fair comparison
- **Zero-copy parsing** where architecturally possible

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Add benchmarks or protocol implementations
4. Submit a pull request

## 📝 License

MIT License - feel free to use in your trading systems.

## ⚠️ Disclaimer

This is educational/research code. Use at your own risk for actual trading. Past performance doesn't guarantee future results. Geographic location and network infrastructure have far greater impact on latency than protocol choice.

---

**Made with ❤️ and ⚡ for the trading community**

*"In trading, milliseconds matter, but geography matters more."*