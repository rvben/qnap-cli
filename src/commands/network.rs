use anyhow::Result;
use serde::Serialize;

use crate::client::{QnapClient, extract_xml_value, parse_xml};
use crate::output::print_kv;

#[derive(Debug, Serialize)]
pub struct NicInfo {
    pub name: String,
    pub display_name: String,
    pub ip: Option<String>,
    pub mask: Option<String>,
    pub mac: Option<String>,
    pub status: String,
    pub speed_mbps: Option<u64>,
    pub mode: Option<String>,
    pub rx_packets: Option<u64>,
    pub tx_packets: Option<u64>,
    pub err_packets: Option<u64>,
}

#[derive(Debug, Serialize)]
struct NetworkOutput {
    adapters: Vec<NicInfo>,
    dns: Vec<String>,
}

fn parse_u64(body: &str, tag: &str) -> Option<u64> {
    extract_xml_value(body, tag)?.trim().parse().ok()
}

fn parse_nics(body: &str) -> Vec<NicInfo> {
    let count = parse_u64(body, "nic_cnt").unwrap_or(0) as usize;
    let mut nics = Vec::with_capacity(count);

    for i in 1..=count {
        let name = extract_xml_value(body, &format!("ifname{}", i))
            .unwrap_or_else(|| format!("eth{}", i - 1));
        let display_name = extract_xml_value(body, &format!("dname{}", i))
            .unwrap_or_else(|| format!("Adapter {}", i));
        let ip = extract_xml_value(body, &format!("eth_ip{}", i));
        let mask = extract_xml_value(body, &format!("eth_mask{}", i));
        let mac = extract_xml_value(body, &format!("eth_mac{}", i));
        let status = match parse_u64(body, &format!("eth_status{}", i)) {
            Some(1) => "up".to_string(),
            Some(0) => "down".to_string(),
            _ => "unknown".to_string(),
        };
        let speed_mbps = parse_u64(body, &format!("eth_max_speed{}", i));
        let mode = extract_xml_value(body, &format!("eth_usage{}", i));
        let rx_packets = parse_u64(body, &format!("rx_packet{}", i));
        let tx_packets = parse_u64(body, &format!("tx_packet{}", i));
        let err_packets = parse_u64(body, &format!("err_packet{}", i));

        nics.push(NicInfo {
            name,
            display_name,
            ip,
            mask,
            mac,
            status,
            speed_mbps,
            mode,
            rx_packets,
            tx_packets,
            err_packets,
        });
    }

    nics
}

fn parse_dns(body: &str) -> Vec<String> {
    // DNS_LIST tags appear under <dnsInfo> — collect all occurrences
    let doc = match parse_xml(body) {
        Ok(d) => d,
        Err(_) => return vec![],
    };
    doc.descendants()
        .filter(|n| n.has_tag_name("DNS_LIST"))
        .filter_map(|n| {
            let v = n.text().unwrap_or_default().trim().to_string();
            if v.is_empty() { None } else { Some(v) }
        })
        .collect()
}

pub async fn run(client: &QnapClient, json: bool) -> Result<()> {
    let body = client
        .get_cgi(
            "/cgi-bin/management/manaRequest.cgi",
            &[("subfunc", "sysinfo")],
        )
        .await?;

    let nics = parse_nics(&body);
    let dns = parse_dns(&body);

    if json {
        let output = NetworkOutput {
            adapters: nics,
            dns,
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    for nic in &nics {
        println!("  {}", nic.display_name);
        let mut pairs: Vec<(String, String)> = vec![
            ("interface".into(), nic.name.clone()),
            ("status".into(), nic.status.clone()),
        ];
        if let Some(ip) = &nic.ip {
            pairs.push(("ip".into(), ip.clone()));
        }
        if let Some(mask) = &nic.mask {
            pairs.push(("mask".into(), mask.clone()));
        }
        if let Some(mac) = &nic.mac {
            pairs.push(("mac".into(), mac.clone()));
        }
        if let Some(speed) = nic.speed_mbps {
            pairs.push(("speed".into(), format!("{} Mbps", speed)));
        }
        if let Some(mode) = &nic.mode {
            pairs.push(("mode".into(), mode.clone()));
        }
        if let (Some(rx), Some(tx)) = (nic.rx_packets, nic.tx_packets) {
            pairs.push(("rx_packets".into(), rx.to_string()));
            pairs.push(("tx_packets".into(), tx.to_string()));
        }
        print_kv(&pairs);
        println!();
    }

    if !dns.is_empty() {
        println!("  DNS");
        for (i, entry) in dns.iter().enumerate() {
            println!("    {}", entry);
            let _ = i;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"<QDocRoot>
        <nic_cnt><![CDATA[2]]></nic_cnt>
        <ifname1>eth0</ifname1>
        <dname1>Adapter 1</dname1>
        <eth_ip1>192.168.1.50</eth_ip1>
        <eth_mask1>255.255.255.0</eth_mask1>
        <eth_mac1>aa:bb:cc:dd:ee:ff</eth_mac1>
        <eth_status1>1</eth_status1>
        <eth_max_speed1>1000</eth_max_speed1>
        <eth_usage1>DHCP</eth_usage1>
        <rx_packet1>1000000</rx_packet1>
        <tx_packet1>500000</tx_packet1>
        <err_packet1>0</err_packet1>
        <ifname2>eth1</ifname2>
        <dname2>Adapter 2</dname2>
        <eth_ip2>0.0.0.0</eth_ip2>
        <eth_status2>0</eth_status2>
        <eth_max_speed2>1000</eth_max_speed2>
        <dnsInfo>
            <DNS_LIST><![CDATA[192.168.1.1]]></DNS_LIST>
            <DNS_LIST><![CDATA[8.8.8.8]]></DNS_LIST>
        </dnsInfo>
    </QDocRoot>"#;

    #[test]
    fn parse_nics_count() {
        let nics = parse_nics(FIXTURE);
        assert_eq!(nics.len(), 2);
    }

    #[test]
    fn parse_nics_first_adapter() {
        let nics = parse_nics(FIXTURE);
        assert_eq!(nics[0].name, "eth0");
        assert_eq!(nics[0].status, "up");
        assert_eq!(nics[0].ip.as_deref(), Some("192.168.1.50"));
        assert_eq!(nics[0].mac.as_deref(), Some("aa:bb:cc:dd:ee:ff"));
        assert_eq!(nics[0].speed_mbps, Some(1000));
        assert_eq!(nics[0].mode.as_deref(), Some("DHCP"));
        assert_eq!(nics[0].rx_packets, Some(1000000));
    }

    #[test]
    fn parse_nics_second_adapter_down() {
        let nics = parse_nics(FIXTURE);
        assert_eq!(nics[1].name, "eth1");
        assert_eq!(nics[1].status, "down");
    }

    #[test]
    fn parse_dns_entries() {
        let dns = parse_dns(FIXTURE);
        assert_eq!(dns, vec!["192.168.1.1", "8.8.8.8"]);
    }
}
