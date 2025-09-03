use colored::*;
use tokio::task::JoinHandle;
use std::time::Instant;

mod websocket_json;
mod websocket_sbe;
mod fix_md;

fn print_header() {
    println!("\n{}", "=".repeat(80).bright_blue());
    println!("{}", "CRYPTO MARKET DATA PARSING BENCHMARK".bright_white().bold());
    println!("{}", "Comparing parsing performance for Binance market data".bright_white());
    println!("{}", "=".repeat(80).bright_blue());
}

fn print_comparison_table(results: Vec<(&str, u64, u64, usize, usize)>) {
    println!("\n{}", "PERFORMANCE COMPARISON TABLE".green().bold());
    println!("{}", "-".repeat(80));
    println!("{:<20} {:<15} {:>12} {:>12} {:>12} {:>12}", 
             "Protocol", "Data Type", "Time (ns)", "Size (bytes)", "Throughput", "vs JSON");
    println!("{}", "-".repeat(80));
    
    // Find JSON baseline for comparison
    let json_book_ns = results.iter()
        .find(|(p, _, _, _, _)| p.contains("JSON"))
        .map(|(_, book, _, _, _)| *book)
        .unwrap_or(1);
    
    let json_depth_ns = results.iter()
        .find(|(p, _, _, _, _)| p.contains("JSON"))
        .map(|(_, _, depth, _, _)| *depth)
        .unwrap_or(1);
    
    for (protocol, book_ns, depth_ns, book_size, depth_size) in results {
        // BookTicker row
        let book_improvement = json_book_ns as f64 / book_ns.max(1) as f64;
        println!("{:<20} {:<15} {:>12} {:>12} {:>12} {:>12}", 
                 protocol,
                 "@bookTicker",
                 book_ns,
                 book_size,
                 format!("{:.1}M msg/s", 1_000_000_000.0 / book_ns as f64 / 1_000_000.0),
                 if book_improvement == 1.0 { 
                     "baseline".to_string() 
                 } else { 
                     format!("{:.1}x", book_improvement) 
                 });
        
        // Depth row
        let depth_improvement = json_depth_ns as f64 / depth_ns.max(1) as f64;
        println!("{:<20} {:<15} {:>12} {:>12} {:>12} {:>12}",
                 "",
                 "@depth",
                 depth_ns,
                 depth_size,
                 format!("{:.1}M msg/s", 1_000_000_000.0 / depth_ns as f64 / 1_000_000.0),
                 if depth_improvement == 1.0 { 
                     "baseline".to_string() 
                 } else { 
                     format!("{:.1}x", depth_improvement) 
                 });
        
        println!("{}", "-".repeat(80));
    }
}

fn print_insights() {
    println!("\n{}", "KEY INSIGHTS".cyan().bold());
    println!("• {} - Human readable, widely supported, slowest parsing", "WebSocket+JSON".yellow());
    println!("• {} - Binary format over WebSocket, zero-copy deserialization", "WebSocket+SBE".yellow());
    println!("• {} - Text-based protocol, institutional standard, moderate speed", "FIX Market Data".yellow());
    println!();
    println!("{}", "LATENCY BREAKDOWN".cyan().bold());
    println!("• Network latency: ~50-100ms (99.9% of total latency)");
    println!("• Parsing latency: ~100-1000ns (0.001% of total latency)");
    println!("• SBE advantage: Eliminates parsing entirely via zero-copy");
    println!();
    println!("{}", "RECOMMENDATIONS".cyan().bold());
    println!("• For ultra-low latency: Use SBE with colocation");
    println!("• For standard trading: JSON is sufficient (parsing is negligible)");
    println!("• For institutional: FIX provides standardization and audit trails");
}

