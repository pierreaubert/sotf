use sotf_audio::engine::playback_runtime_harness::run_fuzzer;

fn main() {
    let args = Args::parse(std::env::args().skip(1).collect());
    match run_fuzzer(args.seed, args.cases) {
        Ok(stats) => {
            println!(
                "playback-runtime-fuzzer ok: seed={}, cases={}, sequences={}, written={}, dropped={}, recycled={}, samples_written={}, events={}",
                args.seed,
                stats.cases,
                stats.sequences,
                stats.written,
                stats.dropped,
                stats.recycled,
                stats.samples_written,
                stats.events
            );
        }
        Err(err) => {
            eprintln!("playback-runtime-fuzzer failed: {err}");
            std::process::exit(1);
        }
    }
}

struct Args {
    seed: u64,
    cases: usize,
}

impl Args {
    fn parse(args: Vec<String>) -> Self {
        let mut seed = 1;
        let mut cases = 10_000;
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--seed" => {
                    i += 1;
                    seed = args
                        .get(i)
                        .and_then(|value| value.parse().ok())
                        .unwrap_or_else(|| usage("--seed requires a u64 value"));
                }
                "--cases" => {
                    i += 1;
                    cases = args
                        .get(i)
                        .and_then(|value| value.parse().ok())
                        .unwrap_or_else(|| usage("--cases requires a usize value"));
                }
                "-h" | "--help" => {
                    print_usage();
                    std::process::exit(0);
                }
                other => usage(&format!("unknown argument: {other}")),
            }
            i += 1;
        }
        Self { seed, cases }
    }
}

fn usage(message: &str) -> ! {
    eprintln!("{message}");
    print_usage();
    std::process::exit(2);
}

fn print_usage() {
    eprintln!("usage: playback-runtime-fuzzer [--seed <u64>] [--cases <usize>]");
}
