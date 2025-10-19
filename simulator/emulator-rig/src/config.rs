use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestConfig {
    /// iOS emulator configuration
    pub ios: IosConfig,
    /// Android emulator configuration  
    pub android: AndroidConfig,
    /// Network proxy configuration
    pub network: NetworkConfig,
    /// Appium configuration
    pub appium: AppiumConfig,
    /// Test timing configuration
    pub timing: TimingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IosConfig {
    /// iOS Simulator device type (e.g. "iPhone 15 Pro")
    pub device_type: String,
    /// iOS runtime version (e.g. "iOS-17-0")
    pub runtime: String,
    /// BitChat iOS app bundle ID
    pub app_bundle_id: String,
    /// App store URL or local IPA path
    pub app_source: String,
    /// Deep link URL scheme for BitChat
    pub url_scheme: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AndroidConfig {
    /// Android Virtual Device name
    pub avd_name: String,
    /// Android API level
    pub api_level: u32,
    /// Device architecture (x86_64, arm64-v8a)
    pub arch: String,
    /// BitChat Android package name
    pub package_name: String,
    /// APK source (Play Store, local file, etc.)
    pub apk_source: String,
    /// Intent action for deep links
    pub intent_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// mitmproxy listen port
    pub proxy_port: u16,
    /// Web interface port for mitmproxy
    pub web_port: u16,
    /// Certificate path for HTTPS interception
    pub cert_path: Option<String>,
    /// Should capture all traffic or filter to BitChat only
    pub capture_all: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppiumConfig {
    /// Appium server URL
    pub server_url: String,
    /// Session timeout
    pub timeout_ms: u64,
    /// Implicit wait timeout
    pub implicit_wait_ms: u64,
    /// Platform-specific capabilities
    pub capabilities: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingConfig {
    /// How long to wait for emulator boot
    pub emulator_boot_timeout: Duration,
    /// How long to wait for app launch
    pub app_launch_timeout: Duration,
    /// How long to wait for network events
    pub network_event_timeout: Duration,
    /// How long to wait for UI elements
    pub ui_element_timeout: Duration,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            ios: IosConfig {
                device_type: "iPhone 15 Pro".to_string(),
                runtime: "iOS-17-0".to_string(),
                app_bundle_id: "tech.permissionless.bitchat".to_string(),
                app_source: "./ios-app-result/BitChat.app".to_string(),
                url_scheme: "bitchat://".to_string(),
            },
            android: AndroidConfig {
                avd_name: "BitChat_Test_API_34".to_string(),
                api_level: 34,
                arch: "x86_64".to_string(),
                package_name: "com.bitchat.android".to_string(),
                apk_source: "./vendored/bitchat-android/app/build/outputs/apk/debug/app-debug.apk".to_string(),
                intent_action: "android.intent.action.VIEW".to_string(),
            },
            network: NetworkConfig {
                proxy_port: 8080,
                web_port: 8081,
                cert_path: None,
                capture_all: false,
            },
            appium: AppiumConfig {
                server_url: "http://127.0.0.1:4723".to_string(),
                timeout_ms: 30000,
                implicit_wait_ms: 5000,
                capabilities: serde_json::json!({}),
            },
            timing: TimingConfig {
                emulator_boot_timeout: Duration::from_secs(120),
                app_launch_timeout: Duration::from_secs(30),
                network_event_timeout: Duration::from_secs(60),
                ui_element_timeout: Duration::from_secs(10),
            },
        }
    }
}

impl TestConfig {
    /// Load configuration from file or environment
    #[allow(dead_code)]
    pub fn load() -> anyhow::Result<Self> {
        // Try to load from environment variables first
        if let Ok(config_str) = std::env::var("BITCHAT_TEST_CONFIG") {
            return Ok(serde_json::from_str(&config_str)?);
        }

        // Try to load from config file
        if let Ok(config_path) = std::env::var("BITCHAT_TEST_CONFIG_PATH") {
            let config_content = std::fs::read_to_string(config_path)?;
            return Ok(serde_json::from_str(&config_content)?);
        }

        // Fall back to default configuration
        Ok(Self::default())
    }

    /// Save configuration to file
    #[allow(dead_code)]
    pub fn save(&self, path: &str) -> anyhow::Result<()> {
        let config_json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, config_json)?;
        Ok(())
    }
}