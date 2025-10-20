use anyhow::{anyhow, Result};
use reqwest::Client;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::{info, warn, debug};

/// Appium WebDriver controller for app automation
#[allow(dead_code)]
pub struct AppiumController {
    _client: Client,
    _server_url: String,
    _session_id: Option<String>,
    _platform: Platform,
    _capabilities: HashMap<String, Value>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Platform {
    Ios,
    Android,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct AppiumCapabilities {
    #[serde(rename = "platformName")]
    pub platform_name: String,
    #[serde(rename = "platformVersion")]
    pub platform_version: Option<String>,
    #[serde(rename = "deviceName")]
    pub device_name: String,
    #[serde(rename = "appPackage")]
    pub app_package: Option<String>,
    #[serde(rename = "appActivity")]
    pub app_activity: Option<String>,
    #[serde(rename = "bundleId")]
    pub bundle_id: Option<String>,
    #[serde(rename = "automationName")]
    pub automation_name: String,
    #[serde(rename = "noReset")]
    pub no_reset: bool,
    #[serde(flatten)]
    pub additional: HashMap<String, Value>,
}

#[derive(Debug, Clone)]
pub struct ElementLocator {
    pub strategy: LocatorStrategy,
    pub value: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum LocatorStrategy {
    Id,
    XPath,
    ClassName,
    AccessibilityId,
    Name,
    TagName,
}

impl AppiumController {
    #[allow(dead_code)]
    pub fn new(server_url: String, platform: Platform) -> Self {
        Self {
            _client: Client::new(),
            _server_url: server_url,
            _session_id: None,
            _platform: platform,
            _capabilities: HashMap::new(),
        }
    }

    /// Start a new Appium session
    #[allow(dead_code)]
    pub async fn start_session(&mut self, capabilities: AppiumCapabilities) -> Result<String> {
        info!("Starting Appium session for {:?} platform", self._platform);
        
        let session_payload = json!({
            "capabilities": {
                "alwaysMatch": capabilities,
                "firstMatch": [{}]
            }
        });

        let response = self
            ._client
            .post(format!("{}/session", self._server_url))
            .json(&session_payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Failed to start Appium session: {}", error_text));
        }

        let session_response: Value = response.json().await?;
        let session_id = session_response["value"]["sessionId"]
            .as_str()
            .ok_or_else(|| anyhow!("No session ID in response"))?
            .to_string();

        self._session_id = Some(session_id.clone());
        info!("Started Appium session: {}", session_id);
        
        Ok(session_id)
    }

    /// End the current Appium session
    #[allow(dead_code)]
    pub async fn end_session(&mut self) -> Result<()> {
        if let Some(session_id) = &self._session_id {
            info!("Ending Appium session: {}", session_id);
            
            let response = self
                ._client
                .delete(format!("{}/session/{}", self._server_url, session_id))
                .send()
                .await?;

            if !response.status().is_success() {
                warn!("Failed to end Appium session cleanly");
            }

            self._session_id = None;
        }
        
        Ok(())
    }

    /// Find element on screen
    pub async fn find_element(&self, locator: ElementLocator) -> Result<String> {
        let session_id = self._session_id.as_ref().ok_or_else(|| anyhow!("No active session"))?;
        
        let strategy = match locator.strategy {
            LocatorStrategy::Id => "id",
            LocatorStrategy::XPath => "xpath",
            LocatorStrategy::ClassName => "class name",
            LocatorStrategy::AccessibilityId => "accessibility id",
            LocatorStrategy::Name => "name",
            LocatorStrategy::TagName => "tag name",
        };

        let payload = json!({
            "using": strategy,
            "value": locator.value
        });

        let response = self
            ._client
            .post(format!("{}/session/{}/element", self._server_url, session_id))
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Failed to find element: {}", error_text));
        }

        let element_response: Value = response.json().await?;
        let element_id = element_response["value"]["ELEMENT"]
            .as_str()
            .or_else(|| element_response["value"]["element-6066-11e4-a52e-4f735466cecf"].as_str())
            .ok_or_else(|| anyhow!("No element ID in response"))?
            .to_string();

        debug!("Found element: {}", element_id);
        Ok(element_id)
    }

    /// Tap/click an element
    pub async fn tap_element(&self, element_id: &str) -> Result<()> {
        let session_id = self._session_id.as_ref().ok_or_else(|| anyhow!("No active session"))?;
        
        debug!("Tapping element: {}", element_id);
        
        let response = self
            ._client
            .post(format!("{}/session/{}/element/{}/click", self._server_url, session_id, element_id))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Failed to tap element: {}", error_text));
        }

        Ok(())
    }

    /// Type text into an element
    pub async fn type_text(&self, element_id: &str, text: &str) -> Result<()> {
        let session_id = self._session_id.as_ref().ok_or_else(|| anyhow!("No active session"))?;
        
        debug!("Typing text '{}' into element: {}", text, element_id);
        
        let payload = json!({
            "value": text.chars().map(|c| c.to_string()).collect::<Vec<_>>()
        });

        let response = self
            ._client
            .post(format!("{}/session/{}/element/{}/value", self._server_url, session_id, element_id))
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Failed to type text: {}", error_text));
        }

        Ok(())
    }

