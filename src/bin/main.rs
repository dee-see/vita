extern crate vita;
use clap::{App, Arg};
use futures::stream::StreamExt;
use std::collections::HashSet;
use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use vita::error::Result;
use vita::{CleanExt, PostProcessor, Runner};

#[tokio::main]
async fn main() -> Result<()> {
    let ParsedArgs {
        runner,
        cleaner,
        flush,
        hosts,
    } = ParsedArgs::new(create_clap_app())?;
    let mut results: HashSet<String> = HashSet::new();

    let mut stream = runner.run(hosts).await?;
    while let Some(v) = stream.next().await {
        v.iter().clean(&cleaner).for_each(|r| {
            if flush {
                println!("{}", r);
            } else {
                results.insert(r);
            }
        });
    }

    if !flush {
        results.iter().for_each(|r| println!("{}", r));
    }

    Ok(())
}

struct ParsedArgs {
    runner: Runner,
    cleaner: PostProcessor,
    flush: bool,
    hosts: HashSet<String>,
}

impl ParsedArgs {
    fn new(app: clap::App<'static, 'static>) -> Result<Self> {
        let matches = app.get_matches();
        // make it a hashset incase user provided duplicate domains
        let mut hosts: HashSet<String> = HashSet::new();
        let max_concurrent: usize = matches.value_of("concurrency").unwrap().parse()?;
        let timeout: u64 = matches.value_of("timeout").unwrap().parse()?;

        if matches.is_present("verbosity") {
            let builder = tracing_subscriber::fmt()
                .with_env_filter(matches.value_of("verbosity").unwrap())
                .with_filter_reloading();
            let _handle = builder.reload_handle();
            builder.try_init()?;
        }

        if matches.is_present("file") {
            let input = matches.value_of("input").unwrap();
            hosts = read_input(Some(input))?;
        } else if matches.is_present("domain") {
            hosts.insert(matches.value_of("input").unwrap().to_string());
        } else {
            hosts = read_input(None)?;
        }

        let mut cleaner = PostProcessor::default();
        if matches.is_present("subs-only") {
            cleaner.any_subdomain(hosts.clone());
        } else {
            cleaner.any_root(hosts.clone());
        }

        let mut runner = Runner::default()
            .concurrency(max_concurrent)
            .timeout(timeout)
            .free_sources();
        if matches.is_present("all_sources") {
            runner = runner.all_sources();
        }

        let flush = matches.is_present("flush");

        Ok(Self {
            runner,
            cleaner,
            flush,
            hosts,
        })
    }
}
/// Reads input from stdin or a file
fn read_input(path: Option<&str>) -> Result<HashSet<String>> {
    let mut contents = HashSet::new();
    let reader: Box<dyn BufRead> = match path {
        Some(filepath) => {
            Box::new(BufReader::new(File::open(filepath).map_err(|e| {
                format!("tried to read filepath {} got {}", &filepath, e)
            })?))
        }
        None => Box::new(BufReader::new(io::stdin())),
    };

    for line in reader.lines() {
        contents.insert(line?);
    }

    Ok(contents)
}

/// Creates the Clap app to use Vita library as a cli tool
fn create_clap_app() -> clap::App<'static, 'static> {
    App::new("vita")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Gather subdomains from passive sources")
        .usage("vita -d <domain.com>")
        .arg(Arg::with_name("input").index(1).required(false))
        .arg(
            Arg::with_name("file")
                .help("vita -f <roots.txt>")
                .short("f")
                .long("file"),
        )
        .arg(
            Arg::with_name("domain")
                .help("vita -d domain.com")
                .short("d")
                .long("domain"),
        )
        .arg(
            Arg::with_name("all_sources")
                .help("use sources which require an Api key")
                .short("a")
                .long("all"),
        )
        .arg(
            Arg::with_name("subs-only")
                .help("filter the results to only those which have the same subdomain")
                .long("subs-only"),
        )
        .arg(
            Arg::with_name("flush")
                .help(
                    "Prints results to stdout as they're received. Results will still be filtered, 
                    but no deduplication will be done",
                )
                .long("flush"),
        )
        .arg(
            Arg::with_name("concurrency")
                .help("The number of domains to fetch data for concurrently")
                .short("c")
                .long("concurrency")
                .default_value("200")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("verbosity")
                .help(
                    "different levels of verbosity you can set for debugging, 
                    values include: debug,info and warn",
                )
                .short("v")
                .long("verbosity")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("timeout")
                .help(
                    "connection timeouts can be useful if you don't want to wait
                    for sources like wayback archive which quite a while. Default is 10 seconds.",
                )
                .short("t")
                .long("timeout")
                .default_value("15")
                .takes_value(true),
        )
}
