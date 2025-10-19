use anyhow::{anyhow, Result};
use std::collections::HashMap;
use tokio::process::Command;
use tracing::{info, warn};
use uuid::Uuid;

use crate::config::TestConfig;
use crate::emulator::{AndroidEmulator, IosSimulator};
use crate::network::NetworkProxy;
use crate::appium::AppiumController;
use crate::ClientType;

/// Main orchestrator that coordinates emulator testing
pub struct EmulatorOrchestrator {
    config: TestConfig,
    _ios_simulators: HashMap<String, IosSimulator>,
    _android_emulators: HashMap<String, AndroidEmulator>,
    _network_proxy: Option<NetworkProxy>,
    _appium_controller: Option<AppiumController>,
    active_sessions: HashMap<String, TestSession>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct TestSession {
    pub id: String,
    pub _platform: Platform,
    pub _emulator_id: String,
    pub _proxy_port: Option<u16>,
    pub _appium_session: Option<String>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Platform {
    Ios,
    Android,
}

impl EmulatorOrchestrator {
    pub fn new(config: TestConfig) -> Self {
        Self {
            config,
            _ios_simulators: HashMap::new(),
            _android_emulators: HashMap::new(),
            _network_proxy: None,
            _appium_controller: None,
            active_sessions: HashMap::new(),
        }
    }

    /// Set up the emulator testing environment
    pub async fn setup_environment(&self) -> Result<()> {
        info!("Setting up emulator testing environment...");

        // Check required tools are available
        self.check_prerequisites().await?;

        // Start network proxy
        self.start_network_proxy().await?;

        // Set up iOS simulators
        self.setup_ios_simulators().await?;

        // Set up Android emulators
        self.setup_android_emulators().await?;

        // Start Appium server
        self.start_appium_server().await?;

        info!("Environment setup completed successfully");
        Ok(())
    }

    /// Clean up emulator environment
    pub async fn cleanup_environment(&self) -> Result<()> {
        info!("Cleaning up emulator environment...");

        // Stop all active test sessions
        for session in self.active_sessions.values() {
            self.cleanup_session(session).await?;
        }

        // Stop Appium server
        self.stop_appium_server().await?;

        // Stop network proxy
        self.stop_network_proxy().await?;

        // Clean up emulators
        self.cleanup_emulators().await?;

        info!("Environment cleanup completed");
        Ok(())
    }

    /// Run iOS emulator tests
    pub async fn run_ios_tests(&mut self, scenario: Option<String>) -> Result<()> {
        info!("Running iOS emulator tests...");

        // Start iOS simulator
        let simulator_id = self.start_ios_simulator().await?;
        
        // Install BitChat app
        self.install_ios_app(&simulator_id).await?;

        // Run test scenarios
        match scenario {
            Some(scenario_name) => {
                self.run_ios_scenario(&simulator_id, &scenario_name).await?;
            }
            None => {
                // Run all iOS scenarios
                let scenarios = vec![
                    "deterministic-messaging",
                    "transport-failover", 
                    "session-rekey",
                    "network-partition",
                ];
                
                for scenario_name in scenarios {
                    self.run_ios_scenario(&simulator_id, scenario_name).await?;
                }
            }
        }

        info!("iOS emulator tests completed");
        Ok(())
    }

    /// Run iOS ↔ iOS stability test (equivalent to old ios-simulator-test)
    pub async fn run_ios_to_ios_test(&mut self) -> Result<()> {
        info!("Starting iOS ↔ iOS communication test");
        
        // Setup multiple iOS simulators
        let device_names = vec!["BitChat-Alice", "BitChat-Bob"];
        let mut simulator_ids = Vec::new();
        
        for device_name in &device_names {
            info!("Setting up iOS simulator: {}", device_name);
            let simulator_id = self.create_ios_simulator(device_name).await?;
            simulator_ids.push(simulator_id);
        }
        
        // Install BitChat app on all simulators
        for (i, simulator_id) in simulator_ids.iter().enumerate() {
            info!("Installing BitChat app on simulator {}", device_names[i]);
            self.install_ios_app(simulator_id).await?;
        }
        
        // Launch apps on all simulators
        for (i, simulator_id) in simulator_ids.iter().enumerate() {
            info!("Launching BitChat app on simulator {}", device_names[i]);
            self.launch_ios_app(simulator_id).await?;
        }
        
        // Monitor stability for 60 seconds
        info!("Monitoring app stability for 60 seconds...");
        let start_time = std::time::Instant::now();
        let test_duration = std::time::Duration::from_secs(60);
        let check_interval = std::time::Duration::from_secs(15);
        
        while start_time.elapsed() < test_duration {
            let check_count = (start_time.elapsed().as_secs() / check_interval.as_secs()) + 1;
            info!("Stability check #{} at {:.1}s", check_count, start_time.elapsed().as_secs_f32());
            
            // Check if apps are still running
            let mut all_running = true;
            for (i, simulator_id) in simulator_ids.iter().enumerate() {
                if !self.check_ios_app_running(simulator_id).await? {
                    warn!("App stopped running on simulator {}", device_names[i]);
                    all_running = false;
                }
            }
            
            if !all_running {
                return Err(anyhow!("App stability test failed - some apps stopped running"));
            }
            
            let remaining = test_duration.saturating_sub(start_time.elapsed());
            let sleep_duration = check_interval.min(remaining);
            
            if sleep_duration.is_zero() {
                break;
            }
            
            tokio::time::sleep(sleep_duration).await;
        }
        
        info!("[OK] iOS ↔ iOS stability test completed successfully after {:.1}s", start_time.elapsed().as_secs_f32());
        
        // Cleanup simulators
        for (i, simulator_id) in simulator_ids.iter().enumerate() {
            info!("Cleaning up simulator {}", device_names[i]);
            self.cleanup_ios_simulator(simulator_id).await?;
        }
        
        Ok(())
    }

    /// Run Android ↔ Android stability test (equivalent to Android integration test)
    pub async fn run_android_to_android_test(&mut self) -> Result<()> {
        info!("Starting Android ↔ Android communication test");
        
        // Setup multiple Android emulators
        let device_names = vec!["BitChat-Alice", "BitChat-Bob"];
        let mut emulator_ids = Vec::new();
        
        for device_name in &device_names {
            info!("Setting up Android emulator: {}", device_name);
            let emulator_id = self.create_android_emulator(device_name).await?;
            emulator_ids.push(emulator_id);
        }
        
        // Install BitChat app on all emulators
        for (i, emulator_id) in emulator_ids.iter().enumerate() {
            info!("Installing BitChat app on emulator {}", device_names[i]);
            self.install_android_app(emulator_id).await?;
        }
        
        // Launch apps on all emulators
        for (i, emulator_id) in emulator_ids.iter().enumerate() {
            info!("Launching BitChat app on emulator {}", device_names[i]);
            self.launch_android_app(emulator_id).await?;
        }
        
        // Monitor stability for 60 seconds
        info!("Monitoring app stability for 60 seconds...");
        let start_time = std::time::Instant::now();
        let test_duration = std::time::Duration::from_secs(60);
        let check_interval = std::time::Duration::from_secs(15);
        
        while start_time.elapsed() < test_duration {
            let check_count = (start_time.elapsed().as_secs() / check_interval.as_secs()) + 1;
            info!("Stability check #{} at {:.1}s", check_count, start_time.elapsed().as_secs_f32());
            
            // Check if apps are still running
            let mut all_running = true;
            for (i, emulator_id) in emulator_ids.iter().enumerate() {
                if !self.check_android_app_running(emulator_id).await? {
                    warn!("App stopped running on emulator {}", device_names[i]);
                    all_running = false;
                }
            }
            
            if !all_running {
                return Err(anyhow!("App stability test failed - some apps stopped running"));
            }
            
            let remaining = test_duration.saturating_sub(start_time.elapsed());
            let sleep_duration = check_interval.min(remaining);
            
            if sleep_duration.is_zero() {
                break;
            }
            
            tokio::time::sleep(sleep_duration).await;
        }
        
        info!("[OK] Android ↔ Android stability test completed successfully after {:.1}s", start_time.elapsed().as_secs_f32());
        
        // Cleanup emulators
        for (i, emulator_id) in emulator_ids.iter().enumerate() {
            info!("Cleaning up emulator {}", device_names[i]);
            self.cleanup_android_emulator(emulator_id).await?;
        }
        
        Ok(())
    }

    /// Run Android emulator tests
    pub async fn run_android_tests(&mut self, scenario: Option<String>) -> Result<()> {
        info!("Running Android emulator tests...");

        // Start Android emulator
        let emulator_id = self.start_android_emulator().await?;
        
        // Install BitChat app
        self.install_android_app(&emulator_id).await?;

        // Run test scenarios
        match scenario {
            Some(scenario_name) => {
                self.run_android_scenario(&emulator_id, &scenario_name).await?;
            }
            None => {
                // Run all Android scenarios
                let scenarios = vec![
                    "deterministic-messaging",
                    "transport-failover",
                    "session-rekey", 
                    "network-partition",
                ];
                
                for scenario_name in scenarios {
                    self.run_android_scenario(&emulator_id, scenario_name).await?;
                }
            }
        }

        info!("Android emulator tests completed");
        Ok(())
    }

    /// Run test with flexible client type combinations
    pub async fn run_client_combination_test(
        &mut self, 
        client1: ClientType, 
        client2: ClientType, 
        scenario: Option<String>
    ) -> Result<()> {
        info!("Running {} ↔ {} client combination test...", client1, client2);

        match (&client1, &client2) {
            (ClientType::Ios, ClientType::Ios) => {
                info!("Starting iOS ↔ iOS test...");
                self.run_ios_to_ios_test().await?;
            }
            (ClientType::Android, ClientType::Android) => {
                info!("Starting Android ↔ Android test...");
                self.run_android_to_android_test().await?;
            }
            (ClientType::Ios, ClientType::Android) | 
            (ClientType::Android, ClientType::Ios) => {
                info!("Starting cross-platform {} ↔ {} test...", client1, client2);
                self.run_cross_platform_test(client1.clone(), client2.clone(), scenario).await?;
            }
        }

        info!("{} ↔ {} client combination test completed", client1, client2);
        Ok(())
    }

    /// Run cross-platform test between different client types
    async fn run_cross_platform_test(
        &mut self, 
        client1: ClientType, 
        client2: ClientType, 
        scenario: Option<String>
    ) -> Result<()> {
        info!("Setting up cross-platform test environment...");

        // Start first client
        let client1_id = match client1 {
            ClientType::Ios => {
                self.start_ios_simulator().await?
            }
            ClientType::Android => {
                self.start_android_emulator().await?
            }
        };

        // Start second client
        let client2_id = match client2 {
            ClientType::Ios => {
                self.start_ios_simulator().await?
            }
            ClientType::Android => {
                self.start_android_emulator().await?
            }
        };

        // Install apps on both clients
        match client1 {
            ClientType::Ios => self.install_ios_app(&client1_id).await?,
            ClientType::Android => self.install_android_app(&client1_id).await?,
        }

        match client2 {
            ClientType::Ios => self.install_ios_app(&client2_id).await?,
            ClientType::Android => self.install_android_app(&client2_id).await?,
        }

        // Run cross-platform scenarios
        match scenario {
            Some(scenario_name) => {
                self.run_cross_platform_scenario(&client1_id, &client2_id, &scenario_name, client1, client2).await?;
            }
            None => {
                // Run all cross-platform scenarios
                let scenarios = vec![
                    "cross-platform-messaging",
                    "cross-platform-discovery",
                    "cross-platform-sessions",
                ];
                
                for scenario_name in scenarios {
                    self.run_cross_platform_scenario(&client1_id, &client2_id, scenario_name, client1.clone(), client2.clone()).await?;
                }
            }
        }

        info!("Cross-platform test completed");
        Ok(())
    }

    /// Run a specific cross-platform scenario
    async fn run_cross_platform_scenario(
        &self,
        client1_id: &str,
        client2_id: &str,
        scenario: &str,
        client1_type: ClientType,
        client2_type: ClientType,
    ) -> Result<()> {
        info!("Running cross-platform scenario '{}' between {} ({}) and {} ({})", 
               scenario, client1_type, client1_id, client2_type, client2_id);

        // Implementation would depend on the specific scenario
        // This is a placeholder for the actual scenario execution
        match scenario {
            "cross-platform-messaging" => {
                info!("Testing message exchange between {} and {}", client1_type, client2_type);
                // TODO: Implement cross-platform messaging test
            }
            "cross-platform-discovery" => {
                info!("Testing peer discovery between {} and {}", client1_type, client2_type);
                // TODO: Implement cross-platform discovery test
            }
            "cross-platform-sessions" => {
                info!("Testing session establishment between {} and {}", client1_type, client2_type);
                // TODO: Implement cross-platform session test
            }
            _ => {
                return Err(anyhow!("Unknown cross-platform scenario: {}", scenario));
            }
        }

        info!("Cross-platform scenario '{}' completed", scenario);
        Ok(())
    }

    /// Run full compatibility matrix
    pub async fn run_compatibility_matrix(&mut self, filter: Option<String>) -> Result<()> {
        info!("Running full compatibility matrix...");

        // Test cases: iOS ↔ iOS, Android ↔ Android, iOS ↔ Android
        let test_combinations = vec![
            ("ios", "ios"),
            ("android", "android"),
            ("ios", "android"),
        ];

        for (platform_a, platform_b) in test_combinations {
            // Skip if filter is specified and doesn't match
            if let Some(ref filter_str) = filter {
                let combo_str = format!("{}-{}", platform_a, platform_b);
                if !combo_str.contains(filter_str) {
                    continue;
                }
            }

            info!("Testing {} ↔ {} compatibility...", platform_a, platform_b);
            self.run_cross_platform_legacy(platform_a, platform_b).await?;
        }

        info!("Compatibility matrix testing completed");
        Ok(())
    }

    /// Check that all required tools are available
    async fn check_prerequisites(&self) -> Result<()> {
        info!("Checking prerequisites...");

        // Check iOS tools (macOS only)
        #[cfg(target_os = "macos")]
        {
            if which::which("xcrun").is_err() {
                return Err(anyhow!("xcrun not found - Xcode command line tools required"));
            }
            
            // Check if simctl is available via xcrun
            let simctl_check = Command::new("xcrun")
                .args(["simctl", "help"])
                .output()
                .await;
            match simctl_check {
                Ok(output) if output.status.success() => {
                    // simctl is available via xcrun
                }
                _ => {
                    return Err(anyhow!("simctl not available via xcrun - iOS Simulator required"));
                }
            }
        }

        // Check Android tools
        if std::env::var("ANDROID_HOME").is_err() {
            warn!("ANDROID_HOME not set - Android emulator may not work");
        }
        if which::which("adb").is_err() {
            return Err(anyhow!("adb not found - Android SDK required"));
        }

        // Check network tools
        if which::which("mitmproxy").is_err() {
            return Err(anyhow!("mitmproxy not found - network interception required"));
        }

        // Check Node.js for Appium
        if which::which("node").is_err() {
            return Err(anyhow!("node not found - Node.js required for Appium"));
        }
        if which::which("appium").is_err() {
            return Err(anyhow!("appium not found - run 'npm install -g appium'"));
        }

        info!("All prerequisites checked successfully");
        Ok(())
    }

    async fn start_network_proxy(&self) -> Result<()> {
        info!("Starting network proxy...");
        // Implementation would start mitmproxy
        Ok(())
    }

    async fn stop_network_proxy(&self) -> Result<()> {
        info!("Stopping network proxy...");
        // Implementation would stop mitmproxy
        Ok(())
    }

    async fn setup_ios_simulators(&self) -> Result<()> {
        info!("Setting up iOS simulators...");
        // Implementation would configure iOS simulators
        Ok(())
    }

    async fn setup_android_emulators(&self) -> Result<()> {
        info!("Setting up Android emulators...");
        // Implementation would configure Android emulators
        Ok(())
    }

    async fn start_appium_server(&self) -> Result<()> {
        info!("Starting Appium server...");
        // Implementation would start Appium server
        Ok(())
    }

    async fn stop_appium_server(&self) -> Result<()> {
        info!("Stopping Appium server...");
        // Implementation would stop Appium server
        Ok(())
    }

    async fn cleanup_emulators(&self) -> Result<()> {
        info!("Cleaning up emulators...");
        // Implementation would clean up emulator state
        Ok(())
    }

    async fn cleanup_session(&self, _session: &TestSession) -> Result<()> {
        // Implementation would clean up individual test session
        Ok(())
    }

    async fn start_ios_simulator(&mut self) -> Result<String> {
        info!("Starting iOS simulator...");
        let simulator_id = Uuid::new_v4().to_string();
        // Implementation would start iOS simulator and return ID
        Ok(simulator_id)
    }

    async fn start_android_emulator(&mut self) -> Result<String> {
        info!("Starting Android emulator...");
        let emulator_id = Uuid::new_v4().to_string();
        // Implementation would start Android emulator and return ID
        Ok(emulator_id)
    }

    async fn install_ios_app(&self, simulator_id: &str) -> Result<()> {
        info!("Installing BitChat iOS app on simulator {}", simulator_id);
        
        // First, build the iOS app if needed
        self.build_ios_app().await?;
        
        // Install the app bundle on the simulator
        let app_bundle_path = &self.config.ios.app_source;
        
        let output = Command::new("xcrun")
            .args(["simctl", "install", simulator_id, app_bundle_path])
            .output()
            .await?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Failed to install iOS app: {}", error));
        }

        info!("Successfully installed BitChat iOS app on simulator {}", simulator_id);
        Ok(())
    }

    async fn install_android_app(&self, emulator_id: &str) -> Result<()> {
        info!("Installing BitChat Android app on emulator {}", emulator_id);
        
        // First, build the Android app if needed
        self.build_android_app().await?;
        
        // Install the APK on the emulator
        let apk_path = &self.config.android.apk_source;
        
        let output = Command::new("adb")
            .args(["install", "-r", apk_path])
            .output()
            .await?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Failed to install Android APK: {}", error));
        }

        info!("Successfully installed BitChat Android app on emulator {}", emulator_id);
        Ok(())
    }

