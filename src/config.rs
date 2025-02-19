use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Server address
    #[arg(short, long, default_value = "0.0.0.0")]
    pub address: String,

    /// Port to run the FTP server on
    #[arg(short, long, default_value = "2121")]
    pub port: u16,

    /// Directory where uploaded files will be stored
    #[arg(short, long, default_value = "./rftps")]
    pub directory: String,

    /// Username for FTP server
    #[arg(short, long, default_value= "rftps")]
    pub username: String,

    /// Password for the FTP user
    #[arg(long)]
    pub password: Option<String>,
}
