use woothee::parser::Parser;

use crate::domain::DeviceType;

#[derive(Debug, Default)]
pub struct ParsedUserAgent {
    pub browser: String,
    pub device: String,
    pub device_type: DeviceType,
    pub os: String,
    pub is_bot: bool,
}

pub fn parse_user_agent(user_agent: &str) -> ParsedUserAgent {
    let parser = Parser::new();

    match parser.parse(user_agent) {
        Some(result) => {
            let is_bot = result.category == "crawler"
                || result.name.to_lowercase().contains("bot")
                || result.name.to_lowercase().contains("spider")
                || result.name.to_lowercase() == "googlebot";

            let device_type = if is_bot {
                DeviceType::Robot
            } else {
                match result.category {
                    "smartphone" => DeviceType::Phone,
                    "mobilephone" => DeviceType::Phone,
                    "tablet" => DeviceType::Tablet,
                    "pc" => DeviceType::Desktop,
                    "crawler" => DeviceType::Robot,
                    _ => DeviceType::Other,
                }
            };

            ParsedUserAgent {
                browser: result.name.to_string(),
                device: result.vendor.to_string(),
                device_type,
                os: result.os.to_string(),
                is_bot,
            }
        }
        None => ParsedUserAgent {
            device_type: DeviceType::Other,
            ..Default::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_chrome_desktop() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36";
        let parsed = parse_user_agent(ua);
        assert_eq!(parsed.browser, "Chrome");
        assert_eq!(parsed.device_type, DeviceType::Desktop);
        assert_eq!(parsed.os, "Windows 10");
        assert!(!parsed.is_bot);
    }

    #[test]
    fn test_parse_firefox() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:89.0) Gecko/20100101 Firefox/89.0";
        let parsed = parse_user_agent(ua);
        assert_eq!(parsed.browser, "Firefox");
        assert_eq!(parsed.device_type, DeviceType::Desktop);
        assert!(!parsed.is_bot);
    }

    #[test]
    fn test_parse_safari_mac() {
        let ua = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/14.1.1 Safari/605.1.15";
        let parsed = parse_user_agent(ua);
        assert_eq!(parsed.browser, "Safari");
        assert_eq!(parsed.device_type, DeviceType::Desktop);
        assert!(!parsed.is_bot);
    }

    #[test]
    fn test_parse_googlebot() {
        let ua = "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)";
        let parsed = parse_user_agent(ua);
        assert!(parsed.is_bot);
        assert_eq!(parsed.device_type, DeviceType::Robot);
    }

    #[test]
    fn test_parse_bingbot() {
        let ua = "Mozilla/5.0 (compatible; bingbot/2.0; +http://www.bing.com/bingbot.htm)";
        let parsed = parse_user_agent(ua);
        assert!(parsed.is_bot);
        assert_eq!(parsed.device_type, DeviceType::Robot);
    }

    #[test]
    fn test_parse_iphone() {
        let ua = "Mozilla/5.0 (iPhone; CPU iPhone OS 14_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/14.1.1 Mobile/15E148 Safari/604.1";
        let parsed = parse_user_agent(ua);
        assert_eq!(parsed.device_type, DeviceType::Phone);
        assert!(!parsed.is_bot);
    }

    #[test]
    fn test_parse_android() {
        let ua = "Mozilla/5.0 (Linux; Android 11; SM-G991B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.120 Mobile Safari/537.36";
        let parsed = parse_user_agent(ua);
        assert_eq!(parsed.device_type, DeviceType::Phone);
        assert!(!parsed.is_bot);
    }

    #[test]
    fn test_parse_ipad() {
        let ua = "Mozilla/5.0 (iPad; CPU OS 14_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/14.1.1 Mobile/15E148 Safari/604.1";
        let parsed = parse_user_agent(ua);
        // iPad may be detected as tablet or phone depending on woothee
        assert!(
            parsed.device_type == DeviceType::Tablet || parsed.device_type == DeviceType::Phone
        );
        assert!(!parsed.is_bot);
    }

    #[test]
    fn test_parse_empty_ua() {
        let parsed = parse_user_agent("");
        assert_eq!(parsed.device_type, DeviceType::Other);
        assert!(!parsed.is_bot);
    }

    #[test]
    fn test_parse_unknown_ua() {
        let parsed = parse_user_agent("SomeUnknownApp/1.0");
        // Should gracefully handle unknown user agents
        assert!(!parsed.is_bot || parsed.device_type == DeviceType::Robot);
    }

    #[test]
    fn test_is_not_bot_for_regular_browsers() {
        let browsers = vec![
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/91.0",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 Safari/605.1.15",
            "Mozilla/5.0 (X11; Linux x86_64; rv:89.0) Gecko/20100101 Firefox/89.0",
        ];

        for ua in browsers {
            let parsed = parse_user_agent(ua);
            assert!(!parsed.is_bot, "Should not be detected as bot: {}", ua);
        }
    }
}
