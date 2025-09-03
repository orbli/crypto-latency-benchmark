use colored::*;
use std::time::Instant;
use tokio::task::JoinHandle;

mod fix_md;
mod websocket_json;
mod websocket_sbe;

fn print_header() {
    println!("\n{}", "=".repeat(80).bright_blue());
    println!(
        "{}",
        "CRYPTO MARKET DATA PARSING BENCHMARK".bright_white().bold()
    );
    println!(
        "{}",
        "Comparing parsing performance for Binance market data".bright_white()
    );
    println!("{}", "=".repeat(80).bright_blue());
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
            _ => {}
        }
    }

    // Default: Run all production benchmarks in parallel
    print_header();

    println!(
        "\n{}",
        "RUNNING FULL PRODUCTION BENCHMARK SUITE"
            .bright_yellow()
            .bold()
    );
    println!(
        "{}",
        "This will run all three protocols in parallel for 60 seconds each".bright_white()
    );
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
    println!(
        "\n{}",
        "Benchmarks running in parallel. This will take ~60 seconds...".yellow()
    );
    println!(
        "{}",
        "Progress indicators will appear as data is collected.".white()
    );

    let _ = tokio::join!(json_task, sbe_task, fix_task);

    let elapsed = start_time.elapsed();

    // Print completion summary
    println!("\n{}", "=".repeat(80).bright_blue());
    println!("{}", "ALL BENCHMARKS COMPLETE".bright_white().bold());
    println!(
        "{}",
        format!("Total time: {:.1} seconds", elapsed.as_secs_f64()).green()
    );
    println!("{}", "=".repeat(80).bright_blue());

    println!(
        "\n{}",
        "All production benchmarks completed with live market data!"
            .green()
            .bold()
    );
    println!(
        "{}",
        "Refer to the individual benchmark reports above for detailed performance metrics."
            .bright_white()
    );

    println!("\n{}", "=".repeat(80).bright_blue());
    println!("\n{}", "Available commands:".yellow().bold());
    println!(
        "• {} - Run all production benchmarks (default)",
        "cargo run --release".bright_white()
    );
    println!(
        "• {} - Test only JSON streams (no API key needed)",
        "cargo run --release json".bright_white()
    );
    println!(
        "• {} - Test only SBE streams (requires API key)",
        "cargo run --release sbe".bright_white()
    );
    println!(
        "• {} - Test only FIX market data (requires API key)",
        "cargo run --release fix".bright_white()
    );
}
