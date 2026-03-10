use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct DnsInfo {
    pub a_records: Vec<String>,
    pub aaaa_records: Vec<String>,
    pub mx_records: Vec<MxRecord>,
    pub txt_records: Vec<String>,
    pub ns_records: Vec<String>,
    pub cname_records: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MxRecord {
    pub priority: u16,
    pub exchange: String,
}

/// Perform DNS lookups for a domain.
pub async fn lookup(domain: &str) -> Result<DnsInfo, Box<dyn std::error::Error + Send + Sync>> {
    use hickory_resolver::TokioAsyncResolver;
    use hickory_resolver::config::*;

    let resolver = TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default());

    let a_records = resolver
        .ipv4_lookup(domain)
        .await
        .map(|r| r.iter().map(|ip| ip.to_string()).collect())
        .unwrap_or_default();

    let aaaa_records = resolver
        .ipv6_lookup(domain)
        .await
        .map(|r| r.iter().map(|ip| ip.to_string()).collect())
        .unwrap_or_default();

    let mx_records = resolver
        .mx_lookup(domain)
        .await
        .map(|r| {
            r.iter()
                .map(|mx| MxRecord {
                    priority: mx.preference(),
                    exchange: mx.exchange().to_string(),
                })
                .collect()
        })
        .unwrap_or_default();

    let txt_records = resolver
        .txt_lookup(domain)
        .await
        .map(|r| {
            r.iter()
                .map(|txt| txt.to_string())
                .collect()
        })
        .unwrap_or_default();

    let ns_records = resolver
        .ns_lookup(domain)
        .await
        .map(|r| r.iter().map(|ns| ns.to_string()).collect())
        .unwrap_or_default();

    let cname_records = resolver
        .lookup(domain, hickory_resolver::proto::rr::RecordType::CNAME)
        .await
        .map(|r| r.iter().map(|rd| rd.to_string()).collect())
        .unwrap_or_default();

    Ok(DnsInfo {
        a_records,
        aaaa_records,
        mx_records,
        txt_records,
        ns_records,
        cname_records,
    })
}
