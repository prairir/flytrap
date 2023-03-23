use clap::Parser;
use std::{
    fs::File,
    io::{stdout, Write},
};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = String::from("ryanprairie.com"))] // TODO: get better root
    root: String,

    #[arg(short, long, default_value_t = String::from("web-graph.txt"))]
    output: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    println!("Hello, {:?}", args);

    match args.output.as_str() {
        "-" => flytrap(args.root, &mut stdout()).await,
        _ => {
            let mut f = File::create(args.output).expect("file not found");
            flytrap(args.root, &mut f).await;
        }
    }
}

async fn flytrap(_root_url: String, out: &mut impl Write) {
    out.write("hi".as_bytes()).expect("whoops");
}
