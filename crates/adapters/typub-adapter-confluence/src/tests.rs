#![allow(clippy::expect_used)]

mod integration {
    use typub_adapters_core::{AdapterRegistrar, PlatformAdapter};
    use typub_core::AssetStrategy;

    use crate::adapter::ConfluenceAdapter;
    use crate::config::{CAPABILITY, ID, register};

    #[test]
    fn test_capability_values() {
        assert_eq!(CAPABILITY.id, "confluence");
        assert_eq!(
            CAPABILITY.default_math_rendering(),
            typub_core::MathRendering::Latex
        );
        assert_eq!(CAPABILITY.default_asset_strategy(), AssetStrategy::Upload);
        const { assert!(!CAPABILITY.code_highlight) };
    }

    #[test]
    fn test_register_adapter() {
        let mut registrar = AdapterRegistrar::new();
        register(&mut registrar).expect("register");
        assert!(registrar.capabilities().contains_key(ID));
    }

    #[test]
    fn test_adapter_public_interface() {
        let adapter = ConfluenceAdapter::new_for_test();
        assert_eq!(adapter.id(), ID);
        assert_eq!(adapter.name(), "Confluence");
        assert_eq!(adapter.asset_strategy(), AssetStrategy::Upload);
        assert!(adapter.supports_shared_link_rewrite());
    }
}