    async fn run_ios_scenario(&self, simulator_id: &str, scenario: &str) -> Result<()> {
        info!("Running iOS scenario '{}' on simulator {}", scenario, simulator_id);
        // Implementation would run specific iOS test scenario
        Ok(())
    }

    async fn run_android_scenario(&self, emulator_id: &str, scenario: &str) -> Result<()> {
        info!("Running Android scenario '{}' on emulator {}", scenario, emulator_id);
        // Implementation would run specific Android test scenario
        Ok(())
    }

    async fn run_cross_platform_legacy(&mut self, platform_a: &str, platform_b: &str) -> Result<()> {
        info!("Running cross-platform test: {} ↔ {}", platform_a, platform_b);
        // Implementation would coordinate cross-platform testing
        Ok(())
    }

    /// Build the real BitChat iOS app
    async fn build_ios_app(&self) -> Result<()> {
        let app_path = &self.config.ios.app_source;
        
        // Check if app already exists
        if std::path::Path::new(app_path).exists() {
            info!("iOS app already exists at: {}", app_path);
            return Ok(());
        }
        
        info!("Building real BitChat iOS app using simulator Justfile...");
        
        // Use the simulator Justfile build-ios target
        let output = Command::new("just")
            .args(["build-ios"])
            .current_dir("..") // Go up to simulator directory
            .output()
            .await?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Failed to build iOS app: {}", error));
        }

