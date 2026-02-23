#![allow(clippy::expect_used)]

mod integration {
    use typub_adapters_core::{AdapterRegistrar, PlatformAdapter};
    use typub_core::AssetStrategy;

    use crate::adapter::HashnodeAdapter;
    use crate::config::{CAPABILITY, ID, register};

    #[test]
    fn test_capability_fields() {
        assert_eq!(CAPABILITY.id, "hashnode");
        assert_eq!(CAPABILITY.name, "Hashnode");
        assert_eq!(CAPABILITY.default_asset_strategy(), AssetStrategy::External);
        assert_eq!(CAPABILITY.asset_strategies.len(), 1);
    }

    #[test]
    fn test_register_adapter() {
        let mut registrar = AdapterRegistrar::new();
        register(&mut registrar).expect("register");
        assert!(registrar.capabilities().contains_key(ID));
    }

    #[test]
    fn test_adapter_public_interface() {
        let adapter = HashnodeAdapter::new_for_test();
        assert_eq!(adapter.id(), ID);
        assert_eq!(adapter.name(), "Hashnode");
        assert_eq!(adapter.asset_strategy(), AssetStrategy::External);
        assert!(adapter.supports_shared_link_rewrite());
    }
}