#[tokio::main]
async fn main() {
    // Initialize the rustls crypto provider
    let _ = rustls::crypto::ring::default_provider().install_default();
    
    // Check command line arguments
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() > 1 {
        match args[1].as_str() {
            "json" => {
                // Run JSON production benchmark only
                websocket_json::run_production_benchmark().await;
                return;
            }
            "sbe" => {
                // Run SBE production benchmark only
                websocket_sbe::run_sbe_production_benchmark().await;
                return;
            }
            "fix" => {
                // Run FIX production benchmark only
                fix_md::run_fix_production_benchmark().await;
                return;
            }
            "quick" => {
                // Run quick synthetic benchmarks only
                run_quick_synthetic_benchmark();
                return;
            }
            _ => {}
        }
    }
    
    // Default: Run all production benchmarks in parallel
    print_header();
    
    println!("\n{}", "RUNNING FULL PRODUCTION BENCHMARK SUITE".bright_yellow().bold());
    println!("{}", "This will run all three protocols in parallel for 60 seconds each".bright_white());
    println!("{}", "=".repeat(80).bright_blue());
    
    let start_time = Instant::now();
    
    // Spawn all three benchmarks concurrently
    println!("\n{}", "Starting parallel benchmarks...".green().bold());
    println!("• WebSocket+JSON: Connecting to public streams");
    println!("• WebSocket+SBE: Requires API key (may fail without proper access)");
    println!("• FIX Market Data: Simulating parsing (full FIX requires institutional access)");
    
    // Create tasks for parallel execution
    let json_task: JoinHandle<()> = tokio::spawn(async move {
        println!("\n{}", "[JSON] Starting...".cyan());
        websocket_json::run_production_benchmark().await;
        println!("{}", "[JSON] Complete!".green());
    });
    
    let sbe_task: JoinHandle<()> = tokio::spawn(async move {
        println!("{}", "[SBE] Starting...".cyan());
        websocket_sbe::run_sbe_production_benchmark().await;
        println!("{}", "[SBE] Complete!".green());
    });
    
    let fix_task: JoinHandle<()> = tokio::spawn(async move {
        println!("{}", "[FIX] Starting...".cyan());
        fix_md::run_fix_production_benchmark().await;
        println!("{}", "[FIX] Complete!".green());
    });
    
    // Wait for all tasks to complete
    println!("\n{}", "Benchmarks running in parallel. This will take ~60 seconds...".yellow());
    println!("{}", "Progress indicators will appear as data is collected.".white());
    
    let _ = tokio::join!(json_task, sbe_task, fix_task);
    
    let elapsed = start_time.elapsed();
    
    // Print completion summary
    println!("\n{}", "=".repeat(80).bright_blue());
    println!("{}", "ALL BENCHMARKS COMPLETE".bright_white().bold());
    println!("{}", format!("Total time: {:.1} seconds", elapsed.as_secs_f64()).green());
    println!("{}", "=".repeat(80).bright_blue());
    
    // Run quick synthetic benchmark for comparison table
    println!("\n{}", "Running quick synthetic parsing benchmark for comparison...".yellow());
    let iterations = 100_000;  // Reduced for quick results
    
    let mut results = Vec::new();
    
    // Quick synthetic benchmarks
    let (json_book_ns, json_depth_ns, json_book_size, json_depth_size) = 
        websocket_json::benchmark_json_parsing(iterations);
    results.push(("WebSocket+JSON", json_book_ns, json_depth_ns, json_book_size, json_depth_size));
    
    let (sbe_book_ns, sbe_depth_ns, sbe_book_size, sbe_depth_size) = 
        websocket_sbe::benchmark_sbe_parsing(iterations);
    results.push(("WebSocket+SBE", sbe_book_ns, sbe_depth_ns, sbe_book_size, sbe_depth_size));
    
    let (fix_book_ns, fix_depth_ns, fix_book_size, fix_depth_size) = 
        fix_md::benchmark_fix_parsing(iterations);
    results.push(("FIX Market Data", fix_book_ns, fix_depth_ns, fix_book_size, fix_depth_size));
    
    // Print comparison table
    print_comparison_table(results);
    
    // Print insights
    print_insights();
    
    println!("\n{}", "=".repeat(80).bright_blue());
    println!("\n{}", "Available commands:".yellow().bold());
    println!("• {} - Run all production benchmarks (default)", "cargo run --release".bright_white());
    println!("• {} - Run only quick synthetic benchmarks", "cargo run --release quick".bright_white());
    println!("• {} - Test only JSON streams", "cargo run --release json".bright_white());
    println!("• {} - Test only SBE streams (requires API key)", "cargo run --release sbe".bright_white());
    println!("• {} - Test only FIX market data", "cargo run --release fix".bright_white());
}

fn run_quick_synthetic_benchmark() {
    print_header();
    
    let iterations = 1_000_000;
    println!("\nRunning {} iterations per test...\n", iterations.to_string().bright_yellow());
    
    let mut results = Vec::new();
    
    // Test WebSocket + JSON
    let (json_book_ns, json_depth_ns, json_book_size, json_depth_size) = 
        websocket_json::benchmark_json_parsing(iterations);
    results.push(("WebSocket+JSON", json_book_ns, json_depth_ns, json_book_size, json_depth_size));
    
    println!();
    
    // Test WebSocket + SBE
    let (sbe_book_ns, sbe_depth_ns, sbe_book_size, sbe_depth_size) = 
        websocket_sbe::benchmark_sbe_parsing(iterations);
    results.push(("WebSocket+SBE", sbe_book_ns, sbe_depth_ns, sbe_book_size, sbe_depth_size));
    
    println!();
    
    // Test FIX Market Data
    let (fix_book_ns, fix_depth_ns, fix_book_size, fix_depth_size) = 
        fix_md::benchmark_fix_parsing(iterations);
    results.push(("FIX Market Data", fix_book_ns, fix_depth_ns, fix_book_size, fix_depth_size));
    
    // Print comparison table
    print_comparison_table(results);
    
    // Print insights
    print_insights();
    
    println!("\n{}", "=".repeat(80).bright_blue());
}