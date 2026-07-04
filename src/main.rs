use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    let seed = args.iter()
        .position(|a| a == "--seed")
        .and_then(|i| args.get(i + 1))
        .unwrap_or(&"0".to_string())
        .clone();

    println!("Planets Craft — Beta (placeholder) — seed {}", seed);
    println!("Generating planet... (this is a lightweight CI test binary)");

    // Simulate a small workload so the CI run produces some output
    for i in 0..5 {
        println!("Step {}: processing...", i + 1);
    }

    println!("Generation complete.");
}
