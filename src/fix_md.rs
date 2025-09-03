use colored::*;
use quanta::Clock;
use rustls::pki_types::ServerName;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::{TlsConnector, rustls};

// FIX Protocol constants
const SOH: char = '\x01'; // Field delimiter
const SOH_BYTE: u8 = 0x01;

// FIX field tags for market data (from Binance fix-md schema)
const TAG_MSG_TYPE: u32 = 35;
const TAG_SENDER_COMP_ID: u32 = 49;
const TAG_TARGET_COMP_ID: u32 = 56;
const TAG_MSG_SEQ_NUM: u32 = 34;
const TAG_SENDING_TIME: u32 = 52;
const TAG_ENCRYPT_METHOD: u32 = 98;
const TAG_HEARTBEAT_INT: u32 = 108;
const TAG_RESET_SEQ_NUM: u32 = 141;
const TAG_USERNAME: u32 = 553;
const TAG_RAW_DATA: u32 = 96;
const TAG_RAW_DATA_LENGTH: u32 = 95;
const TAG_CHECKSUM: u32 = 10;
const TAG_SYMBOL: u32 = 55;
const TAG_MD_REQ_ID: u32 = 262;
const TAG_SUBSCRIPTION_REQUEST_TYPE: u32 = 263;
const TAG_MARKET_DEPTH: u32 = 264;
const TAG_MD_UPDATE_TYPE: u32 = 265;
const TAG_NO_MD_ENTRY_TYPES: u32 = 267;
const TAG_NO_MD_ENTRIES: u32 = 268;
const TAG_MD_ENTRY_TYPE: u32 = 269;
const TAG_MD_ENTRY_PX: u32 = 270;
const TAG_MD_ENTRY_SIZE: u32 = 271;
const TAG_NO_RELATED_SYM: u32 = 146;

// Message types
const MSG_TYPE_LOGON: &str = "A";
const MSG_TYPE_HEARTBEAT: &str = "0";
const MSG_TYPE_TEST_REQUEST: &str = "1";
const MSG_TYPE_LOGOUT: &str = "5";
const MSG_TYPE_MARKET_DATA_REQUEST: &str = "V";
const MSG_TYPE_MARKET_DATA_SNAPSHOT: &str = "W";
const MSG_TYPE_MARKET_DATA_INCREMENTAL: &str = "X";

// MD Entry Types
const MD_ENTRY_TYPE_BID: char = '0';
const MD_ENTRY_TYPE_OFFER: char = '1';
const MD_ENTRY_TYPE_TRADE: char = '2';

// ============================================================================
// FIX MESSAGE STRUCTURES
// ============================================================================

pub struct FixBookTicker {
    pub symbol: String,
    pub bid_price: f64,
    pub bid_qty: f64,
    pub ask_price: f64,
    pub ask_qty: f64,
}

pub struct FixDepthUpdate {
    pub symbol: String,
    pub bids: Vec<(f64, f64)>, // (price, quantity)
    pub asks: Vec<(f64, f64)>,
}

// ============================================================================
// FIX SESSION MANAGEMENT
// ============================================================================

pub struct FixSession {
    api_key: String,
    private_key: Vec<u8>,
    sender_comp_id: String,
    target_comp_id: String,
    msg_seq_num: u32,
    heartbeat_interval: u32,
}

impl FixSession {
    pub fn new(api_key: String, private_key: Vec<u8>) -> Self {
        Self {
            api_key,
            private_key,
            sender_comp_id: "RUST001".to_string(), // Unique 1-8 char ID
            target_comp_id: "SPOT".to_string(),
            msg_seq_num: 1,
            heartbeat_interval: 30,
        }
    }

    fn get_timestamp(&self) -> String {
        // Get current UTC timestamp
        use chrono::{DateTime, Utc};

        let now: DateTime<Utc> = Utc::now();
        // Format: YYYYMMDD-HH:MM:SS.mmm
        now.format("%Y%m%d-%H:%M:%S%.3f").to_string()
    }

