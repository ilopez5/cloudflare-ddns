use anyhow::{Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::env;
use std::net::{IpAddr, ToSocketAddrs};

#[derive(Parser)]
struct Cli {
    domain: String,
}

#[derive(Deserialize)]
struct ZonesResponse {
    result: Vec<Zone>,
}

#[derive(Deserialize)]
struct DnsRecordsResponse {
    result: Vec<DnsRecord>,
}

#[derive(Deserialize)]
struct PatchDnsRecordsResponse {
    success: bool,
}

#[derive(Deserialize)]
struct Zone {
    id: String,
    name: String,
}

#[derive(Deserialize, Serialize)]
struct DnsRecord {
    id: String,
    zone_id: String,
    name: String,
    r#type: String,
    content: String,
    ttl: isize,
}

fn main() -> Result<()> {
    let args: Cli = Cli::parse();
    let domain: &str = &args.domain;
    let current_ip: IpAddr = public_ip().context("Getting current public IPv4 Address")?;
    let dns_ip: IpAddr = dns_ip(domain).context("Getting IPv4 Address Associated With Domain")?;

    if current_ip == dns_ip {
        println!("IPv4 Address matches. Exiting.");
        return Ok(());
    }

    println!("IPv4 Address does not match. Updating Cloudflare DNS records.");

    let api_key: String = env::var("CLOUDFLARE_API_KEY")?;

    let zone_id: String = get_zones(&api_key)?
        .into_iter()
        .find(|zone| zone.name == domain)
        .map(|zone| zone.id)
        .context("Getting Zone for Domain")?;

    let mut dns_record: DnsRecord = get_dns_records(&api_key, &zone_id)?
        .into_iter()
        .find(|record| record.r#type == "A" && record.name == domain)
        .context("Getting A Record")?;
    dns_record.content = current_ip.to_string();

    let outcome: bool = patch_dns_record(&api_key, &dns_record)
        .context("Patching DNS Record")?
        .success;
    if outcome {
        println!("Updated DNS Record from {dns_ip} to {current_ip}");
    }

    Ok(())
}

/// Gets the current public IPv4 address.
///
/// GET request is made to <https://api.ipify.org>.
fn public_ip() -> Result<IpAddr, ureq::Error> {
    let ip: IpAddr = ureq::get("https://api.ipify.org")
        .call()?
        .into_string()?
        .parse()
        .unwrap();
    Ok(ip)
}

/// Performs a DNS lookup on `domain` and returns first result.
fn dns_ip(domain: &str) -> Result<IpAddr, anyhow::Error> {
    let ip: IpAddr = format!("{domain}:80")
        .to_socket_addrs()?
        .next()
        .context("Getting IPv4 Address For Domain")?
        .ip();
    Ok(ip)
}

/// Gets all Cloudflare zones for associated with the `api_key`.
///
/// Uses Cloudflare v4 API. See <https://developers.cloudflare.com/api/operations/zones-get>.
fn get_zones(api_key: &str) -> Result<Vec<Zone>, ureq::Error> {
    let auth: &str = &format!("Bearer {api_key}");
    let url: &str = "https://api.cloudflare.com/client/v4/zones";
    let response: ZonesResponse = ureq::get(url)
        .set("Authorization", auth)
        .set("Content-Type", "application/json")
        .call()?
        .into_json()?;
    Ok(response.result)
}

/// Gets all DNS records associated with Zone ID `zone_id`.
///
/// Uses Cloudflare v4 API. See <https://developers.cloudflare.com/api/operations/dns-records-for-a-zone-list-dns-records>.
fn get_dns_records(api_key: &str, zone_id: &str) -> Result<Vec<DnsRecord>, ureq::Error> {
    let auth: &str = &format!("Bearer {api_key}");
    let url: &str = &format!("https://api.cloudflare.com/client/v4/zones/{zone_id}/dns_records");
    let response: DnsRecordsResponse = ureq::get(url)
        .set("Authorization", auth)
        .set("Content-Type", "application/json")
        .call()?
        .into_json()?;
    Ok(response.result)
}

/// Updates DNS record `dns_record`.
///
/// In this script, the `content` field of an A Record is updated to match the new IPv4 Address.
///
/// Uses Cloudflare v4 API. See <https://developers.cloudflare.com/api/operations/dns-records-for-a-zone-patch-dns-record>
fn patch_dns_record(
    api_key: &str,
    dns_record: &DnsRecord,
) -> Result<PatchDnsRecordsResponse, ureq::Error> {
    let auth: &str = &format!("Bearer {api_key}");
    let url: &str = &format!(
        "https://api.cloudflare.com/client/v4/zones/{}/dns_records/{}",
        dns_record.zone_id, dns_record.id
    );
    let response: PatchDnsRecordsResponse = ureq::patch(url)
        .set("Authorization", auth)
        .set("Content-Type", "application/json")
        .send_json(ureq::json!(dns_record))?
        .into_json()?;
    Ok(response)
}