        // Verify the app was built
        if !std::path::Path::new(app_path).exists() {
            return Err(anyhow!("iOS app not found at expected path: {}", app_path));
        }

        info!("Successfully built BitChat iOS app");
        Ok(())
    }

    /// Build the real BitChat Android app
    async fn build_android_app(&self) -> Result<()> {
        info!("Building real BitChat Android app...");
        
        let output = Command::new("./gradlew")
            .args(["assembleDebug"])
            .current_dir("./vendored/bitchat-android")
            .env("ANDROID_HOME", std::env::var("ANDROID_HOME").unwrap_or_default())
            .output()
            .await?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Failed to build Android app: {}", error));
        }

        info!("Successfully built BitChat Android app");
        Ok(())
    }

    /// Create and boot a new iOS simulator
    async fn create_ios_simulator(&mut self, device_name: &str) -> Result<String> {
        info!("Creating iOS simulator device: {}", device_name);
        
        let device_type = "iPhone 15 Pro";
        let runtime = "com.apple.CoreSimulator.SimRuntime.iOS-26-0";
        
        // Delete any existing device with this name
        let _delete_result = Command::new("xcrun")
            .args(["simctl", "delete", device_name])
            .output()
            .await;
        
        // Create new device
        let create_output = Command::new("xcrun")
            .args(["simctl", "create", device_name, device_type, runtime])
            .output()
            .await?;
        
        if !create_output.status.success() {
            let stderr = String::from_utf8_lossy(&create_output.stderr);
            return Err(anyhow!("Failed to create iOS simulator '{}': {}", device_name, stderr));
        }
        
        let simulator_id = String::from_utf8(create_output.stdout)?.trim().to_string();
        
        // Boot the simulator
        info!("Booting iOS simulator: {}", device_name);
        let boot_output = Command::new("xcrun")
            .args(["simctl", "boot", &simulator_id])
            .output()
            .await?;
        
        if !boot_output.status.success() {
            let stderr = String::from_utf8_lossy(&boot_output.stderr);
            // Device might already be booted, which is okay
            warn!("Boot warning for '{}': {}", device_name, stderr);
        }
        
        // Wait for device to be ready
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        
        info!("iOS simulator '{}' created and booted with ID: {}", device_name, simulator_id);
        Ok(simulator_id)
    }
    
    /// Launch BitChat app on iOS simulator
    async fn launch_ios_app(&self, simulator_id: &str) -> Result<()> {
        info!("Launching BitChat app on simulator {}", simulator_id);
        
        let app_bundle_id = "tech.permissionless.bitchat";
        
        let launch_output = Command::new("xcrun")
            .args(["simctl", "launch", simulator_id, app_bundle_id])
            .output()
            .await?;
        
        if !launch_output.status.success() {
            let stderr = String::from_utf8_lossy(&launch_output.stderr);
            return Err(anyhow!("Failed to launch BitChat app on simulator: {}", stderr));
        }
        
        info!("BitChat app launched successfully on simulator");
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        Ok(())
    }
    
    /// Check if BitChat app is running on iOS simulator
    async fn check_ios_app_running(&self, simulator_id: &str) -> Result<bool> {
        let ps_output = Command::new("xcrun")
            .args(["simctl", "spawn", simulator_id, "launchctl", "list"])
            .output()
            .await?;
        
        if ps_output.status.success() {
            let ps_stdout = String::from_utf8_lossy(&ps_output.stdout);
            let app_running = ps_stdout.contains("bitchat") || ps_stdout.contains("tech.permissionless.bitchat");
            Ok(app_running)
        } else {
            warn!("Failed to check app status on simulator");
            Ok(false)
        }
    }
    
    /// Cleanup iOS simulator
    async fn cleanup_ios_simulator(&self, simulator_id: &str) -> Result<()> {
        info!("Cleaning up iOS simulator: {}", simulator_id);
        
        // Terminate app
        let _terminate_result = Command::new("xcrun")
            .args(["simctl", "terminate", simulator_id, "tech.permissionless.bitchat"])
            .output()
            .await;
        
        // Shutdown simulator
        let _shutdown_result = Command::new("xcrun")
            .args(["simctl", "shutdown", simulator_id])
            .output()
            .await;
        
        // Delete simulator
        let delete_output = Command::new("xcrun")
            .args(["simctl", "delete", simulator_id])
            .output()
            .await;
        
        if delete_output.is_ok() {
            info!("Successfully cleaned up iOS simulator");
        } else {
            warn!("Failed to delete iOS simulator: {}", simulator_id);
        }
        
        Ok(())
    }
    
    /// Create and start a new Android emulator
    async fn create_android_emulator(&mut self, device_name: &str) -> Result<String> {
        info!("Creating Android emulator device: {}", device_name);
        
        let avd_name = format!("BitChat_{}", device_name);
        
        // Delete any existing AVD with this name
        let _delete_result = Command::new("avdmanager")
            .args(["delete", "avd", "-n", &avd_name])
            .output()
            .await;
        
        // Create new AVD
        let create_output = Command::new("avdmanager")
            .args([
                "create", "avd",
                "-n", &avd_name,
                "-k", "system-images;android-30;google_apis;x86_64",
                "--device", "pixel_4",
                "--force"
            ])
            .output()
            .await?;
        
        if !create_output.status.success() {
            let stderr = String::from_utf8_lossy(&create_output.stderr);
            return Err(anyhow!("Failed to create Android AVD '{}': {}", avd_name, stderr));
        }
        
        // Start the emulator
        info!("Starting Android emulator: {}", device_name);
        let mut emulator_process = Command::new("emulator")
            .args([
                "-avd", &avd_name,
                "-no-audio",
                "-no-window",
                "-gpu", "off",
                "-no-snapshot",
                "-wipe-data"
            ])
            .spawn()?;
        
        // Wait for emulator to be ready
        info!("Waiting for Android emulator to be ready...");
        let mut ready = false;
        for _attempt in 0..30 {
            let check_output = Command::new("adb")
                .args(["shell", "getprop", "init.svc.bootanim"])
                .output()
                .await;
            
            if let Ok(output) = check_output {
                let status = String::from_utf8_lossy(&output.stdout);
                if status.trim() == "stopped" {
                    ready = true;
                    break;
                }
            }
            
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        }
        
        if !ready {
            // Kill the emulator process if it's not ready
            let _ = emulator_process.kill().await;
            return Err(anyhow!("Android emulator failed to start within timeout"));
        }
        
        // Get emulator serial number
        let serial_output = Command::new("adb")
            .args(["devices"])
            .output()
            .await?;
        
        let devices_output = String::from_utf8_lossy(&serial_output.stdout);
        let emulator_id = devices_output
            .lines()
            .find(|line| line.contains("emulator") && line.contains("device"))
            .and_then(|line| line.split_whitespace().next())
            .unwrap_or("emulator-5554")
            .to_string();
        
        info!("Android emulator '{}' created and started with ID: {}", device_name, emulator_id);
        Ok(emulator_id)
    }
    
    /// Launch BitChat app on Android emulator
    async fn launch_android_app(&self, emulator_id: &str) -> Result<()> {
        info!("Launching BitChat app on emulator {}", emulator_id);
        
        let package_name = "com.bitchat.android"; // This should match the actual package name
        let activity_name = "com.bitchat.android.MainActivity"; // This should match the actual main activity
        
        let launch_output = Command::new("adb")
            .args([
                "-s", emulator_id,
                "shell", "am", "start",
                "-n", &format!("{}/{}", package_name, activity_name)
            ])
            .output()
            .await?;
        
        if !launch_output.status.success() {
            let stderr = String::from_utf8_lossy(&launch_output.stderr);
            return Err(anyhow!("Failed to launch BitChat app on emulator: {}", stderr));
        }
        
        info!("BitChat app launched successfully on emulator");
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        Ok(())
    }
    
    /// Check if BitChat app is running on Android emulator
    async fn check_android_app_running(&self, emulator_id: &str) -> Result<bool> {
        let ps_output = Command::new("adb")
            .args(["-s", emulator_id, "shell", "ps", "|", "grep", "bitchat"])
            .output()
            .await?;
        
        if ps_output.status.success() {
            let ps_stdout = String::from_utf8_lossy(&ps_output.stdout);
            let app_running = ps_stdout.contains("bitchat") || ps_stdout.contains("com.bitchat");
            Ok(app_running)
        } else {
            // Alternative check using dumpsys
            let dumpsys_output = Command::new("adb")
                .args(["-s", emulator_id, "shell", "dumpsys", "activity", "activities"])
                .output()
                .await?;
            
            if dumpsys_output.status.success() {
                let dumpsys_stdout = String::from_utf8_lossy(&dumpsys_output.stdout);
                let app_running = dumpsys_stdout.contains("bitchat") || dumpsys_stdout.contains("com.bitchat");
                Ok(app_running)
            } else {
                warn!("Failed to check app status on emulator");
                Ok(false)
            }
        }
    }
    
    /// Cleanup Android emulator
    async fn cleanup_android_emulator(&self, emulator_id: &str) -> Result<()> {
        info!("Cleaning up Android emulator: {}", emulator_id);
        
        // Force stop app
        let _stop_result = Command::new("adb")
            .args(["-s", emulator_id, "shell", "am", "force-stop", "com.bitchat.android"])
            .output()
            .await;
        
        // Kill emulator
        let _kill_result = Command::new("adb")
            .args(["-s", emulator_id, "emu", "kill"])
            .output()
            .await;
        
        // Wait a bit for cleanup
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        
        info!("Successfully cleaned up Android emulator");
        Ok(())
    }
}