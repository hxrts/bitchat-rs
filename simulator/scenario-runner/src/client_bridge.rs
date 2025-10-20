//! Client Type Bridge
//!
//! Provides a unified client type system that bridges scenario-runner and emulator-rig
//! frameworks, enabling cross-framework testing (CLI↔iOS, Web↔Android, etc.)

use crate::event_orchestrator::ClientType as ScenarioClientType;

/// Unified client type that encompasses all supported BitChat implementations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnifiedClientType {
    /// Command-line interface application (native Rust)
    Cli,
    /// Web browser application (WASM)
    Web,
    /// iOS application (Swift, via emulator-rig)
    Ios,
    /// Android application (Kotlin, via emulator-rig)
    Android,
}

impl UnifiedClientType {
    /// Get human-readable name for the client type
    pub fn name(&self) -> &'static str {
        match self {
            UnifiedClientType::Cli => "CLI Application",
            UnifiedClientType::Web => "Web Application",
            UnifiedClientType::Ios => "iOS Application",
            UnifiedClientType::Android => "Android Application",
        }
    }

    /// Get short identifier for the client type
    pub fn identifier(&self) -> &'static str {
        match self {
            UnifiedClientType::Cli => "cli",
            UnifiedClientType::Web => "web",
            UnifiedClientType::Ios => "ios",
            UnifiedClientType::Android => "android",
        }
    }

    /// Check if this client type requires emulator-rig framework
    pub fn requires_emulator_rig(&self) -> bool {
        matches!(self, UnifiedClientType::Ios | UnifiedClientType::Android)
    }

    /// Check if this client type can be run via scenario-runner directly
    pub fn supports_scenario_runner(&self) -> bool {
        matches!(self, UnifiedClientType::Cli | UnifiedClientType::Web)
    }

    /// Get the testing framework needed for this client type
    pub fn framework(&self) -> TestingFramework {
        match self {
            UnifiedClientType::Cli | UnifiedClientType::Web => TestingFramework::ScenarioRunner,
            UnifiedClientType::Ios | UnifiedClientType::Android => TestingFramework::EmulatorRig,
        }
    }
}

/// Testing framework type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestingFramework {
    /// Event-driven scenario testing (CLI and Web)
    ScenarioRunner,
    /// Real mobile app testing (iOS and Android)
    EmulatorRig,
}

impl std::fmt::Display for UnifiedClientType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ----------------------------------------------------------------------------
// Conversion from scenario-runner ClientType
// ----------------------------------------------------------------------------

impl From<ScenarioClientType> for UnifiedClientType {
    fn from(client_type: ScenarioClientType) -> Self {
        match client_type {
            ScenarioClientType::Cli => UnifiedClientType::Cli,
            ScenarioClientType::Web => UnifiedClientType::Web,
        }
    }
}

impl TryFrom<UnifiedClientType> for ScenarioClientType {
    type Error = ClientTypeBridgeError;

    fn try_from(unified: UnifiedClientType) -> Result<Self, Self::Error> {
        match unified {
            UnifiedClientType::Cli => Ok(ScenarioClientType::Cli),
            UnifiedClientType::Web => Ok(ScenarioClientType::Web),
            UnifiedClientType::Ios | UnifiedClientType::Android => {
                Err(ClientTypeBridgeError::UnsupportedInScenarioRunner(unified))
            }
        }
    }
}

// ----------------------------------------------------------------------------
// Conversion to/from emulator-rig ClientType
// ----------------------------------------------------------------------------

impl From<bitchat_emulator_harness::ClientType> for UnifiedClientType {
    fn from(client_type: bitchat_emulator_harness::ClientType) -> Self {
        match client_type {
            bitchat_emulator_harness::ClientType::Ios => UnifiedClientType::Ios,
            bitchat_emulator_harness::ClientType::Android => UnifiedClientType::Android,
        }
    }
}

impl TryFrom<UnifiedClientType> for bitchat_emulator_harness::ClientType {
    type Error = ClientTypeBridgeError;

    fn try_from(unified: UnifiedClientType) -> Result<Self, Self::Error> {
        match unified {
            UnifiedClientType::Ios => Ok(bitchat_emulator_harness::ClientType::Ios),
            UnifiedClientType::Android => Ok(bitchat_emulator_harness::ClientType::Android),
            UnifiedClientType::Cli | UnifiedClientType::Web => {
                Err(ClientTypeBridgeError::UnsupportedInEmulatorRig(unified))
            }
        }
    }
}

// ----------------------------------------------------------------------------
// String parsing for CLI arguments
// ----------------------------------------------------------------------------

impl std::str::FromStr for UnifiedClientType {
    type Err = ClientTypeBridgeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "cli" => Ok(UnifiedClientType::Cli),
            "web" | "wasm" => Ok(UnifiedClientType::Web),
            "ios" | "swift" => Ok(UnifiedClientType::Ios),
            "android" | "kotlin" => Ok(UnifiedClientType::Android),
            _ => Err(ClientTypeBridgeError::UnknownClientType(s.to_string())),
        }
    }
}

// ----------------------------------------------------------------------------
// Error Types
// ----------------------------------------------------------------------------

/// Errors that can occur when bridging between client type systems
#[derive(Debug, thiserror::Error)]
pub enum ClientTypeBridgeError {
    /// Client type not supported in scenario-runner framework
    #[error("Client type {0} is not supported in scenario-runner framework (use emulator-rig)")]
    UnsupportedInScenarioRunner(UnifiedClientType),