    /// Get element text
    pub async fn get_element_text(&self, element_id: &str) -> Result<String> {
        let session_id = self._session_id.as_ref().ok_or_else(|| anyhow!("No active session"))?;
        
        let response = self
            ._client
            .get(format!("{}/session/{}/element/{}/text", self._server_url, session_id, element_id))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Failed to get element text: {}", error_text));
        }

        let text_response: Value = response.json().await?;
        let text = text_response["value"]
            .as_str()
            .ok_or_else(|| anyhow!("No text in response"))?
            .to_string();

        Ok(text)
    }

    /// Wait for element to appear
    pub async fn wait_for_element(&self, locator: ElementLocator, timeout: std::time::Duration) -> Result<String> {
        let start = std::time::Instant::now();
        
        while start.elapsed() < timeout {
            match self.find_element(locator.clone()).await {
                Ok(element_id) => return Ok(element_id),
                Err(_) => {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                }
            }
        }
        
        Err(anyhow!("Element not found within timeout: {:?}", timeout))
    }

    /// Take screenshot
    #[allow(dead_code)]
    pub async fn take_screenshot(&self) -> Result<Vec<u8>> {
        let session_id = self._session_id.as_ref().ok_or_else(|| anyhow!("No active session"))?;
        
        let response = self
            ._client
            .get(format!("{}/session/{}/screenshot", self._server_url, session_id))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Failed to take screenshot: {}", error_text));
        }

        let screenshot_response: Value = response.json().await?;
        let base64_data = screenshot_response["value"]
            .as_str()
            .ok_or_else(|| anyhow!("No screenshot data in response"))?;

        let screenshot_data = base64::engine::general_purpose::STANDARD.decode(base64_data)?;
        Ok(screenshot_data)
    }

    /// Launch app via deep link (iOS) or intent (Android)
    #[allow(dead_code)]
    pub async fn launch_app_via_url(&self, url: &str) -> Result<()> {
        let session_id = self._session_id.as_ref().ok_or_else(|| anyhow!("No active session"))?;
        
        info!("Launching app via URL: {}", url);
        
        match self._platform {
            Platform::Ios => {
                // Use mobile:launchApp for iOS
                let payload = json!({
                    "script": "mobile: launchApp",
                    "args": [{
                        "bundleId": "chat.bitchat",
                        "arguments": ["-url", url]
                    }]
                });

                let response = self
                    ._client
                    .post(format!("{}/session/{}/execute", self._server_url, session_id))
                    .json(&payload)
                    .send()
                    .await?;

                if !response.status().is_success() {
                    let error_text = response.text().await?;
                    return Err(anyhow!("Failed to launch iOS app: {}", error_text));
                }
            }
            Platform::Android => {
                // Use mobile:deepLink for Android
                let payload = json!({
                    "script": "mobile: deepLink",
                    "args": [{
                        "url": url,
                        "package": "com.bitchat.droid"
                    }]
                });

                let response = self
                    ._client
                    .post(format!("{}/session/{}/execute", self._server_url, session_id))
                    .json(&payload)
                    .send()
                    .await?;

                if !response.status().is_success() {
                    let error_text = response.text().await?;
                    return Err(anyhow!("Failed to launch Android app: {}", error_text));
                }
            }
        }
        
        Ok(())
    }

    /// Get app state
    #[allow(dead_code)]
    pub async fn get_app_state(&self, app_id: &str) -> Result<AppState> {
        let session_id = self._session_id.as_ref().ok_or_else(|| anyhow!("No active session"))?;
        
        let payload = json!({
            "appId": app_id
        });

        let response = self
            ._client
            .post(format!("{}/session/{}/appium/device/app_state", self._server_url, session_id))
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Failed to get app state: {}", error_text));
        }

        let state_response: Value = response.json().await?;
        let state_code = state_response["value"]
            .as_u64()
            .ok_or_else(|| anyhow!("No app state in response"))?;

        let app_state = match state_code {
            0 => AppState::NotInstalled,
            1 => AppState::NotRunning,
            2 => AppState::RunningInBackground,
            3 => AppState::RunningInForeground,
            4 => AppState::RunningInBackground, // iOS specific
            _ => AppState::Unknown,
        };

        Ok(app_state)
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum AppState {
    NotInstalled,
    NotRunning,
    RunningInBackground,
    RunningInForeground,
    Unknown,
}