    fn sign_message(&self, payload: &str) -> String {
        // Parse PEM-encoded ED25519 private key and sign the payload
        use ring::signature::{Ed25519KeyPair, KeyPair};

        // The private_key should be PEM format after base64 decoding from .env
        let pem_str = String::from_utf8_lossy(&self.private_key);

        // Extract the base64 content between PEM headers
        let key_data = if pem_str.contains("BEGIN") && pem_str.contains("END") {
            // It's PEM format - extract the base64 content
            let lines: Vec<&str> = pem_str.lines().collect();
            let mut b64_content = String::new();
            let mut in_key = false;

            for line in lines {
                if line.contains("BEGIN") {
                    in_key = true;
                    continue;
                }
                if line.contains("END") {
                    break;
                }
                if in_key && !line.is_empty() {
                    b64_content.push_str(line);
                }
            }

            // Decode the base64 content to get the DER-encoded key
            match base64::decode(&b64_content) {
                Ok(der_bytes) => der_bytes,
                Err(e) => {
                    eprintln!("Failed to decode PEM content: {}", e);
                    return String::new();
                }
            }
        } else {
            // Try as raw bytes
            self.private_key.clone()
        };

        // Try to parse as PKCS8 DER format
        if let Ok(key_pair) = Ed25519KeyPair::from_pkcs8(&key_data) {
            let signature = key_pair.sign(payload.as_bytes());
            return base64::encode(signature.as_ref());
        }

        // PKCS8 v1 ED25519 structure typically has the 32-byte seed at offset 16
        if key_data.len() == 48 {
            // Common PKCS8 v1 format for ED25519
            if let Ok(key_pair) = Ed25519KeyPair::from_seed_unchecked(&key_data[16..48]) {
                let signature = key_pair.sign(payload.as_bytes());
                return base64::encode(signature.as_ref());
            }
        }

        // Try to find the 32-byte seed in the DER structure
        // Look for OCTET STRING (0x04) followed by length (0x20 = 32)
        if let Some(pos) = key_data
            .windows(2)
            .position(|w| w[0] == 0x04 && w[1] == 0x20)
        {
            if key_data.len() >= pos + 2 + 32 {
                if let Ok(key_pair) =
                    Ed25519KeyPair::from_seed_unchecked(&key_data[pos + 2..pos + 34])
                {
                    let signature = key_pair.sign(payload.as_bytes());
                    return base64::encode(signature.as_ref());
                }
            }
        }

        eprintln!("Warning: Failed to parse ED25519 private key");
        eprintln!(
            "Key data length after PEM parsing: {} bytes",
            key_data.len()
        );
        String::new()
    }

    fn calculate_checksum(&self, message: &str) -> String {
        let sum: u32 = message.bytes().map(|b| b as u32).sum();
        format!("{:03}", sum % 256)
    }

    pub fn build_logon(&mut self) -> String {
        let timestamp = self.get_timestamp();

        // Build payload for signature according to Binance spec:
        // MsgType(35) + SOH + SenderCompId(49) + SOH + TargetCompId(56) + SOH + MsgSeqNum(34) + SOH + SendingTime(52)
        let payload = format!(
            "A{}{}{}{}{}{}{}{}",
            SOH,
            self.sender_comp_id,
            SOH,
            self.target_comp_id,
            SOH,
            self.msg_seq_num,
            SOH,
            timestamp
        );

        let signature = self.sign_message(&payload);

        if signature.is_empty() {
            eprintln!("Failed to generate signature for FIX logon");
            eprintln!("Check that your ED25519 private key is correctly formatted");
        }

        // Build full message
        let mut msg = String::new();
        msg.push_str(&format!("35={}{}", MSG_TYPE_LOGON, SOH));
        msg.push_str(&format!("49={}{}", self.sender_comp_id, SOH));
        msg.push_str(&format!("56={}{}", self.target_comp_id, SOH));
        msg.push_str(&format!("34={}{}", self.msg_seq_num, SOH));
        msg.push_str(&format!("52={}{}", timestamp, SOH));
        msg.push_str(&format!("98=0{}", SOH)); // EncryptMethod
        msg.push_str(&format!("108={}{}", self.heartbeat_interval, SOH));
        msg.push_str(&format!("141=Y{}", SOH)); // ResetSeqNumFlag
        msg.push_str(&format!("553={}{}", self.api_key, SOH)); // Username with API key
        msg.push_str(&format!("25035=1{}", SOH)); // MessageHandling (1 = Real-time)

        // RawDataLength must be the actual byte length of the signature
        let sig_bytes = signature.as_bytes();
        msg.push_str(&format!("95={}{}", sig_bytes.len(), SOH)); // RawDataLength
        msg.push_str(&format!("96={}{}", signature, SOH)); // RawData (base64 signature)

        // Add header and checksum
        let body_length = msg.len();
        let full_msg = format!("8=FIX.4.4{}9={}{}{}", SOH, body_length, SOH, msg);
        let checksum = self.calculate_checksum(&full_msg);

        self.msg_seq_num += 1;

        format!("{}10={}{}", full_msg, checksum, SOH)
    }