    /// Client type not supported in emulator-rig framework
    #[error("Client type {0} is not supported in emulator-rig framework (use scenario-runner)")]
    UnsupportedInEmulatorRig(UnifiedClientType),

    /// Unknown client type string
    #[error("Unknown client type: {0}. Valid types: cli, web, ios, android")]
    UnknownClientType(String),

    /// Incompatible client pair for testing framework
    #[error("Cannot test {0} ↔ {1}: requires bridging between different frameworks")]
    IncompatibleFrameworks(UnifiedClientType, UnifiedClientType),
}

// ----------------------------------------------------------------------------
// Client Pair Validation
// ----------------------------------------------------------------------------

/// Represents a pair of clients for cross-implementation testing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientPair {
    pub client1: UnifiedClientType,
    pub client2: UnifiedClientType,
}

impl ClientPair {
    /// Create a new client pair
    pub fn new(client1: UnifiedClientType, client2: UnifiedClientType) -> Self {
        Self { client1, client2 }
    }

    /// Check if this pair can be tested within a single framework
    pub fn is_same_framework(&self) -> bool {
        self.client1.framework() == self.client2.framework()
    }

    /// Check if this pair requires framework bridging
    pub fn requires_bridging(&self) -> bool {
        !self.is_same_framework()
    }

    /// Get the testing strategy for this client pair
    pub fn testing_strategy(&self) -> TestingStrategy {
        if self.is_same_framework() {
            TestingStrategy::SingleFramework(self.client1.framework())
        } else {
            TestingStrategy::CrossFramework {
                framework1: self.client1.framework(),
                framework2: self.client2.framework(),
            }
        }
    }

    /// Get a human-readable description of this client pair
    pub fn description(&self) -> String {
        format!("{} ↔ {}", self.client1.identifier(), self.client2.identifier())
    }
}

impl std::fmt::Display for ClientPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description())
    }
}

/// Testing strategy required for a client pair
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestingStrategy {
    /// Both clients can be tested within a single framework
    SingleFramework(TestingFramework),
    /// Clients require bridging between two frameworks
    CrossFramework {
        framework1: TestingFramework,
        framework2: TestingFramework,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unified_client_type_conversion() {
        // Test scenario-runner conversions
        let cli = ScenarioClientType::Cli;
        let unified: UnifiedClientType = cli.into();
        assert_eq!(unified, UnifiedClientType::Cli);
        
        let web = ScenarioClientType::Web;
        let unified: UnifiedClientType = web.into();
        assert_eq!(unified, UnifiedClientType::Web);

        // Test emulator-rig conversions
        let ios = bitchat_emulator_harness::ClientType::Ios;
        let unified: UnifiedClientType = ios.into();
        assert_eq!(unified, UnifiedClientType::Ios);

        let android = bitchat_emulator_harness::ClientType::Android;
        let unified: UnifiedClientType = android.into();
        assert_eq!(unified, UnifiedClientType::Android);
    }

    #[test]
    fn test_framework_detection() {
        assert!(UnifiedClientType::Cli.supports_scenario_runner());
        assert!(UnifiedClientType::Web.supports_scenario_runner());
        assert!(!UnifiedClientType::Ios.supports_scenario_runner());
        assert!(!UnifiedClientType::Android.supports_scenario_runner());

        assert!(!UnifiedClientType::Cli.requires_emulator_rig());
        assert!(!UnifiedClientType::Web.requires_emulator_rig());
        assert!(UnifiedClientType::Ios.requires_emulator_rig());
        assert!(UnifiedClientType::Android.requires_emulator_rig());
    }

    #[test]
    fn test_client_pair_framework_detection() {
        // Same framework pairs
        let cli_web = ClientPair::new(UnifiedClientType::Cli, UnifiedClientType::Web);
        assert!(cli_web.is_same_framework());
        assert!(!cli_web.requires_bridging());

        let ios_android = ClientPair::new(UnifiedClientType::Ios, UnifiedClientType::Android);
        assert!(ios_android.is_same_framework());
        assert!(!ios_android.requires_bridging());

        // Cross-framework pairs
        let cli_ios = ClientPair::new(UnifiedClientType::Cli, UnifiedClientType::Ios);
        assert!(!cli_ios.is_same_framework());
        assert!(cli_ios.requires_bridging());

        let web_android = ClientPair::new(UnifiedClientType::Web, UnifiedClientType::Android);
        assert!(!web_android.is_same_framework());
        assert!(web_android.requires_bridging());
    }

    #[test]
    fn test_string_parsing() {
        assert_eq!("cli".parse::<UnifiedClientType>().unwrap(), UnifiedClientType::Cli);
        assert_eq!("web".parse::<UnifiedClientType>().unwrap(), UnifiedClientType::Web);
        assert_eq!("wasm".parse::<UnifiedClientType>().unwrap(), UnifiedClientType::Web);
        assert_eq!("ios".parse::<UnifiedClientType>().unwrap(), UnifiedClientType::Ios);
        assert_eq!("swift".parse::<UnifiedClientType>().unwrap(), UnifiedClientType::Ios);
        assert_eq!("android".parse::<UnifiedClientType>().unwrap(), UnifiedClientType::Android);
        assert_eq!("kotlin".parse::<UnifiedClientType>().unwrap(), UnifiedClientType::Android);

        assert!("invalid".parse::<UnifiedClientType>().is_err());
    }
}

