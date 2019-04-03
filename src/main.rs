#[macro_use]
extern crate clap;
#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde;

error_chain!();

quick_main!(run);

mod server;

fn run() -> Result<()> {
    env_logger::init();

    let cli_arg_matches = clap::App::new(crate_name!())
        .version(crate_version!())
        .about(crate_description!())
        .author(crate_authors!())
        .arg_from_usage("-p --port=[port] 'TCP/IP port the server will listen on (default: 8080)'")
        .arg_from_usage("-c --config=[config] 'Server configuration file (default: ~/.config/cds_server.json)'")
        .get_matches();

    let config_path = match cli_arg_matches.value_of("config") {
        Some(c) => std::path::PathBuf::from(c),
        None    => {
            match dirs::home_dir() {
                Some(h) => h.join(".config/cds_server.json"),
                None    => bail!("Unable to locate home directory to load server config! You might work around this by explicitly specifying the server's config file."),
            }
        }
    };
    let port = cli_arg_matches.value_of("port")
        .unwrap_or("8080")
        .parse()
        .chain_err(|| "Unable to parse given port")?;

    let server = server::Server::new(&config_path.as_path(), port)?;
    let _ = server.run()?;

    Ok(())
}