    pub fn build_market_data_request(&mut self, symbol: &str) -> String {
        let timestamp = self.get_timestamp();
        let req_id = format!("REQ{}", self.msg_seq_num);

        let mut msg = String::new();
        msg.push_str(&format!("35={}{}", MSG_TYPE_MARKET_DATA_REQUEST, SOH));
        msg.push_str(&format!("49={}{}", self.sender_comp_id, SOH));
        msg.push_str(&format!("56={}{}", self.target_comp_id, SOH));
        msg.push_str(&format!("34={}{}", self.msg_seq_num, SOH));
        msg.push_str(&format!("52={}{}", timestamp, SOH));
        msg.push_str(&format!("262={}{}", req_id, SOH)); // MDReqID
        msg.push_str(&format!("263=1{}", SOH)); // SubscriptionRequestType (1=Subscribe)
        msg.push_str(&format!("264=10{}", SOH)); // MarketDepth (10 levels)
        msg.push_str(&format!("146=1{}", SOH)); // NoRelatedSym (moved before symbols)
        msg.push_str(&format!("55={}{}", symbol, SOH)); // Symbol
        msg.push_str(&format!("267=2{}", SOH)); // NoMDEntryTypes (moved after symbol)
        msg.push_str(&format!("269=0{}", SOH)); // Bid
        msg.push_str(&format!("269=1{}", SOH)); // Offer

        let body_length = msg.len();
        let full_msg = format!("8=FIX.4.4{}9={}{}{}", SOH, body_length, SOH, msg);
        let checksum = self.calculate_checksum(&full_msg);

        self.msg_seq_num += 1;

        format!("{}10={}{}", full_msg, checksum, SOH)
    }
}

// ============================================================================
// FIX PARSER
// ============================================================================

pub struct FixMdParser;

impl FixMdParser {
    pub fn new() -> Self {
        Self
    }

    pub fn parse_message(&self, fix_msg: &str) -> HashMap<u32, String> {
        let mut fields = HashMap::new();

        for field in fix_msg.split(SOH) {
            if let Some(eq_pos) = field.find('=') {
                if let Ok(tag) = field[..eq_pos].parse::<u32>() {
                    let value = field[eq_pos + 1..].to_string();
                    fields.insert(tag, value);
                }
            }
        }

        fields
    }

    pub fn parse_book_ticker(&self, fix_msg: &str) -> Option<FixBookTicker> {
        let fields = self.parse_message(fix_msg);

        if fields.get(&TAG_MSG_TYPE)? != MSG_TYPE_MARKET_DATA_INCREMENTAL
            && fields.get(&TAG_MSG_TYPE)? != MSG_TYPE_MARKET_DATA_SNAPSHOT
        {
            return None;
        }

        let symbol = fields.get(&TAG_SYMBOL)?.clone();
        let mut bid_price = 0.0;
        let mut bid_qty = 0.0;
        let mut ask_price = 0.0;
        let mut ask_qty = 0.0;

        // Parse MD entries
        if let Some(num_entries) = fields.get(&TAG_NO_MD_ENTRIES)?.parse::<usize>().ok() {
            for i in 0..num_entries {
                if let Some(entry_type) = fields.get(&(TAG_MD_ENTRY_TYPE + i as u32))
                    && let Some(price) = fields.get(&(TAG_MD_ENTRY_PX + i as u32))
                    && let Some(size) = fields.get(&(TAG_MD_ENTRY_SIZE + i as u32)) {
                        let price_val = price.parse::<f64>().unwrap_or(0.0);
                        let size_val = size.parse::<f64>().unwrap_or(0.0);

                        match entry_type.chars().next() {
                            Some(MD_ENTRY_TYPE_BID) => {
                                bid_price = price_val;
                                bid_qty = size_val;
                            }
                            Some(MD_ENTRY_TYPE_OFFER) => {
                                ask_price = price_val;
                                ask_qty = size_val;
                            }
                            _ => {}
                        }
                    }
            }
        }

        Some(FixBookTicker {
            symbol,
            bid_price,
            bid_qty,
            ask_price,
            ask_qty,
        })
    }
}

