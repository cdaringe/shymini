use maxminddb::{geoip2, Reader};
use std::net::IpAddr;
use std::path::Path;
use tracing::{debug, warn};

use crate::error::Result;

#[derive(Debug, Default)]
pub struct GeoIpData {
    pub asn: String,
    pub country: String,
    pub longitude: Option<f64>,
    pub latitude: Option<f64>,
    pub time_zone: String,
}

pub struct GeoIpLookup {
    city_reader: Option<Reader<Vec<u8>>>,
    asn_reader: Option<Reader<Vec<u8>>>,
}

impl GeoIpLookup {
    pub fn new(city_db_path: Option<&str>, asn_db_path: Option<&str>) -> Result<Self> {
        let city_reader = if let Some(path) = city_db_path {
            if Path::new(path).exists() {
                match Reader::open_readfile(path) {
                    Ok(reader) => {
                        debug!("Loaded GeoIP city database from {}", path);
                        Some(reader)
                    }
                    Err(e) => {
                        warn!("Failed to load GeoIP city database: {}", e);
                        None
                    }
                }
            } else {
                warn!("GeoIP city database not found at {}", path);
                None
            }
        } else {
            None
        };

        let asn_reader = if let Some(path) = asn_db_path {
            if Path::new(path).exists() {
                match Reader::open_readfile(path) {
                    Ok(reader) => {
                        debug!("Loaded GeoIP ASN database from {}", path);
                        Some(reader)
                    }
                    Err(e) => {
                        warn!("Failed to load GeoIP ASN database: {}", e);
                        None
                    }
                }
            } else {
                warn!("GeoIP ASN database not found at {}", path);
                None
            }
        } else {
            None
        };

        Ok(Self {
            city_reader,
            asn_reader,
        })
    }

    pub fn lookup(&self, ip: &str) -> GeoIpData {
        let ip_addr: IpAddr = match ip.parse() {
            Ok(addr) => addr,
            Err(_) => return GeoIpData::default(),
        };

        let mut data = GeoIpData::default();

        // City lookup
        if let Some(ref reader) = self.city_reader {
            if let Ok(city) = reader.lookup::<geoip2::City>(ip_addr) {
                if let Some(country) = city.country {
                    data.country = country.iso_code.unwrap_or_default().to_string();
                }

                if let Some(location) = city.location {
                    data.longitude = location.longitude;
                    data.latitude = location.latitude;
                    data.time_zone = location.time_zone.unwrap_or_default().to_string();
                }
            }
        }

        // ASN lookup
        if let Some(ref reader) = self.asn_reader {
            if let Ok(asn) = reader.lookup::<geoip2::Asn>(ip_addr) {
                data.asn = asn
                    .autonomous_system_organization
                    .unwrap_or_default()
                    .to_string();
            }
        }

        data
    }

    pub fn is_available(&self) -> bool {
        self.city_reader.is_some() || self.asn_reader.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_geoip_lookup_new_without_dbs() {
        let lookup = GeoIpLookup::new(None, None).unwrap();
        assert!(!lookup.is_available());
    }

    #[test]
    fn test_geoip_lookup_new_with_nonexistent_path() {
        let lookup = GeoIpLookup::new(
            Some("/nonexistent/path/GeoLite2-City.mmdb"),
            Some("/nonexistent/path/GeoLite2-ASN.mmdb"),
        )
        .unwrap();
        // Should gracefully handle missing files
        assert!(!lookup.is_available());
    }

    #[test]
    fn test_geoip_data_default() {
        let data = GeoIpData::default();
        assert!(data.asn.is_empty());
        assert!(data.country.is_empty());
        assert!(data.longitude.is_none());
        assert!(data.latitude.is_none());
        assert!(data.time_zone.is_empty());
    }

    #[test]
    fn test_geoip_lookup_without_dbs() {
        let lookup = GeoIpLookup::new(None, None).unwrap();
        let data = lookup.lookup("8.8.8.8");
        // Without databases, should return default
        assert!(data.country.is_empty());
        assert!(data.asn.is_empty());
    }

    #[test]
    fn test_geoip_lookup_invalid_ip() {
        let lookup = GeoIpLookup::new(None, None).unwrap();
        let data = lookup.lookup("not-an-ip-address");
        // Should return default for invalid IPs
        assert!(data.country.is_empty());
    }

    #[test]
    fn test_geoip_lookup_ipv6() {
        let lookup = GeoIpLookup::new(None, None).unwrap();
        // Valid IPv6, but no DB
        let data = lookup.lookup("2001:4860:4860::8888");
        assert!(data.country.is_empty());
    }

    #[test]
    fn test_geoip_lookup_localhost() {
        let lookup = GeoIpLookup::new(None, None).unwrap();
        let data = lookup.lookup("127.0.0.1");
        // Localhost should still parse as valid IP
        assert!(data.country.is_empty()); // But no geo data without DB
    }
}
