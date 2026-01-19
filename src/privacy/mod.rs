use axum::http::HeaderMap;
use ipnetwork::IpNetwork;
use std::net::IpAddr;

/// Check if DNT (Do Not Track) or GPC (Global Privacy Control) is enabled
pub fn is_dnt_enabled(headers: &HeaderMap) -> bool {
    let dnt = headers
        .get("dnt")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim() == "1")
        .unwrap_or(false);

    let gpc = headers
        .get("sec-gpc")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim() == "1")
        .unwrap_or(false);

    dnt || gpc
}

/// Check if an IP address should be ignored based on CIDR list
pub fn is_ip_ignored(ip: &str, ignored_networks: &[IpNetwork]) -> bool {
    if ignored_networks.is_empty() {
        return false;
    }

    let ip_addr: IpAddr = match ip.parse() {
        Ok(addr) => addr,
        Err(_) => return false,
    };

    for network in ignored_networks {
        if network.contains(ip_addr) {
            return true;
        }
    }

    false
}

/// Parse a comma-separated list of CIDR networks
pub fn parse_ignored_networks(networks_str: &str) -> Vec<IpNetwork> {
    if networks_str.trim().is_empty() {
        return Vec::new();
    }

    networks_str
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect()
}

/// Extract client IP from headers (supports common proxy headers)
pub fn get_client_ip(headers: &HeaderMap) -> Option<String> {
    // Check X-Forwarded-For first (most common)
    if let Some(xff) = headers.get("x-forwarded-for") {
        if let Ok(xff_str) = xff.to_str() {
            // Take the first IP (client IP in a proxy chain)
            if let Some(first_ip) = xff_str.split(',').next() {
                let ip = first_ip.trim();
                if !ip.is_empty() {
                    return Some(ip.to_string());
                }
            }
        }
    }

    // Check X-Real-IP (Nginx)
    if let Some(real_ip) = headers.get("x-real-ip") {
        if let Ok(ip) = real_ip.to_str() {
            let ip = ip.trim();
            if !ip.is_empty() {
                return Some(ip.to_string());
            }
        }
    }

    // Check CF-Connecting-IP (Cloudflare)
    if let Some(cf_ip) = headers.get("cf-connecting-ip") {
        if let Ok(ip) = cf_ip.to_str() {
            let ip = ip.trim();
            if !ip.is_empty() {
                return Some(ip.to_string());
            }
        }
    }

    // Check True-Client-IP (Akamai, Cloudflare Enterprise)
    if let Some(true_ip) = headers.get("true-client-ip") {
        if let Ok(ip) = true_ip.to_str() {
            let ip = ip.trim();
            if !ip.is_empty() {
                return Some(ip.to_string());
            }
        }
    }

    None
}