// ============================================================================
// PRODUCTION FIX CONNECTION
// ============================================================================

async fn connect_fix_tls(
    endpoint: &str,
) -> Result<tokio_rustls::client::TlsStream<TcpStream>, Box<dyn std::error::Error + Send + Sync>> {
    // Parse endpoint
    let parts: Vec<&str> = endpoint.split(':').collect();
    let host = parts[0];
    let port: u16 = parts[1].parse()?;

    // Connect TCP
    let tcp_stream = TcpStream::connect((host, port)).await?;

    // Setup TLS
    let mut root_cert_store = rustls::RootCertStore::empty();
    root_cert_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

    let config = rustls::ClientConfig::builder()
        .with_root_certificates(root_cert_store)
        .with_no_client_auth();

    let connector = TlsConnector::from(Arc::new(config));
    let server_name = ServerName::try_from(host.to_string())?;

    // Perform TLS handshake
    let tls_stream = connector.connect(server_name, tcp_stream).await?;

    Ok(tls_stream)
}

async fn run_fix_session(
    api_key: String,
    private_key: Vec<u8>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let endpoint = "fix-md.binance.com:9000";

    println!(
        "Establishing TCP+TLS connection to {}...",
        endpoint.bright_white()
    );
    let mut stream = connect_fix_tls(endpoint).await?;

    let mut session = FixSession::new(api_key, private_key);

    // Send Logon message
    let logon = session.build_logon();
    println!("Sending FIX Logon message...");
    stream.write_all(logon.as_bytes()).await?;
    stream.flush().await?;

    // Read response
    let mut buffer = vec![0u8; 4096];
    let mut accumulated = Vec::new();
    let clock = Clock::new();
    let parser = FixMdParser::new();
    let mut parsing_latencies = Vec::new();
    let mut message_count = 0;
    let end_time = Instant::now() + Duration::from_secs(60);

    // Send market data subscription
    tokio::time::sleep(Duration::from_millis(100)).await;
    let market_data_req = session.build_market_data_request("BTCUSDT");
    println!("Subscribing to BTCUSDT market data...");
    stream.write_all(market_data_req.as_bytes()).await?;
    stream.flush().await?;

    println!("Collecting FIX market data for 60 seconds...");

    while Instant::now() < end_time {
        match tokio::time::timeout(Duration::from_secs(5), stream.read(&mut buffer)).await {
            Ok(Ok(n)) if n > 0 => {
                accumulated.extend_from_slice(&buffer[..n]);

                // Process complete FIX messages (look for start of message "8=FIX")
                while let Some(start_pos) = accumulated.windows(5).position(|w| w == b"8=FIX") {
                    // Find the checksum field (10=XXX)
                    if let Some(checksum_pos) = accumulated[start_pos..]
                        .windows(3)
                        .position(|w| w[0] == b'1' && w[1] == b'0' && w[2] == b'=')
                    {
                        // Find end of checksum (next SOH after 10=)
                        let checksum_start = start_pos + checksum_pos;
                        if let Some(end_offset) = accumulated[checksum_start + 3..]
                            .iter()
                            .position(|&b| b == SOH_BYTE)
                        {
                            let msg_end = checksum_start + 3 + end_offset + 1;
                            let fix_message =
                                String::from_utf8_lossy(&accumulated[start_pos..msg_end]);

                            // Parse and identify message type
                            let parse_start = clock.raw();
                            let fields = parser.parse_message(&fix_message);
                            let parse_end = clock.raw();
                            let parsing_ns = clock.delta(parse_start, parse_end).as_nanos() as u64;

                            parsing_latencies.push(parsing_ns);
                            message_count += 1;

                            // Log first few messages for debugging
                            if message_count <= 5
                                && let Some(msg_type) = fields.get(&TAG_MSG_TYPE) {
                                    let msg_type_name = match msg_type.as_str() {
                                        "A" => "Logon",
                                        "0" => "Heartbeat",
                                        "1" => "TestRequest",
                                        "3" => "Reject",
                                        "5" => "Logout",
                                        "W" => "MarketDataSnapshot",
                                        "X" => "MarketDataIncremental",
                                        "Y" => "MarketDataRequestReject",
                                        _ => "Unknown",
                                    };

                                    println!(
                                        "\n  Received FIX message type: {} ({})",
                                        msg_type, msg_type_name
                                    );

                                    // Show reject details if it's a reject message
                                    if msg_type == "3" {
                                        if let Some(text) = fields.get(&58) {
                                            // Tag 58 is Text field
                                            println!("  Reject reason: {}", text);
                                        }
                                        if let Some(ref_seq) = fields.get(&45) {
                                            // Tag 45 is RefSeqNum
                                            println!("  Rejected message seq: {}", ref_seq);
                                        }
                                        if let Some(ref_tag) = fields.get(&371) {
                                            // Tag 371 is RefTagID
                                            println!("  Problem with tag: {}", ref_tag);
                                        }
                                        if let Some(ref_msg_type) = fields.get(&372) {
                                            // Tag 372 is RefMsgType
                                            println!("  Rejected message type: {}", ref_msg_type);
                                        }
                                    }
                                }

                            // Print progress
                            if message_count % 10 == 0 {
                                print!(".");
                                use std::io::Write;
                                std::io::stdout().flush().unwrap();
                            }

                            // Remove processed message
                            accumulated.drain(..msg_end);
                        } else {
                            break; // Incomplete checksum
                        }
                    } else {
                        break; // Incomplete message
                    }
                }
            }
            Ok(Ok(0)) => {
                // Connection closed by server
                println!("\nConnection closed by server");
                break;
            }
            Ok(Ok(_)) => {
                // Received some bytes but less than expected, continue
                continue;
            }
            Ok(Err(e)) => {
                eprintln!("\nError reading from FIX stream: {}", e);
                break;
            }
            Err(_) => {
                // Timeout - send heartbeat to keep connection alive
                let heartbeat_msg = format!(
                    "8=FIX.4.4{}35=0{}49={}{}56={}{}34={}{}52={}{}",
                    SOH,
                    SOH,
                    session.sender_comp_id,
                    SOH,
                    session.target_comp_id,
                    SOH,
                    session.msg_seq_num,
                    SOH,
                    session.get_timestamp(),
                    SOH
                );
                let checksum = session.calculate_checksum(&heartbeat_msg);
                let full_heartbeat = format!("{}10={}{}", heartbeat_msg, checksum, SOH);

                if let Err(e) = stream.write_all(full_heartbeat.as_bytes()).await {
                    eprintln!("\nFailed to send heartbeat: {}", e);
                    break;
                }
                session.msg_seq_num += 1;

                // Print heartbeat indicator
                print!("♥");
                use std::io::Write;
                std::io::stdout().flush().unwrap();
            }
        }
    }

    // Send Logout
    println!("\nSending FIX Logout...");
    let logout = format!(
        "8=FIX.4.4{}35=5{}34={}{}52={}{}10=000{}",
        SOH,
        SOH,
        session.msg_seq_num,
        SOH,
        session.get_timestamp(),
        SOH,
        SOH
    );
    let _ = stream.write_all(logout.as_bytes()).await;

    // Print statistics
    println!("\n\n{}", "FIX PRODUCTION RESULTS:".green().bold());
    println!("{}", "=".repeat(60));

    if !parsing_latencies.is_empty() {
        let mut sorted = parsing_latencies.clone();
        sorted.sort_unstable();

        let mean = parsing_latencies.iter().sum::<u64>() as f64 / parsing_latencies.len() as f64;
        let min = sorted[0];
        let max = sorted[sorted.len() - 1];
        let p50 = sorted[sorted.len() / 2];
        let p90 = sorted[sorted.len() * 90 / 100];
        let p95 = sorted[sorted.len() * 95 / 100];
        let p99 = sorted[sorted.len() * 99 / 100];
        let p999 = sorted[(sorted.len() * 999 / 1000).min(sorted.len() - 1)];

        println!("Messages processed: {}", message_count.to_string().green());
        println!("Duration: 60 seconds");
        println!("Message rate: {:.1} msg/s", message_count as f64 / 60.0);

        println!("\n{}", "FIX Parsing Latency:".yellow());
        println!("  Mean:   {:>8.0} ns ({:.3} µs)", mean, mean / 1000.0);
        println!("  Min:    {:>8} ns ({:.3} µs)", min, min as f64 / 1000.0);
        println!("  Max:    {:>8} ns ({:.3} µs)", max, max as f64 / 1000.0);
        println!(
            "  P50:    {:>8} ns ({:.3} µs) (median)",
            p50,
            p50 as f64 / 1000.0
        );
        println!("  P90:    {:>8} ns ({:.3} µs)", p90, p90 as f64 / 1000.0);
        println!("  P95:    {:>8} ns ({:.3} µs)", p95, p95 as f64 / 1000.0);
        println!("  P99:    {:>8} ns ({:.3} µs)", p99, p99 as f64 / 1000.0);
        println!("  P99.9:  {:>8} ns ({:.3} µs)", p999, p999 as f64 / 1000.0);

        println!("\n{}", "Throughput:".green());
        let throughput = 1_000_000_000.0 / mean;
        println!(
            "  {:.1}M messages/second parsing capacity",
            throughput / 1_000_000.0
        );
    } else {
        println!("{}", "No messages received".yellow());
        println!("Possible issues:");
        println!("• FIX_API permission not enabled on API key");
        println!("• Incorrect market data subscription parameters");
        println!("• Authentication failed");
    }

    Ok(())
}

