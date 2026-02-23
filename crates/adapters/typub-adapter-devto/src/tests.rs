#![allow(clippy::expect_used)]

mod integration {
    use typub_adapters_core::{AdapterRegistrar, PlatformAdapter};
    use typub_core::AssetStrategy;

    use crate::adapter::DevtoAdapter;
    use crate::config::{CAPABILITY, ID, register};

    #[test]
    fn test_capability_fields() {
        assert_eq!(CAPABILITY.id, "devto");
        assert_eq!(CAPABILITY.name, "Dev.to");
        assert_eq!(CAPABILITY.default_asset_strategy(), AssetStrategy::External);
        assert_eq!(CAPABILITY.asset_strategies.len(), 2);
    }

    #[test]
    fn test_register_adapter() {
        let mut registrar = AdapterRegistrar::new();
        register(&mut registrar).expect("register");
        assert!(registrar.capabilities().contains_key(ID));
    }

    #[test]
    fn test_adapter_public_interface() {
        let adapter = DevtoAdapter::new_for_test();
        assert_eq!(adapter.id(), ID);
        assert_eq!(adapter.name(), "Dev.to");
        assert_eq!(adapter.asset_strategy(), AssetStrategy::External);
        assert!(adapter.supports_shared_link_rewrite());
    }
}