impl Default for AppiumCapabilities {
    fn default() -> Self {
        Self {
            platform_name: "iOS".to_string(),
            platform_version: None,
            device_name: "iPhone Simulator".to_string(),
            app_package: None,
            app_activity: None,
            bundle_id: Some("chat.bitchat".to_string()),
            automation_name: "XCUITest".to_string(),
            no_reset: true,
            additional: HashMap::new(),
        }
    }
}

/// High-level BitChat app automation helpers
#[allow(dead_code)]
pub struct BitChatAppAutomation {
    appium: AppiumController,
}

impl BitChatAppAutomation {
    #[allow(dead_code)]
    pub fn new(appium: AppiumController) -> Self {
        Self { appium }
    }

    /// Navigate to chat screen and send a message
    #[allow(dead_code)]
    pub async fn send_message(&self, recipient: &str, message: &str) -> Result<()> {
        info!("Sending message to {} via app automation", recipient);
        
        // Wait for main screen
        let chat_button = self.appium.wait_for_element(
            ElementLocator {
                strategy: LocatorStrategy::AccessibilityId,
                value: "chat-button".to_string(),
            },
            std::time::Duration::from_secs(10),
        ).await?;
        
        self.appium.tap_element(&chat_button).await?;
        
        // Find recipient field and enter recipient
        let recipient_field = self.appium.wait_for_element(
            ElementLocator {
                strategy: LocatorStrategy::AccessibilityId,
                value: "recipient-field".to_string(),
            },
            std::time::Duration::from_secs(5),
        ).await?;
        
        self.appium.type_text(&recipient_field, recipient).await?;
        
        // Find message field and enter message
        let message_field = self.appium.wait_for_element(
            ElementLocator {
                strategy: LocatorStrategy::AccessibilityId,
                value: "message-field".to_string(),
            },
            std::time::Duration::from_secs(5),
        ).await?;
        
        self.appium.type_text(&message_field, message).await?;
        
        // Tap send button
        let send_button = self.appium.find_element(ElementLocator {
            strategy: LocatorStrategy::AccessibilityId,
            value: "send-button".to_string(),
        }).await?;
        
        self.appium.tap_element(&send_button).await?;
        
        info!("Message sent successfully");
        Ok(())
    }

    /// Wait for and verify incoming message
    #[allow(dead_code)]
    pub async fn wait_for_message(&self, expected_text: &str, timeout: std::time::Duration) -> Result<bool> {
        info!("Waiting for message containing: {}", expected_text);
        
        let start = std::time::Instant::now();
        
        while start.elapsed() < timeout {
            // Look for message elements
            if let Ok(message_elements) = self.appium.find_element(ElementLocator {
                strategy: LocatorStrategy::ClassName,
                value: "message-bubble".to_string(),
            }).await {
                let text = self.appium.get_element_text(&message_elements).await?;
                if text.contains(expected_text) {
                    info!("Found expected message: {}", text);
                    return Ok(true);
                }
            }
            
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
        
        Ok(false)
    }
}