pub async fn run_fix_production_benchmark() {
    println!("{}", "=".repeat(80).bright_blue());
    println!(
        "{}",
        "FIX MARKET DATA PRODUCTION BENCHMARK".bright_white().bold()
    );
    println!(
        "{}",
        "Connecting to real FIX market data stream".bright_white()
    );
    println!("{}", "=".repeat(80).bright_blue());

    // Load credentials from .env file
    use std::fs;
    let env_content = fs::read_to_string(".env").expect("Failed to read .env file");
    let mut api_key = String::new();
    let mut private_key_env = String::new();

    for line in env_content.lines() {
        if line.starts_with("BINANCE_API_KEY=") {
            api_key = line
                .strip_prefix("BINANCE_API_KEY=")
                .unwrap_or("")
                .to_string();
        } else if line.starts_with("BINANCE_API_SECRET=") {
            private_key_env = line
                .strip_prefix("BINANCE_API_SECRET=")
                .unwrap_or("")
                .to_string();
        }
    }

    if api_key.is_empty() {
        eprintln!("Error: BINANCE_API_KEY not found in .env file");
        return;
    }
    if private_key_env.is_empty() {
        eprintln!("Error: BINANCE_API_SECRET not found in .env file");
        return;
    }

    println!(
        "DEBUG: Loaded API key from file: {}...{}",
        &api_key[..4.min(api_key.len())],
        &api_key[api_key.len().saturating_sub(4)..]
    );

    // Handle the private key - it might be double base64 encoded
    let private_key = if private_key_env.starts_with("LS0tLS1CRUdJTi") {
        // This is base64 of "-----BEGIN", so it's double-encoded
        match base64::decode(&private_key_env) {
            Ok(pem_bytes) => pem_bytes,
            Err(e) => {
                eprintln!("Failed to decode base64 private key: {}", e);
                return;
            }
        }
    } else if private_key_env.starts_with("-----BEGIN") {
        // Already PEM format
        private_key_env.into_bytes()
    } else {
        // Try as base64
        match base64::decode(&private_key_env) {
            Ok(key) => key,
            Err(_) => private_key_env.into_bytes(), // Use as-is
        }
    };

    println!("\n{}", "Configuration:".yellow().bold());
    println!("• Protocol: FIX 4.4");
    println!("• Endpoint: fix-md.binance.com:9000");
    println!(
        "• API Key: {}...{}",
        &api_key[..4],
        &api_key[api_key.len() - 4..]
    );
    println!("• Authentication: ED25519 signature");
    println!("• Duration: 60 seconds");

    // Connect to real FIX market data
    match run_fix_session(api_key, private_key).await {
        Ok(_) => {
            println!("\n{}", "FIX session completed successfully".green().bold());
        }
        Err(e) => {
            eprintln!("\n{}", format!("FIX connection error: {}", e).red().bold());
            eprintln!("Troubleshooting:");
            eprintln!("• Ensure FIX_API permission is enabled on your API key");
            eprintln!("• Verify ED25519 key is correctly configured");
            eprintln!("• Check network connectivity to fix-md.binance.com:9000");
        }
    }
}