/// Get the user agent string from headers
pub fn get_user_agent(headers: &HeaderMap) -> String {
    headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Get the referrer URL from headers
pub fn get_referrer(headers: &HeaderMap) -> String {
    headers
        .get("referer")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Get the origin from headers (for CORS validation)
pub fn get_origin(headers: &HeaderMap) -> Option<String> {
    // First try Origin header
    if let Some(origin) = headers.get("origin") {
        if let Ok(origin_str) = origin.to_str() {
            let origin = origin_str.trim();
            if !origin.is_empty() {
                return Some(origin.to_lowercase());
            }
        }
    }

    // Fall back to Referer header
    if let Some(referer) = headers.get("referer") {
        if let Ok(referer_str) = referer.to_str() {
            // Parse origin from referer URL
            if let Ok(url) = url::Url::parse(referer_str) {
                if let Some(host) = url.host_str() {
                    let port = url.port().map(|p| format!(":{}", p)).unwrap_or_default();
                    return Some(format!("{}://{}{}", url.scheme(), host, port).to_lowercase());
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn test_dnt_disabled_by_default() {
        let headers = HeaderMap::new();
        assert!(!is_dnt_enabled(&headers));
    }

    #[test]
    fn test_dnt_enabled_with_dnt_header() {
        let mut headers = HeaderMap::new();
        headers.insert("dnt", HeaderValue::from_static("1"));
        assert!(is_dnt_enabled(&headers));
    }

    #[test]
    fn test_dnt_disabled_with_zero() {
        let mut headers = HeaderMap::new();
        headers.insert("dnt", HeaderValue::from_static("0"));
        assert!(!is_dnt_enabled(&headers));
    }

    #[test]
    fn test_gpc_enabled() {
        let mut headers = HeaderMap::new();
        headers.insert("sec-gpc", HeaderValue::from_static("1"));
        assert!(is_dnt_enabled(&headers));
    }

    #[test]
    fn test_ip_ignored_in_cidr() {
        let networks = parse_ignored_networks("192.168.1.0/24, 10.0.0.0/8");

        assert!(is_ip_ignored("192.168.1.100", &networks));
        assert!(is_ip_ignored("192.168.1.1", &networks));
        assert!(is_ip_ignored("192.168.1.255", &networks));
        assert!(is_ip_ignored("10.50.100.200", &networks));
        assert!(is_ip_ignored("10.0.0.1", &networks));
    }

    #[test]
    fn test_ip_not_ignored_outside_cidr() {
        let networks = parse_ignored_networks("192.168.1.0/24");

        assert!(!is_ip_ignored("192.168.2.1", &networks));
        assert!(!is_ip_ignored("8.8.8.8", &networks));
        assert!(!is_ip_ignored("172.16.0.1", &networks));
    }

    #[test]
    fn test_ip_not_ignored_with_empty_networks() {
        let networks = parse_ignored_networks("");
        assert!(!is_ip_ignored("192.168.1.100", &networks));
    }

    #[test]
    fn test_ip_not_ignored_with_invalid_ip() {
        let networks = parse_ignored_networks("192.168.1.0/24");
        assert!(!is_ip_ignored("not-an-ip", &networks));
    }

    #[test]
    fn test_parse_ignored_networks() {
        let networks = parse_ignored_networks("192.168.1.0/24, 10.0.0.0/8, 172.16.0.0/12");
        assert_eq!(networks.len(), 3);
    }

    #[test]
    fn test_parse_ignored_networks_empty() {
        let networks = parse_ignored_networks("");
        assert!(networks.is_empty());
    }

    #[test]
    fn test_parse_ignored_networks_invalid_entries() {
        let networks = parse_ignored_networks("192.168.1.0/24, invalid, 10.0.0.0/8");
        assert_eq!(networks.len(), 2); // Only valid ones
    }

    #[test]
    fn test_get_client_ip_x_forwarded_for() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("203.0.113.195, 70.41.3.18, 150.172.238.178"),
        );
        assert_eq!(
            get_client_ip(&headers),
            Some("203.0.113.195".to_string())
        );
    }

    #[test]
    fn test_get_client_ip_x_real_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", HeaderValue::from_static("192.168.1.100"));
        assert_eq!(
            get_client_ip(&headers),
            Some("192.168.1.100".to_string())
        );
    }

    #[test]
    fn test_get_client_ip_cf_connecting_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("cf-connecting-ip", HeaderValue::from_static("104.28.1.1"));
        assert_eq!(
            get_client_ip(&headers),
            Some("104.28.1.1".to_string())
        );
    }

    #[test]
    fn test_get_client_ip_true_client_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("true-client-ip", HeaderValue::from_static("8.8.8.8"));
        assert_eq!(get_client_ip(&headers), Some("8.8.8.8".to_string()));
    }

    #[test]
    fn test_get_client_ip_none() {
        let headers = HeaderMap::new();
        assert_eq!(get_client_ip(&headers), None);
    }

    #[test]
    fn test_get_user_agent() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "user-agent",
            HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64)"),
        );
        assert_eq!(
            get_user_agent(&headers),
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64)"
        );
    }

    #[test]
    fn test_get_user_agent_empty() {
        let headers = HeaderMap::new();
        assert_eq!(get_user_agent(&headers), "");
    }

    #[test]
    fn test_get_referrer() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "referer",
            HeaderValue::from_static("https://example.com/page"),
        );
        assert_eq!(get_referrer(&headers), "https://example.com/page");
    }

    #[test]
    fn test_get_referrer_empty() {
        let headers = HeaderMap::new();
        assert_eq!(get_referrer(&headers), "");
    }

    #[test]
    fn test_get_origin_from_origin_header() {
        let mut headers = HeaderMap::new();
        headers.insert("origin", HeaderValue::from_static("https://example.com"));
        assert_eq!(get_origin(&headers), Some("https://example.com".to_string()));
    }

    #[test]
    fn test_get_origin_from_referer() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "referer",
            HeaderValue::from_static("https://example.com/path/to/page"),
        );
        assert_eq!(get_origin(&headers), Some("https://example.com".to_string()));
    }

    #[test]
    fn test_get_origin_none() {
        let headers = HeaderMap::new();
        assert_eq!(get_origin(&headers), None);
    }
}
