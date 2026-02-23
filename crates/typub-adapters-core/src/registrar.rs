use std::collections::HashMap;

use anyhow::Result;

use typub_config::Config;

use crate::adapter::PlatformAdapter;
use crate::capability::AdapterCapability;

/// Factory function type for creating adapters.
pub type AdapterFactory = fn(&Config) -> Result<Box<dyn PlatformAdapter>>;

/// Registration API for adapter factories and capabilities.
#[derive(Default)]
pub struct AdapterRegistrar {
    factories: HashMap<String, AdapterFactory>,
    capabilities: HashMap<String, AdapterCapability>,
}

impl AdapterRegistrar {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_factory(&mut self, platform_id: &str, factory: AdapterFactory) -> Result<()> {
        if platform_id.is_empty() {
            anyhow::bail!("Platform ID cannot be empty");
        }
        if self.factories.contains_key(platform_id) {
            anyhow::bail!(
                "Factory already registered for platform '{}'; duplicate registration is not allowed",
                platform_id
            );
        }
        self.factories.insert(platform_id.to_string(), factory);
        Ok(())
    }

    pub fn register_capability(
        &mut self,
        platform_id: &str,
        capability: AdapterCapability,
    ) -> Result<()> {
        if platform_id.is_empty() {
            anyhow::bail!("Platform ID cannot be empty");
        }
        if self.capabilities.contains_key(platform_id) {
            anyhow::bail!(
                "Capability already registered for platform '{}'; duplicate registration is not allowed",
                platform_id
            );
        }
        self.capabilities
            .insert(platform_id.to_string(), capability);
        Ok(())
    }

    pub fn into_factories(self) -> HashMap<String, AdapterFactory> {
        self.factories
    }

    pub fn capabilities(&self) -> &HashMap<String, AdapterCapability> {
        &self.capabilities
    }

    pub fn into_capabilities(self) -> HashMap<String, AdapterCapability> {
        self.capabilities
    }

    pub fn into_parts(
        self,
    ) -> (
        HashMap<String, AdapterFactory>,
        HashMap<String, AdapterCapability>,
    ) {
        (self.factories, self.capabilities)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::capability::{AdapterCapability, NodePolicy};
    use typub_core::{
        AssetStrategy, CapabilitySupport, MathDelimiters, MathRendering, NodePolicyAction,
        TaxonomyCapability,
    };

    const TEST_CAP: AdapterCapability = AdapterCapability {
        id: "test",
        name: "Test",
        short_code: "ts",
        local_output: false,
        requires_config: true,
        taxonomy: TaxonomyCapability::new(
            CapabilitySupport::Supported,
            CapabilitySupport::Supported,
            CapabilitySupport::Supported,
            typub_core::DraftSupport::None,
        ),
        asset_strategies: &[AssetStrategy::Upload],
        math_renderings: &[MathRendering::Svg],
        math_delimiters: &[MathDelimiters::Dollar],
        code_highlight: true,
        notes: "",
        node_policy: NodePolicy {
            raw: NodePolicyAction::Pass,
            unknown: NodePolicyAction::Pass,
        },
    };

    fn dummy_factory(_: &Config) -> anyhow::Result<Box<dyn crate::adapter::PlatformAdapter>> {
        anyhow::bail!("not implemented")
    }

    #[test]
    fn test_registrar_new() {
        let registrar = AdapterRegistrar::new();
        assert!(registrar.capabilities().is_empty());
    }

    #[test]
    fn test_registrar_register_factory() {
        let mut registrar = AdapterRegistrar::new();
        registrar
            .register_factory("test", dummy_factory)
            .expect("register");
        let factories = registrar.into_factories();
        assert!(factories.contains_key("test"));
    }

    #[test]
    fn test_registrar_register_capability() {
        let mut registrar = AdapterRegistrar::new();
        registrar
            .register_capability("test", TEST_CAP)
            .expect("register");
        assert!(registrar.capabilities().contains_key("test"));
    }

    #[test]
    fn test_registrar_duplicate_factory() {
        let mut registrar = AdapterRegistrar::new();
        registrar
            .register_factory("test", dummy_factory)
            .expect("first");
        let err = registrar
            .register_factory("test", dummy_factory)
            .expect_err("duplicate");
        assert!(err.to_string().contains("already registered"));
    }

    #[test]
    fn test_registrar_duplicate_capability() {
        let mut registrar = AdapterRegistrar::new();
        registrar
            .register_capability("test", TEST_CAP)
            .expect("first");
        let err = registrar
            .register_capability("test", TEST_CAP)
            .expect_err("duplicate");
        assert!(err.to_string().contains("already registered"));
    }

    #[test]
    fn test_registrar_into_parts() {
        let mut registrar = AdapterRegistrar::new();
        registrar
            .register_factory("f1", dummy_factory)
            .expect("register");
        registrar
            .register_capability("c1", TEST_CAP)
            .expect("register");

        let (factories, capabilities) = registrar.into_parts();
        assert!(factories.contains_key("f1"));
        assert!(capabilities.contains_key("c1"));
    }
}