// ============================================================================
// SYNTHETIC BENCHMARK
// ============================================================================

pub fn benchmark_fix_parsing(iterations: usize) -> (u64, u64, usize, usize) {
    println!("=== FIX Market Data Parsing Benchmark ===");

    let clock = Clock::new();
    let parser = FixMdParser::new();

    // Create sample FIX messages
    let book_ticker_fix = format!(
        "8=FIX.4.4{}35=X{}52=20240101-12:00:00.000{}55=BTCUSDT{}268=2{}269=0{}270=50000.00{}271=0.50000{}279=0{}269=1{}270=50001.00{}271=0.30000{}279=0{}",
        SOH, SOH, SOH, SOH, SOH, SOH, SOH, SOH, SOH, SOH, SOH, SOH, SOH
    );

    let depth_fix = format!(
        "8=FIX.4.4{}35=X{}52=20240101-12:00:00.000{}55=BTCUSDT{}268=6{}269=0{}270=50000.00{}271=0.50000{}279=0{}269=0{}270=49999.00{}271=1.00000{}279=0{}269=0{}270=49998.00{}271=0.75000{}279=0{}269=1{}270=50001.00{}271=0.30000{}279=0{}269=1{}270=50002.00{}271=0.80000{}279=0{}269=1{}270=50003.00{}271=0.25000{}279=0{}",
        SOH,
        SOH,
        SOH,
        SOH,
        SOH,
        SOH,
        SOH,
        SOH,
        SOH, // First bid
        SOH,
        SOH,
        SOH,
        SOH, // Second bid
        SOH,
        SOH,
        SOH,
        SOH, // Third bid
        SOH,
        SOH,
        SOH,
        SOH, // First ask
        SOH,
        SOH,
        SOH,
        SOH, // Second ask
        SOH,
        SOH,
        SOH,
        SOH // Third ask
    );

    // Benchmark bookTicker parsing
    let start = clock.raw();
    for _ in 0..iterations {
        let _ = parser.parse_message(&book_ticker_fix);
    }
    let end = clock.raw();
    let book_ticker_ns = clock.delta(start, end).as_nanos() as u64 / iterations as u64;

    // Benchmark depth parsing
    let start = clock.raw();
    for _ in 0..iterations {
        let _ = parser.parse_message(&depth_fix);
    }
    let end = clock.raw();
    let depth_ns = clock.delta(start, end).as_nanos() as u64 / iterations as u64;

    println!(
        "@bookTicker: {} ns/msg ({} bytes)",
        book_ticker_ns,
        book_ticker_fix.len()
    );
    println!(
        "@depth:      {} ns/msg ({} bytes)",
        depth_ns,
        depth_fix.len()
    );

    (
        book_ticker_ns,
        depth_ns,
        book_ticker_fix.len(),
        depth_fix.len(),
    )
}
