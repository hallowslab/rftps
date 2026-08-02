use rcgen::{CertificateParams, DistinguishedName, DnType, IsCa, KeyPair, SanType};
use std::net::IpAddr;
use std::path::Path;
use std::process::exit;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!(
            "Usage: cargo run --example gen_cert -- <output_dir> [ip1] [ip2] ... [hostname1] ..."
        );
        eprintln!("  e.g. cargo run --example gen_cert -- C:\\RFTPS 192.168.1.123 127.0.0.1 localhost");
        exit(1);
    }
    let out_dir = &args[1];

    let mut san_dns: Vec<String> = vec!["localhost".to_string()];
    let mut san_ip: Vec<IpAddr> = vec!["127.0.0.1".parse().unwrap()];

    for arg in &args[2..] {
        match arg.parse::<IpAddr>() {
            Ok(ip) => san_ip.push(ip),
            Err(_) => san_dns.push(arg.clone()),
        }
    }

    let mut params = CertificateParams::new(san_dns).expect("failed to build cert params");
    params.is_ca = IsCa::NoCa;
    params.distinguished_name = DistinguishedName::new();
    params
        .distinguished_name
        .push(DnType::CommonName, "rftps");
    params.not_before = rcgen::date_time_ymd(2026, 7, 15);
    params.not_after = rcgen::date_time_ymd(2036, 7, 15);
    params.subject_alt_names = san_ip.into_iter().map(SanType::IpAddress).collect();

    let key_pair = KeyPair::generate().expect("failed to generate key");
    let cert = params.self_signed(&key_pair).expect("failed to self-sign cert");

    let cert_path = Path::new(out_dir).join("cert.pem");
    let key_path = Path::new(out_dir).join("key.pem");
    std::fs::write(&cert_path, cert.pem()).expect("failed to write cert.pem");
    std::fs::write(&key_path, key_pair.serialize_pem()).expect("failed to write key.pem");

    println!("Wrote {}", cert_path.display());
    println!("Wrote {}", key_path.display());
}
